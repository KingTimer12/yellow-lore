use serde::{Deserialize, Serialize};

/// One embedded chunk of a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub doc_id: String,
    pub doc_name: String,
    /// 0-based position of this chunk within its own document (reading order).
    /// Not persisted as its own column — recomputed on load from rowid order.
    #[serde(default)]
    pub ordinal: usize,
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

/// Lexical fallback ranking (IDF-weighted). Complements semantic search: when
/// the answer hinges on a rare literal word (a name, a place, "meteorito") that
/// the embedding model under-ranks, this still surfaces the chunk.
///
/// Terms are weighted by inverse document frequency, so a rare, discriminating
/// word ("meteorito", present in one chunk) outweighs common query words
/// ("país", "caiu", spread across many). Chunks are ranked FIRST by the single
/// rarest query term they contain, then by the summed weight — guaranteeing the
/// chunk holding the most distinctive term wins, even against chunks that stack
/// several common matches. Returns top-k with at least one match.
pub fn keyword_search(chunks: &[Chunk], question: &str, k: usize) -> Vec<ScoredChunk> {
    let terms = query_terms(question);
    if terms.is_empty() || chunks.is_empty() {
        return Vec::new();
    }
    let n = chunks.len() as f32;
    let lowers: Vec<String> = chunks.iter().map(|c| c.text.to_lowercase()).collect();

    // Document frequency, then smoothed IDF per term.
    let idf: Vec<f32> = terms
        .iter()
        .map(|t| {
            let df = lowers.iter().filter(|lc| lc.contains(t.as_str())).count();
            ((n + 1.0) / (df as f32 + 1.0)).ln().max(0.0)
        })
        .collect();

    let mut scored: Vec<(f32, f32, ScoredChunk)> = chunks
        .iter()
        .zip(&lowers)
        .filter_map(|(c, lower)| {
            let mut max_idf = 0.0f32;
            let mut sum_idf = 0.0f32;
            for (t, &w) in terms.iter().zip(&idf) {
                if w > 0.0 && lower.contains(t.as_str()) {
                    sum_idf += w;
                    if w > max_idf {
                        max_idf = w;
                    }
                }
            }
            if sum_idf <= 0.0 {
                None
            } else {
                Some((max_idf, sum_idf, ScoredChunk { chunk: c.clone(), score: sum_idf }))
            }
        })
        .collect();

    // Rank by rarest matched term first, then total weight.
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    scored.truncate(k);
    scored.into_iter().map(|(_, _, sc)| sc).collect()
}

/// Meaningful query terms: lowercased, length ≥ 4, minus common Portuguese
/// question/stop words. Used by [`keyword_search`].
fn query_terms(question: &str) -> Vec<String> {
    const STOP: [&str; 34] = [
        "onde", "qual", "quais", "quem", "como", "quando", "porque", "para", "pelo",
        "pela", "pelos", "pelas", "dos", "das", "com", "sem", "que", "uma", "uns",
        "umas", "essa", "esse", "esses", "essas", "isso", "aquilo", "sobre", "esta",
        "este", "estes", "estas", "seu", "sua", "seus",
    ];
    question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4 && !STOP.contains(w))
        .map(|w| w.to_string())
        .collect()
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
