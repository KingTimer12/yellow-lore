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
            CREATE INDEX IF NOT EXISTS idx_chunks_vault ON chunks(vault_id);
            CREATE INDEX IF NOT EXISTS idx_docs_vault ON documents(vault_id);
            ",
        )
        .map_err(sql)?;
        let db = Db { conn: Mutex::new(conn) };
        db.ensure_default_vault()?;
        Ok(db)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    // --- Vaults ---

    fn ensure_default_vault(&self) -> AppResult<()> {
        if self.list_vaults()?.is_empty() {
            let v = self.create_vault("Yellow Lore".into())?;
            self.set_active_vault(&v.id)?;
        } else if self.active_vault()?.is_none() {
            let first = self.list_vaults()?.remove(0);
            self.set_active_vault(&first.id)?;
        }
        Ok(())
    }

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
            for t in ["documents", "chunks", "characters", "places", "relations"] {
                conn.execute(&format!("DELETE FROM {t} WHERE vault_id = ?1"), params![id])
                    .map_err(sql)?;
            }
            conn.execute("DELETE FROM vaults WHERE id = ?1", params![id]).map_err(sql)?;
        }
        // Re-establish an active vault if we removed the current one.
        self.ensure_default_vault()?;
        if self.active_vault()?.as_deref() == Some(id) {
            if let Some(first) = self.list_vaults()?.first() {
                self.set_active_vault(&first.id)?;
            }
        }
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
            let vec_json = serde_json::to_string(&c.vector)?;
            tx.execute(
                "INSERT INTO chunks (id, vault_id, doc_id, doc_name, text, vector)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![c.id, vault, c.doc_id, c.doc_name, c.text, vec_json],
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
                let vec_json: String = r.get(4)?;
                Ok(Chunk {
                    id: r.get(0)?,
                    doc_id: r.get(1)?,
                    doc_name: r.get(2)?,
                    text: r.get(3)?,
                    vector: serde_json::from_str(&vec_json).unwrap_or_default(),
                })
            })
            .map_err(sql)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
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
}

fn sql(e: rusqlite::Error) -> AppError {
    AppError::Msg(format!("erro no banco: {e}"))
}
