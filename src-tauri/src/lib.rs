mod config;
mod db;
mod error;
mod extract;
mod providers;
mod rag;
mod vector_store;

use base64::Engine;

use config::RagConfig;
use db::{Character, ChatSession, Db, DocMeta, Entities, Place, StoredMessage, Vault};
use error::{AppError, AppResult};
use rag::Answer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tokio::sync::Mutex;

/// Shared app state. `db` owns its own internal lock; `config` is guarded here.
struct AppState {
    client: reqwest::Client,
    db: Db,
    config: Mutex<RagConfig>,
    data_dir: PathBuf,
    /// Set by `cancel_generation` to stop an in-flight streaming answer.
    cancel: AtomicBool,
}

impl AppState {
    fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }
    fn active(&self) -> AppResult<String> {
        self.db
            .active_vault()?
            .ok_or_else(|| AppError::Msg("nenhum vault ativo".into()))
    }
}

fn load_config(path: &PathBuf) -> RagConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ---- Config ---------------------------------------------------------------

#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>) -> AppResult<RagConfig> {
    Ok(state.config.lock().await.clone())
}

#[tauri::command]
async fn save_config(state: tauri::State<'_, AppState>, config: RagConfig) -> AppResult<()> {
    let json = serde_json::to_string_pretty(&config)?;
    std::fs::create_dir_all(&state.data_dir)?;
    std::fs::write(state.config_path(), json)?;
    *state.config.lock().await = config;
    Ok(())
}

// ---- Vaults ---------------------------------------------------------------

#[tauri::command]
fn list_vaults(state: tauri::State<'_, AppState>) -> AppResult<Vec<Vault>> {
    state.db.list_vaults()
}

#[tauri::command]
fn get_active_vault(state: tauri::State<'_, AppState>) -> AppResult<Option<String>> {
    state.db.active_vault()
}

#[tauri::command]
fn set_active_vault(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.set_active_vault(&id)
}

#[tauri::command]
fn create_vault(state: tauri::State<'_, AppState>, name: String) -> AppResult<Vault> {
    let vault = state.db.create_vault(name)?;
    state.db.set_active_vault(&vault.id)?;
    Ok(vault)
}

#[tauri::command]
fn rename_vault(state: tauri::State<'_, AppState>, id: String, name: String) -> AppResult<()> {
    state.db.rename_vault(&id, &name)
}

#[tauri::command]
fn delete_vault(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.delete_vault(&id)
}

// ---- Documents ------------------------------------------------------------

#[tauri::command]
fn list_documents(state: tauri::State<'_, AppState>) -> AppResult<Vec<DocMeta>> {
    let vault = state.active()?;
    state.db.list_documents(&vault)
}

#[tauri::command]
async fn ingest_document(
    state: tauri::State<'_, AppState>,
    name: String,
    content: String,
) -> AppResult<DocMeta> {
    if content.trim().is_empty() {
        return Err(AppError::Msg("documento vazio".into()));
    }
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    persist_document(&state, &vault, &cfg, name, content).await
}

/// Ingest a binary document (PDF/DOCX) sent as base64. Text is extracted in
/// Rust, then it flows through the same chunk→embed pipeline as plain text.
#[tauri::command]
async fn ingest_binary(
    state: tauri::State<'_, AppState>,
    name: String,
    data: String,
) -> AppResult<DocMeta> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| AppError::Msg(format!("base64 inválido: {e}")))?;
    let content = extract::extract_text(&name, &bytes)?;
    if content.trim().is_empty() {
        return Err(AppError::Msg("nenhum texto extraído do documento".into()));
    }
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    persist_document(&state, &vault, &cfg, name, content).await
}

/// The embedding a vault is (or would be) indexed with: "provider/model".
fn emb_tag(cfg: &RagConfig) -> String {
    format!("{}/{}", cfg.embedding_provider, cfg.embedding_model)
}

