//! Persistent response cache for provider calls.
//!
//! Free tiers meter *requests*, not just tokens (Gemini's smaller models allow as
//! few as 5 per minute and 20 per day), and this app repeats identical calls a
//! lot: re-extracting the same chapter, re-indexing a document, asking the same
//! question twice. Every repeat is billed against the daily quota for an answer
//! we already had.
//!
//! So the cache is keyed by the exact request — provider, endpoint, model,
//! temperature and the full prompt — which makes a hit byte-identical to what the
//! provider would return. Anything that changes the answer (edited document, new
//! context, different model) changes the key, so a stale hit is not possible; the
//! cache never needs invalidating.
//!
//! It lives in its own SQLite file, apart from the vault database: it is
//! disposable, may grow large, and has no business in a backup of the user's
//! actual work. Every operation is best-effort — a broken cache degrades into
//! extra requests, never into an error.

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static CACHE: OnceLock<Option<Mutex<Connection>>> = OnceLock::new();

/// Open (or create) the cache file. Called once at startup; a failure here just
/// leaves the cache disabled.
pub fn init(path: &Path) {
    CACHE.get_or_init(|| {
        let conn = Connection::open(path).ok()?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS entries (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at TEXT NOT NULL
             );",
        )
        .ok()?;
        Some(Mutex::new(conn))
    });
}

fn conn() -> Option<&'static Mutex<Connection>> {
    CACHE.get().and_then(|c| c.as_ref())
}

/// Hash the parts that fully determine a response. Uses blake3 so the key stays
/// short regardless of how big the prompt is.
pub fn key(kind: &str, parts: &[&str]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(kind.as_bytes());
    for p in parts {
        // Length-prefix each part so ("ab", "c") and ("a", "bc") never collide.
        h.update(&(p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    h.finalize().to_hex().to_string()
}

pub fn get(key: &str) -> Option<String> {
    let guard = conn()?.lock().ok()?;
    guard
        .query_row("SELECT value FROM entries WHERE key = ?1", [key], |r| r.get(0))
        .ok()
}

pub fn put(key: &str, value: &str) {
    let Some(lock) = conn() else { return };
    let Ok(guard) = lock.lock() else { return };
    let _ = guard.execute(
        "INSERT OR REPLACE INTO entries (key, value, created_at) VALUES (?1, ?2, datetime('now'))",
        rusqlite::params![key, value],
    );
}

/// Entry count and total value size in bytes, for the Settings screen.
pub fn stats() -> (i64, i64) {
    let Some(lock) = conn() else { return (0, 0) };
    let Ok(guard) = lock.lock() else { return (0, 0) };
    guard
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(LENGTH(value)), 0) FROM entries",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0))
}

pub fn clear() {
    let Some(lock) = conn() else { return };
    let Ok(guard) = lock.lock() else { return };
    let _ = guard.execute("DELETE FROM entries", []);
    // Hand the freed pages back to the filesystem — the point of clearing is to
    // reclaim the space, not to leave an empty file the same size.
    let _ = guard.execute_batch("VACUUM");
}
