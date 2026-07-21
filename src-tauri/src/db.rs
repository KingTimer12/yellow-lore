use crate::error::{AppError, AppResult};
use crate::vector_store::Chunk;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

// ---- Models (camelCase for the frontend) ----------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocMeta {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub pages: usize, // chunk count
    pub status: String,
    pub added_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub id: String,
    pub name: String,
    pub role: String,
    pub summary: String,
    pub traits: Vec<String>,
    pub status: String,
    pub source_doc: String,
    pub source_quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub summary: String,
    pub status: String,
    pub source_doc: String,
    pub source_quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub thinking: String,
    /// Sources as the JSON array the frontend uses (doc/quote/text objects).
    pub sources: serde_json::Value,
}

/// Everything the frontend needs to render the entity views for a vault.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entities {
    pub characters: Vec<Character>,
    pub places: Vec<Place>,
    pub relations: Vec<Relation>,
}

// ---- Database -------------------------------------------------------------

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path).map_err(sql)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
            CREATE TABLE IF NOT EXISTS vaults (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, name TEXT, kind TEXT,
                pages INTEGER, status TEXT, added_label TEXT
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, doc_id TEXT NOT NULL,
                doc_name TEXT, text TEXT, vector TEXT
            );
            CREATE TABLE IF NOT EXISTS characters (
                id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, name TEXT, role TEXT,
                summary TEXT, traits TEXT, status TEXT, source_doc TEXT, source_quote TEXT
            );
            CREATE TABLE IF NOT EXISTS places (
                id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, name TEXT, kind TEXT,
                summary TEXT, status TEXT, source_doc TEXT, source_quote TEXT
            );
            CREATE TABLE IF NOT EXISTS relations (
                id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, from_name TEXT, to_name TEXT, label TEXT
            );
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, title TEXT,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY, session_id TEXT NOT NULL, vault_id TEXT NOT NULL,
                role TEXT, text TEXT, thinking TEXT, sources TEXT,
                ordinal INTEGER, created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_vault ON chunks(vault_id);
            CREATE INDEX IF NOT EXISTS idx_docs_vault ON documents(vault_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_vault ON sessions(vault_id);
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            ",
        )
        .map_err(sql)?;
        Ok(Db { conn: Mutex::new(conn) })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    // --- Vaults ---
    //
    // Vaults start empty (Obsidian-style): the app opens with no vault, and the
    // user creates the first one. There is no auto-created default vault.

    pub fn list_vaults(&self) -> AppResult<Vec<Vault>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, created_at FROM vaults ORDER BY created_at ASC")
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Vault {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    created_at: r.get(2)?,
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn create_vault(&self, name: String) -> AppResult<Vault> {
        let id = Uuid::new_v4().to_string();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO vaults (id, name, created_at) VALUES (?1, ?2, datetime('now'))",
            params![id, name],
        )
        .map_err(sql)?;
        let created_at: String = conn
            .query_row("SELECT created_at FROM vaults WHERE id = ?1", params![id], |r| r.get(0))
            .map_err(sql)?;
        Ok(Vault { id, name, created_at })
    }

    pub fn rename_vault(&self, id: &str, name: &str) -> AppResult<()> {
        self.lock()
            .execute("UPDATE vaults SET name = ?2 WHERE id = ?1", params![id, name])
            .map_err(sql)?;
        Ok(())
    }

    pub fn delete_vault(&self, id: &str) -> AppResult<()> {
        {
            let conn = self.lock();
            for t in ["documents", "chunks", "characters", "places", "relations", "sessions", "messages"] {
                conn.execute(&format!("DELETE FROM {t} WHERE vault_id = ?1"), params![id])
                    .map_err(sql)?;
            }
            conn.execute("DELETE FROM vaults WHERE id = ?1", params![id]).map_err(sql)?;
        }
        // If we removed the active vault, fall back to the first remaining one
        // (or clear the active pointer when none are left).
        if self.active_vault()?.as_deref() == Some(id) {
            match self.list_vaults()?.first() {
                Some(first) => self.set_active_vault(&first.id)?,
                None => self.clear_active_vault()?,
            }
        }
        Ok(())
    }

    fn clear_active_vault(&self) -> AppResult<()> {
        self.lock()
            .execute("DELETE FROM meta WHERE key = 'active_vault'", [])
            .map_err(sql)?;
        Ok(())
    }

    pub fn active_vault(&self) -> AppResult<Option<String>> {
        let conn = self.lock();
        let v = conn
            .query_row("SELECT value FROM meta WHERE key = 'active_vault'", [], |r| r.get::<_, String>(0))
            .ok();
        Ok(v)
    }

    pub fn set_active_vault(&self, id: &str) -> AppResult<()> {
        self.lock()
            .execute(
                "INSERT INTO meta (key, value) VALUES ('active_vault', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = ?1",
                params![id],
            )
            .map_err(sql)?;
        Ok(())
    }

    // --- Documents & chunks ---

    pub fn list_documents(&self, vault: &str) -> AppResult<Vec<DocMeta>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, kind, pages, status, added_label FROM documents WHERE vault_id = ?1")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![vault], |r| {
                Ok(DocMeta {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    kind: r.get(2)?,
                    pages: r.get::<_, i64>(3)? as usize,
                    status: r.get(4)?,
                    added_label: r.get(5)?,
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn add_document(&self, vault: &str, meta: &DocMeta, chunks: &[Chunk]) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(sql)?;
        tx.execute("DELETE FROM documents WHERE id = ?1", params![meta.id]).map_err(sql)?;
        tx.execute("DELETE FROM chunks WHERE doc_id = ?1", params![meta.id]).map_err(sql)?;
        tx.execute(
            "INSERT INTO documents (id, vault_id, name, kind, pages, status, added_label)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![meta.id, vault, meta.name, meta.kind, meta.pages as i64, meta.status, meta.added_label],
        )
        .map_err(sql)?;
        for c in chunks {
            tx.execute(
                "INSERT INTO chunks (id, vault_id, doc_id, doc_name, text, vector)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![c.id, vault, c.doc_id, c.doc_name, c.text, vec_to_blob(&c.vector)],
            )
            .map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        Ok(())
    }

    pub fn remove_document(&self, vault: &str, doc_id: &str) -> AppResult<()> {
        let conn = self.lock();
        conn.execute("DELETE FROM documents WHERE id = ?1 AND vault_id = ?2", params![doc_id, vault])
            .map_err(sql)?;
        conn.execute("DELETE FROM chunks WHERE doc_id = ?1 AND vault_id = ?2", params![doc_id, vault])
            .map_err(sql)?;
        Ok(())
    }

    /// Load all chunks (with parsed vectors) for a vault — used by retrieval
    /// and by entity extraction.
    pub fn load_chunks(&self, vault: &str) -> AppResult<Vec<Chunk>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, doc_id, doc_name, text, vector FROM chunks WHERE vault_id = ?1")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![vault], |r| {
                Ok(Chunk {
                    id: r.get(0)?,
                    doc_id: r.get(1)?,
                    doc_name: r.get(2)?,
                    text: r.get(3)?,
                    vector: read_vector(r.get_ref(4)?),
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Look up a document by its (content-hash) id within a vault.
    pub fn get_document(&self, vault: &str, id: &str) -> AppResult<Option<DocMeta>> {
        let conn = self.lock();
        let row = conn
            .query_row(
                "SELECT id, name, kind, pages, status, added_label FROM documents WHERE id = ?1 AND vault_id = ?2",
                params![id, vault],
                |r| {
                    Ok(DocMeta {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        kind: r.get(2)?,
                        pages: r.get::<_, i64>(3)? as usize,
                        status: r.get(4)?,
                        added_label: r.get(5)?,
                    })
                },
            )
            .ok();
        Ok(row)
    }

    /// Overwrite the stored vectors after a re-embed (reindex). Ids are chunk ids.
    pub fn update_chunk_vectors(&self, vault: &str, vecs: &[(String, Vec<f32>)]) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(sql)?;
        for (id, v) in vecs {
            tx.execute(
                "UPDATE chunks SET vector = ?3 WHERE id = ?1 AND vault_id = ?2",
                params![id, vault, vec_to_blob(v)],
            )
            .map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        Ok(())
    }

    /// The embedding "provider/model" a vault was last indexed with (meta table).
    pub fn indexed_embedding(&self, vault: &str) -> AppResult<Option<String>> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![format!("emb:{vault}")],
                |r| r.get::<_, String>(0),
            )
            .ok())
    }

    pub fn set_indexed_embedding(&self, vault: &str, tag: &str) -> AppResult<()> {
        self.lock()
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = ?2",
                params![format!("emb:{vault}"), tag],
            )
            .map_err(sql)?;
        Ok(())
    }

    // --- Entities ---

    pub fn entities(&self, vault: &str) -> AppResult<Entities> {
        Ok(Entities {
            characters: self.list_characters(vault)?,
            places: self.list_places(vault)?,
            relations: self.list_relations(vault)?,
        })
    }

    pub fn list_characters(&self, vault: &str) -> AppResult<Vec<Character>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, role, summary, traits, status, source_doc, source_quote FROM characters WHERE vault_id = ?1")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![vault], |r| {
                let traits: String = r.get(4)?;
                Ok(Character {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    role: r.get(2)?,
                    summary: r.get(3)?,
                    traits: serde_json::from_str(&traits).unwrap_or_default(),
                    status: r.get(5)?,
                    source_doc: r.get(6)?,
                    source_quote: r.get(7)?,
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_places(&self, vault: &str) -> AppResult<Vec<Place>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, kind, summary, status, source_doc, source_quote FROM places WHERE vault_id = ?1")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![vault], |r| {
                Ok(Place {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    kind: r.get(2)?,
                    summary: r.get(3)?,
                    status: r.get(4)?,
                    source_doc: r.get(5)?,
                    source_quote: r.get(6)?,
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_relations(&self, vault: &str) -> AppResult<Vec<Relation>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT from_name, to_name, label FROM relations WHERE vault_id = ?1")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![vault], |r| {
                Ok(Relation { from: r.get(0)?, to: r.get(1)?, label: r.get(2)? })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Replace all extracted entities for a vault (extraction is a full refresh).
    pub fn replace_entities(
        &self,
        vault: &str,
        characters: &[Character],
        places: &[Place],
        relations: &[Relation],
    ) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(sql)?;
        for t in ["characters", "places", "relations"] {
            tx.execute(&format!("DELETE FROM {t} WHERE vault_id = ?1"), params![vault]).map_err(sql)?;
        }
        for c in characters {
            let traits = serde_json::to_string(&c.traits)?;
            tx.execute(
                "INSERT INTO characters (id, vault_id, name, role, summary, traits, status, source_doc, source_quote)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![c.id, vault, c.name, c.role, c.summary, traits, c.status, c.source_doc, c.source_quote],
            ).map_err(sql)?;
        }
        for p in places {
            tx.execute(
                "INSERT INTO places (id, vault_id, name, kind, summary, status, source_doc, source_quote)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![p.id, vault, p.name, p.kind, p.summary, p.status, p.source_doc, p.source_quote],
            ).map_err(sql)?;
        }
        for r in relations {
            tx.execute(
                "INSERT INTO relations (id, vault_id, from_name, to_name, label) VALUES (?1,?2,?3,?4,?5)",
                params![Uuid::new_v4().to_string(), vault, r.from, r.to, r.label],
            ).map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        Ok(())
    }

    pub fn update_character(&self, vault: &str, c: &Character) -> AppResult<()> {
        let traits = serde_json::to_string(&c.traits)?;
        self.lock().execute(
            "UPDATE characters SET name=?3, role=?4, summary=?5, traits=?6, status=?7 WHERE id=?1 AND vault_id=?2",
            params![c.id, vault, c.name, c.role, c.summary, traits, c.status],
        ).map_err(sql)?;
        Ok(())
    }

    pub fn update_place(&self, vault: &str, p: &Place) -> AppResult<()> {
        self.lock().execute(
            "UPDATE places SET name=?3, kind=?4, summary=?5, status=?6 WHERE id=?1 AND vault_id=?2",
            params![p.id, vault, p.name, p.kind, p.summary, p.status],
        ).map_err(sql)?;
        Ok(())
    }

    // --- Chat sessions ---

    pub fn list_sessions(&self, vault: &str) -> AppResult<Vec<ChatSession>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, title, created_at, updated_at FROM sessions WHERE vault_id = ?1 ORDER BY updated_at DESC")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![vault], |r| {
                Ok(ChatSession {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    created_at: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn create_session(&self, vault: &str, title: &str) -> AppResult<ChatSession> {
        let id = Uuid::new_v4().to_string();
        let conn = self.lock();
        conn.execute(
            "INSERT INTO sessions (id, vault_id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'), datetime('now'))",
            params![id, vault, title],
        )
        .map_err(sql)?;
        let (created_at, updated_at): (String, String) = conn
            .query_row("SELECT created_at, updated_at FROM sessions WHERE id = ?1", params![id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(sql)?;
        Ok(ChatSession { id, title: title.to_string(), created_at, updated_at })
    }

    pub fn rename_session(&self, id: &str, title: &str) -> AppResult<()> {
        self.lock()
            .execute("UPDATE sessions SET title = ?2 WHERE id = ?1", params![id, title])
            .map_err(sql)?;
        Ok(())
    }

    pub fn delete_session(&self, id: &str) -> AppResult<()> {
        let conn = self.lock();
        conn.execute("DELETE FROM messages WHERE session_id = ?1", params![id]).map_err(sql)?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]).map_err(sql)?;
        Ok(())
    }

    pub fn session_messages(&self, session: &str) -> AppResult<Vec<StoredMessage>> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, role, text, thinking, sources FROM messages WHERE session_id = ?1 ORDER BY ordinal ASC")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![session], |r| {
                let sources: String = r.get::<_, Option<String>>(4)?.unwrap_or_default();
                Ok(StoredMessage {
                    id: r.get(0)?,
                    role: r.get(1)?,
                    text: r.get(2)?,
                    thinking: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    sources: serde_json::from_str(&sources).unwrap_or(serde_json::Value::Array(vec![])),
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Append a message to a session and bump the session's updated_at.
    pub fn add_message(
        &self,
        vault: &str,
        session: &str,
        role: &str,
        text: &str,
        thinking: &str,
        sources: &serde_json::Value,
    ) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(sql)?;
        let ord: i64 = tx
            .query_row("SELECT COUNT(*) FROM messages WHERE session_id = ?1", params![session], |r| r.get(0))
            .unwrap_or(0);
        let sources_json = serde_json::to_string(sources).unwrap_or_else(|_| "[]".into());
        tx.execute(
            "INSERT INTO messages (id, session_id, vault_id, role, text, thinking, sources, ordinal, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            params![Uuid::new_v4().to_string(), session, vault, role, text, thinking, sources_json, ord],
        )
        .map_err(sql)?;
        tx.execute("UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1", params![session])
            .map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(())
    }
}

fn sql(e: rusqlite::Error) -> AppError {
    AppError::Msg(format!("erro no banco: {e}"))
}

/// Vectors are stored as a compact little-endian f32 BLOB (4 bytes/dim) instead
/// of a JSON string — smaller on disk and much faster to parse than JSON.
fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 4);
    for f in v {
        b.extend_from_slice(&f.to_le_bytes());
    }
    b
}

fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Read a vector column, accepting both the new BLOB format and the legacy
/// JSON-text format (so databases written before the change still load).
fn read_vector(v: rusqlite::types::ValueRef<'_>) -> Vec<f32> {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Blob(b) => blob_to_vec(b),
        ValueRef::Text(t) => std::str::from_utf8(t)
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}