/// Shared ingest tail: skip identical docs already embedded with the current
/// model, otherwise chunk + embed + persist and stamp the vault's index model.
async fn persist_document(
    state: &tauri::State<'_, AppState>,
    vault: &str,
    cfg: &RagConfig,
    name: String,
    content: String,
) -> AppResult<DocMeta> {
    let id = blake3::hash(content.as_bytes()).to_hex().to_string();
    let tag = emb_tag(cfg);
    // Same bytes, already embedded with the same model → nothing to do.
    if state.db.indexed_embedding(vault)?.as_deref() == Some(tag.as_str()) {
        if let Some(existing) = state.db.get_document(vault, &id)? {
            return Ok(existing);
        }
    }
    let built = rag::build_document(&state.client, cfg, name, content).await?;
    let meta = DocMeta {
        id: built.id,
        name: built.name,
        kind: built.kind,
        pages: built.chunks.len(),
        status: "Indexado".into(),
        added_label: "agora".into(),
    };
    state.db.add_document(vault, &meta, &built.chunks)?;
    state.db.set_indexed_embedding(vault, &tag)?;
    Ok(meta)
}

/// Index state for the active vault, so the UI can offer a reindex when the
/// embedding model has changed since the documents were embedded.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexInfo {
    indexed: String,
    current: String,
    stale: bool,
}

#[tauri::command]
async fn index_info(state: tauri::State<'_, AppState>) -> AppResult<IndexInfo> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let indexed = state.db.indexed_embedding(&vault)?.unwrap_or_default();
    let current = emb_tag(&cfg);
    let stale = !indexed.is_empty() && indexed != current;
    Ok(IndexInfo { indexed, current, stale })
}

/// Re-embed every chunk in the active vault with the current embedding model
/// and stamp the vault. Returns the number of chunks re-embedded.
#[tauri::command]
async fn reindex(state: tauri::State<'_, AppState>) -> AppResult<usize> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let tag = emb_tag(&cfg);
    let chunks = state.db.load_chunks(&vault)?;
    if chunks.is_empty() {
        state.db.set_indexed_embedding(&vault, &tag)?;
        return Ok(0);
    }
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let vecs = providers::embed(&state.client, &cfg, &texts).await?;
    let pairs: Vec<(String, Vec<f32>)> = chunks
        .into_iter()
        .zip(vecs)
        .map(|(c, v)| (c.id, v))
        .collect();
    let n = pairs.len();
    state.db.update_chunk_vectors(&vault, &pairs)?;
    state.db.set_indexed_embedding(&vault, &tag)?;
    Ok(n)
}

#[tauri::command]
fn remove_document(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let vault = state.active()?;
    state.db.remove_document(&vault, &id)
}

// ---- Ask ------------------------------------------------------------------

#[tauri::command]
async fn ask(
    state: tauri::State<'_, AppState>,
    question: String,
    history: Option<Vec<rag::HistoryTurn>>,
) -> AppResult<Answer> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let chunks = state.db.load_chunks(&vault)?;
    let (names, relations) = graph_inputs(&state, &vault)?;
    rag::ask(&state.client, &cfg, &chunks, question, history.unwrap_or_default(), &names, &relations).await
}

/// Entity names + relations for the active vault, fed to GraphRAG-lite so the
/// retriever can inject the relation subgraph around entities the question names.
fn graph_inputs(state: &tauri::State<'_, AppState>, vault: &str) -> AppResult<(Vec<String>, Vec<db::Relation>)> {
    let ents = state.db.entities(vault)?;
    let mut names: Vec<String> = Vec::with_capacity(ents.characters.len() + ents.places.len());
    names.extend(ents.characters.iter().map(|c| c.name.clone()));
    names.extend(ents.places.iter().map(|p| p.name.clone()));
    Ok((names, ents.relations))
}

/// Streamed events sent to the frontend during `ask_stream`.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum StreamEvent {
    Token { value: String },
    Done { sources: Vec<rag::Source> },
    Error { message: String },
}

