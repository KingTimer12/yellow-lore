use crate::config::RagConfig;
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};

pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

/// Embed a batch of texts with the configured embedding provider/model.
pub async fn embed(
    client: &reqwest::Client,
    cfg: &RagConfig,
    inputs: &[String],
) -> AppResult<Vec<Vec<f32>>> {
    match cfg.embedding_provider.as_str() {
        "openai" => openai_embed(client, cfg, inputs).await,
        "ollama" => ollama_embed(client, cfg, inputs).await,
        other => Err(AppError::Provider(format!(
            "provedor de embedding desconhecido: {other}"
        ))),
    }
}

/// Generate a chat completion with the configured LLM provider/model.
pub async fn chat(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
) -> AppResult<String> {
    match cfg.llm_provider.as_str() {
        "openai" => openai_chat(client, cfg, messages).await,
        "ollama" => ollama_chat(client, cfg, messages).await,
        other => Err(AppError::Provider(format!(
            "provedor de LLM desconhecido: {other}"
        ))),
    }
}

// ---- OpenAI ---------------------------------------------------------------

async fn openai_embed(
    client: &reqwest::Client,
    cfg: &RagConfig,
    inputs: &[String],
) -> AppResult<Vec<Vec<f32>>> {
    if cfg.openai_api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key da OpenAI não configurada".into()));
    }
    let url = format!("{}/embeddings", cfg.openai_base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .bearer_auth(&cfg.openai_api_key)
        .json(&json!({ "model": cfg.embedding_model, "input": inputs }))
        .send()
        .await?;
    let body: Value = check(resp).await?;
    let data = body["data"]
        .as_array()
        .ok_or_else(|| AppError::Provider("resposta de embedding inválida".into()))?;
    Ok(data.iter().map(|d| to_vec_f32(&d["embedding"])).collect())
}

async fn openai_chat(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
) -> AppResult<String> {
    if cfg.openai_api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key da OpenAI não configurada".into()));
    }
    let url = format!("{}/chat/completions", cfg.openai_base_url.trim_end_matches('/'));
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let resp = client
        .post(url)
        .bearer_auth(&cfg.openai_api_key)
        .json(&json!({ "model": cfg.llm_model, "messages": msgs }))
        .send()
        .await?;
    let body: Value = check(resp).await?;
    Ok(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

// ---- Ollama (local) -------------------------------------------------------

async fn ollama_embed(
    client: &reqwest::Client,
    cfg: &RagConfig,
    inputs: &[String],
) -> AppResult<Vec<Vec<f32>>> {
    let base = cfg.ollama_endpoint.trim_end_matches('/');
    let url = format!("{base}/api/embed");
    let resp = client
        .post(&url)
        .json(&json!({ "model": cfg.embedding_model, "input": inputs }))
        .send()
        .await?;
    let body: Value = check(resp).await?;
    let arr = body["embeddings"]
        .as_array()
        .ok_or_else(|| AppError::Provider("resposta de embedding do Ollama inválida".into()))?;
    Ok(arr.iter().map(to_vec_f32).collect())
}

async fn ollama_chat(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
) -> AppResult<String> {
    let base = cfg.ollama_endpoint.trim_end_matches('/');
    let url = format!("{base}/api/chat");
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let resp = client
        .post(&url)
        .json(&json!({ "model": cfg.llm_model, "messages": msgs, "stream": false }))
        .send()
        .await?;
    let body: Value = check(resp).await?;
    Ok(body["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

// ---- helpers --------------------------------------------------------------

async fn check(resp: reqwest::Response) -> AppResult<Value> {
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(AppError::Provider(format!("{status}: {text}")));
    }
    serde_json::from_str(&text).map_err(AppError::from)
}

fn to_vec_f32(v: &Value) -> Vec<f32> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_f64().map(|n| n as f32)).collect())
        .unwrap_or_default()
}
