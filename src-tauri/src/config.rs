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
    /// Ollama context window (num_ctx). Reasoning models emit long <think> blocks;
    /// with the default (~4096) the prompt + reasoning overflow and generation is
    /// cut off before the answer. Raise for reasoning models, lower to save RAM.
    /// 0 = don't send (use the model's own default).
    #[serde(default = "default_num_ctx")]
    pub ollama_num_ctx: u32,
    /// vLLM OpenAI-compatible server (e.g. http://localhost:8000/v1). Key optional.
    #[serde(default = "default_vllm_base_url")]
    pub vllm_base_url: String,
    #[serde(default)]
    pub vllm_api_key: String,
    /// Google Gemini, consumed through its OpenAI-compatible endpoint so it reuses
    /// the same chat/embeddings/SSE code paths as OpenAI.
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default = "default_gemini_base_url")]
    pub gemini_base_url: String,

    // --- Agent behaviour ---
    /// System prompt the user can tune to steer answers.
    pub system_prompt: String,

    // --- RAG knobs (fully user-configurable) ---
    pub chunk_size: usize,    // approx tokens per chunk
    pub chunk_overlap: usize, // approx tokens of overlap
    pub top_k: usize,         // retrieved chunks per query
    /// Sampling temperature for generation. Low (≈0.2) keeps answers faithful to
    /// the retrieved context; higher values invite the model to embellish/guess.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    pub show_sources: bool,

    /// Run an extra LLM pass after extraction to merge entities that refer to the
    /// same person/place (nicknames, titles) beyond the name-substring heuristic.
    #[serde(default = "default_true")]
    pub dedup_entities: bool,

    /// Send each candidate's one-line summary and direct relations to the dedup
    /// pass, not just the bare name. Catches duplicates whose names share nothing
    /// ("Elisa" vs an unnamed role) — but multiplies that call's payload by roughly
    /// ten and, because merging the wrong two people is much worse than leaving a
    /// duplicate, it is OFF by default.
    #[serde(default)]
    pub dedup_with_context: bool,

    /// Optional model used ONLY for entity extraction. Empty = reuse `llm_model`
    /// (no second model to download or hold in VRAM — safe for weak GPUs).
    /// Point it at a smaller/faster model when you have the memory to spare.
    #[serde(default)]
    pub extraction_model: String,
    /// How many extraction windows to run concurrently. 1 = sequential (the safe
    /// default for a single local GPU — Ollama serializes anyway and concurrent
    /// requests only thrash VRAM). Raise it for cloud providers (OpenAI/vLLM),
    /// where parallel calls cut wall-clock time roughly linearly.
    #[serde(default = "default_extraction_concurrency")]
    pub extraction_concurrency: usize,
    /// After hybrid retrieval, run one cheap LLM pass to re-order the retrieved
    /// chunks by relevance before building the context (trims top-k noise).
    /// Costs one extra LLM call per question; off by default.
    #[serde(default)]
    pub rerank: bool,
    /// Corrective RAG (CRAG-limited): after drafting an answer, the model grades
    /// whether it actually resolves the question; if not, it re-retrieves with a
    /// wider net and answers once more. Bounded to a single retry (no open agent
    /// loop). Higher precision at the cost of extra LLM calls; off by default.
    #[serde(default)]
    pub corrective: bool,
    /// Let the final answer "think" (emit a reasoning block). Off by default:
    /// reasoning is unnecessary for these summarization/lookup answers and, with
    /// models that reason in plain text, leaks a long preamble into the reply.
    /// Internal RAG steps never think regardless of this.
    #[serde(default)]
    pub show_thinking: bool,

    /// Cache provider responses on disk, keyed by the exact request. Free tiers
    /// meter requests per minute/day, and this app repeats identical calls often
    /// (re-extracting a chapter, re-indexing, asking the same question) — a hit
    /// costs no request at all. On by default; turn it off to always hit the model.
    #[serde(default = "default_true")]
    pub cache_llm: bool,

    /// Characters of text per extraction window. Each window is one LLM request, so
    /// this is the main lever on how many requests an extraction spends: doubling it
    /// halves the request count. The default is sized for small local models; cloud
    /// models with large context windows take far more per call, which matters when
    /// the free tier allows only a few requests per day.
    #[serde(default = "default_window_chars")]
    pub extraction_window_chars: usize,
}

fn default_true() -> bool {
    true
}

fn default_temperature() -> f32 {
    0.2
}

fn default_num_ctx() -> u32 {
    8192
}

fn default_vllm_base_url() -> String {
    "http://localhost:8000/v1".into()
}

fn default_gemini_base_url() -> String {
    "https://generativelanguage.googleapis.com/v1beta/openai".into()
}

fn default_extraction_concurrency() -> usize {
    1
}

fn default_window_chars() -> usize {
    12_000
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
            ollama_num_ctx: default_num_ctx(),
            vllm_base_url: default_vllm_base_url(),
            vllm_api_key: String::new(),
            gemini_api_key: String::new(),
            gemini_base_url: default_gemini_base_url(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            chunk_size: 800,
            chunk_overlap: 120,
            top_k: 5,
            temperature: default_temperature(),
            show_sources: true,
            dedup_entities: true,
            dedup_with_context: false,
            extraction_model: String::new(),
            extraction_concurrency: default_extraction_concurrency(),
            rerank: false,
            corrective: false,
            show_thinking: false,
            cache_llm: true,
            extraction_window_chars: default_window_chars(),
        }
    }
}