/// Streaming chat: emits each generated token as it arrives, then a final
/// `done` event carrying the sources (or an `error` event on failure).
#[tauri::command]
async fn ask_stream(
    state: tauri::State<'_, AppState>,
    question: String,
    history: Option<Vec<rag::HistoryTurn>>,
    on_event: tauri::ipc::Channel<StreamEvent>,
) -> AppResult<()> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let chunks = state.db.load_chunks(&vault)?;
    let (names, relations) = graph_inputs(&state, &vault)?;

    // Fresh generation — clear any leftover cancel request.
    state.cancel.store(false, Ordering::Relaxed);

    let ch = on_event.clone();
    let result = rag::ask_stream(
        &state.client,
        &cfg,
        &chunks,
        question,
        history.unwrap_or_default(),
        &names,
        &relations,
        &state.cancel,
        |tok| {
            let _ = ch.send(StreamEvent::Token { value: tok.to_string() });
        },
    )
    .await;

    match result {
        Ok(sources) => {
            let _ = on_event.send(StreamEvent::Done { sources });
        }
        Err(e) => {
            let _ = on_event.send(StreamEvent::Error { message: e.to_string() });
        }
    }
    Ok(())
}

/// Stop an in-flight streaming answer. The stream loop checks this flag and
/// returns the partial text generated so far.
#[tauri::command]
fn cancel_generation(state: tauri::State<'_, AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

// ---- Chat sessions --------------------------------------------------------

#[tauri::command]
fn list_sessions(state: tauri::State<'_, AppState>) -> AppResult<Vec<ChatSession>> {
    let vault = state.active()?;
    state.db.list_sessions(&vault)
}

#[tauri::command]
fn create_session(state: tauri::State<'_, AppState>, title: String) -> AppResult<ChatSession> {
    let vault = state.active()?;
    state.db.create_session(&vault, &title)
}

#[tauri::command]
fn rename_session(state: tauri::State<'_, AppState>, id: String, title: String) -> AppResult<()> {
    state.db.rename_session(&id, &title)
}

#[tauri::command]
fn delete_session(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.delete_session(&id)
}

#[tauri::command]
fn session_messages(state: tauri::State<'_, AppState>, id: String) -> AppResult<Vec<StoredMessage>> {
    state.db.session_messages(&id)
}

/// Summarize the first exchange into a short title and store it on the session.
/// Returns the generated title.
#[tauri::command]
async fn generate_session_title(
    state: tauri::State<'_, AppState>,
    id: String,
    question: String,
    answer: String,
) -> AppResult<String> {
    let cfg = state.config.lock().await.clone();
    let title = rag::summarize_title(&state.client, &cfg, &question, &answer).await?;
    if !title.trim().is_empty() {
        state.db.rename_session(&id, &title)?;
    }
    Ok(title)
}

#[tauri::command]
fn add_message(
    state: tauri::State<'_, AppState>,
    session: String,
    role: String,
    text: String,
    thinking: String,
    sources: serde_json::Value,
) -> AppResult<()> {
    let vault = state.active()?;
    state.db.add_message(&vault, &session, &role, &text, &thinking, &sources)
}

// ---- Entities -------------------------------------------------------------

#[tauri::command]
fn get_entities(state: tauri::State<'_, AppState>) -> AppResult<Entities> {
    let vault = state.active()?;
    state.db.entities(&vault)
}

/// Extract entities from the vault. Incremental by default: only documents not
/// covered by a previous run are scanned, and results are MERGED — entities the
/// user edited or added are never overwritten. `force = true` re-scans every
/// document (still preserving edited/added entities).
#[tauri::command]
async fn extract_entities(
    state: tauri::State<'_, AppState>,
    force: Option<bool>,
) -> AppResult<Entities> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let chunks = state.db.load_chunks(&vault)?;
    let force = force.unwrap_or(false);

    if force {
        state.db.reset_extracted(&vault)?;
    }

    // Pick the chunks to scan: everything on a forced run, otherwise only chunks
    // from documents not yet extracted.
    let done = state.db.extracted_docs(&vault)?;
    let target: Vec<_> = if force {
        chunks.clone()
    } else {
        chunks.iter().filter(|c| !done.contains(&c.doc_id)).cloned().collect()
    };

    // Nothing new to do — return the current entities untouched.
    if target.is_empty() {
        return state.db.entities(&vault);
    }

    // Pass the names already saved so the extractor can fold a name introduced in
    // this run into a fuller name from a previous run (cross-run dedup).
    let existing = state.db.entities(&vault)?;
    let existing_chars: Vec<String> = existing.characters.iter().map(|c| c.name.clone()).collect();
    let existing_places: Vec<String> = existing.places.iter().map(|p| p.name.clone()).collect();

    let (characters, places, relations) =
        rag::extract_entities(&state.client, &cfg, &target, &existing_chars, &existing_places)
            .await?;
    state.db.merge_extracted(&vault, &characters, &places, &relations)?;

    // Record the documents we just covered so the next run skips them.
    let mut ids: Vec<String> = target.iter().map(|c| c.doc_id.clone()).collect();
    ids.sort();
    ids.dedup();
    state.db.mark_extracted(&vault, &ids)?;

    state.db.entities(&vault)
}

