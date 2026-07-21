use serde::{Deserialize, Serialize};

/// RAG + provider configuration. Persisted to `config.json` in the app data
/// dir and editable from the Settings screen. LLM and embedding are configured
/// independently (different model, and possibly different provider).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagConfig {
    // --- LLM (generation) ---
    pub llm_provider: String, // "ollama" | "openai"
    pub llm_model: String,

    // --- Embedding (retrieval) — separate from the LLM ---
    pub embedding_provider: String, // "ollama" | "openai"
    pub embedding_model: String,

    // --- Provider credentials / endpoints ---
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub ollama_endpoint: String,
    /// vLLM OpenAI-compatible server (e.g. http://localhost:8000/v1). Key optional.
    #[serde(default = "default_vllm_base_url")]
    pub vllm_base_url: String,
    #[serde(default)]
    pub vllm_api_key: String,

    // --- Agent behaviour ---
    /// System prompt the user can tune to steer answers.
    pub system_prompt: String,

    // --- RAG knobs (fully user-configurable) ---
    pub chunk_size: usize,    // approx tokens per chunk
    pub chunk_overlap: usize, // approx tokens of overlap
    pub top_k: usize,         // retrieved chunks per query
    pub show_sources: bool,

    /// Run an extra LLM pass after extraction to merge entities that refer to the
    /// same person/place (nicknames, titles) beyond the name-substring heuristic.
    #[serde(default = "default_true")]
    pub dedup_entities: bool,
}

fn default_true() -> bool {
    true
}

fn default_vllm_base_url() -> String {
    "http://localhost:8000/v1".into()
}

pub const DEFAULT_SYSTEM_PROMPT: &str = "Você é o assistente do Yellow Lore. \
Responda SEMPRE com base nos trechos de conhecimento fornecidos no contexto. \
Se a resposta não estiver no contexto, diga que não encontrou nos documentos \
indexados. Cite os fatos de forma objetiva e no idioma da pergunta.";

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            llm_provider: "ollama".into(),
            llm_model: "llama3.1".into(),
            embedding_provider: "ollama".into(),
            embedding_model: "nomic-embed-text".into(),
            openai_api_key: String::new(),
            openai_base_url: "https://api.openai.com/v1".into(),
            ollama_endpoint: "http://localhost:11434".into(),
            vllm_base_url: default_vllm_base_url(),
            vllm_api_key: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            chunk_size: 800,
            chunk_overlap: 120,
            top_k: 5,
            show_sources: true,
            dedup_entities: true,
        }
    }
}
