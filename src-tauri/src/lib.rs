mod config;
mod db;
mod error;
mod providers;
mod rag;
mod vector_store;

use config::RagConfig;
use db::{Character, Db, DocMeta, Entities, Place, Vault};
use error::{AppError, AppResult};
use rag::Answer;
use std::path::PathBuf;
use tauri::Manager;
use tokio::sync::Mutex;

/// Shared app state. `db` owns its own internal lock; `config` is guarded here.
struct AppState {
    client: reqwest::Client,
    db: Db,
    config: Mutex<RagConfig>,
    data_dir: PathBuf,
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
fn get_active_vault(state: tauri::State<'_, AppState>) -> AppResult<String> {
    state.active()
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
    let built = rag::build_document(&state.client, &cfg, name, content).await?;
    let meta = DocMeta {
        id: built.id,
        name: built.name,
        kind: built.kind,
        pages: built.chunks.len(),
        status: "Indexado".into(),
        added_label: "agora".into(),
    };
    state.db.add_document(&vault, &meta, &built.chunks)?;
    Ok(meta)
}

#[tauri::command]
fn remove_document(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let vault = state.active()?;
    state.db.remove_document(&vault, &id)
}

// ---- Ask ------------------------------------------------------------------

#[tauri::command]
async fn ask(state: tauri::State<'_, AppState>, question: String) -> AppResult<Answer> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let chunks = state.db.load_chunks(&vault)?;
    rag::ask(&state.client, &cfg, &chunks, question).await
}

// ---- Entities -------------------------------------------------------------

#[tauri::command]
fn get_entities(state: tauri::State<'_, AppState>) -> AppResult<Entities> {
    let vault = state.active()?;
    state.db.entities(&vault)
}

#[tauri::command]
async fn extract_entities(state: tauri::State<'_, AppState>) -> AppResult<Entities> {
    let vault = state.active()?;
    let cfg = state.config.lock().await.clone();
    let chunks = state.db.load_chunks(&vault)?;
    let (characters, places, relations) =
        rag::extract_entities(&state.client, &cfg, &chunks).await?;
    state.db.replace_entities(&vault, &characters, &places, &relations)?;
    state.db.entities(&vault)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            remove_document,
            ask,
            get_entities,
            extract_entities,
            update_character,
            update_place
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