#[tauri::command]
fn add_character(state: tauri::State<'_, AppState>, character: Character) -> AppResult<()> {
    let vault = state.active()?;
    state.db.add_character(&vault, &character)
}

#[tauri::command]
fn add_place(state: tauri::State<'_, AppState>, place: Place) -> AppResult<()> {
    let vault = state.active()?;
    state.db.add_place(&vault, &place)
}

#[tauri::command]
fn update_character(state: tauri::State<'_, AppState>, character: Character) -> AppResult<()> {
    let vault = state.active()?;
    state.db.update_character(&vault, &character)
}

#[tauri::command]
fn update_place(state: tauri::State<'_, AppState>, place: Place) -> AppResult<()> {
    let vault = state.active()?;
    state.db.update_place(&vault, &place)
}

/// Manually add a relation (graph edge) between two entities. Feeds GraphRAG so
/// user-curated links improve retrieval as more chapters make auto-extraction noisier.
#[tauri::command]
fn add_relation(state: tauri::State<'_, AppState>, relation: db::Relation) -> AppResult<()> {
    let vault = state.active()?;
    state.db.add_relation(&vault, &relation)
}

/// Delete a relation by its (from, to, label) triple.
#[tauri::command]
fn remove_relation(state: tauri::State<'_, AppState>, relation: db::Relation) -> AppResult<()> {
    let vault = state.active()?;
    state.db.remove_relation(&vault, &relation)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // Auto-update stack (desktop only): the updater downloads/installs the signed
    // bundle from the GitHub Release; process enables the relaunch afterwards.
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&data_dir).ok();

            let db = Db::open(&data_dir.join("yellow-lore.db"))
                .expect("falha ao abrir o banco SQLite");
            let config = load_config(&data_dir.join("config.json"));

            app.manage(AppState {
                client: reqwest::Client::new(),
                db,
                config: Mutex::new(config),
                data_dir,
                cancel: AtomicBool::new(false),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            list_vaults,
            get_active_vault,
            set_active_vault,
            create_vault,
            rename_vault,
            delete_vault,
            list_documents,
            ingest_document,
            ingest_binary,
            remove_document,
            index_info,
            reindex,
            ask,
            ask_stream,
            cancel_generation,
            list_sessions,
            create_session,
            rename_session,
            delete_session,
            session_messages,
            generate_session_title,
            add_message,
            get_entities,
            extract_entities,
            add_character,
            add_place,
            update_character,
            update_place,
            add_relation,
            remove_relation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
