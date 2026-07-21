use serde::{Deserialize, Serialize};

/// One embedded chunk of a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub doc_id: String,
    pub doc_name: String,
    pub text: String,
    pub vector: Vec<f32>,
}

/// A retrieval hit.
#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub chunk: Chunk,
    pub score: f32,
}

/// Top-k chunks by cosine similarity. Brute-force over the active vault's
/// chunks — more than fast enough for a personal knowledge base.
pub fn search(chunks: &[Chunk], query: &[f32], k: usize) -> Vec<ScoredChunk> {
    let mut scored: Vec<ScoredChunk> = chunks
        .iter()
        .map(|c| ScoredChunk {
            chunk: c.clone(),
            score: cosine(query, &c.vector),
        })
        .collect();
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}
