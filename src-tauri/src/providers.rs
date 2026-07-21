use crate::config::RagConfig;
use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

/// Max inputs sent per embedding request. Large documents are split into
/// several requests so a single payload never gets huge (and to stay under
/// provider per-request input caps).
const EMBED_BATCH: usize = 96;

/// Retry policy for transient failures (rate limits, 5xx). Quota/auth errors
/// are NOT retried — they won't succeed on a retry.
const MAX_RETRIES: u32 = 3;

/// Embed texts with the configured embedding provider/model. Inputs are chunked
/// into batches of `EMBED_BATCH` and the resulting vectors concatenated in order.
pub async fn embed(
    client: &reqwest::Client,
    cfg: &RagConfig,
    inputs: &[String],
) -> AppResult<Vec<Vec<f32>>> {
    let mut out = Vec::with_capacity(inputs.len());
    for batch in inputs.chunks(EMBED_BATCH) {
        let vecs = match cfg.embedding_provider.as_str() {
            "openai" => {
                oai_embed(client, &cfg.openai_base_url, &cfg.openai_api_key, &cfg.embedding_model, batch, true).await
            }
            "vllm" => {
                oai_embed(client, &cfg.vllm_base_url, &cfg.vllm_api_key, &cfg.embedding_model, batch, false).await
            }
            "ollama" => ollama_embed(client, cfg, batch).await,
            other => Err(AppError::Provider(format!(
                "provedor de embedding desconhecido: {other}"
            ))),
        }?;
        out.extend(vecs);
    }
    Ok(out)
}

/// Generate a chat completion with the configured LLM provider/model.
pub async fn chat(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
) -> AppResult<String> {
    match cfg.llm_provider.as_str() {
        "openai" => {
            oai_chat(client, &cfg.openai_base_url, &cfg.openai_api_key, &cfg.llm_model, messages, true, cfg.temperature).await
        }
        "vllm" => {
            oai_chat(client, &cfg.vllm_base_url, &cfg.vllm_api_key, &cfg.llm_model, messages, false, cfg.temperature).await
        }
        "ollama" => ollama_chat(client, cfg, messages).await,
        other => Err(AppError::Provider(format!(
            "provedor de LLM desconhecido: {other}"
        ))),
    }
}

// ---- OpenAI-compatible (OpenAI + vLLM) ------------------------------------
//
// vLLM serves the same `/v1/chat/completions` and `/v1/embeddings` schema as
// OpenAI; only the base URL differs and the API key is optional. `key_required`
// distinguishes OpenAI (needs a key) from a local vLLM (key optional).

async fn oai_embed(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    inputs: &[String],
    key_required: bool,
) -> AppResult<Vec<Vec<f32>>> {
    if key_required && api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key da OpenAI não configurada".into()));
    }
    let url = format!("{}/embeddings", base_url.trim_end_matches('/'));
    let mut req = client.post(url).json(&json!({ "model": model, "input": inputs }));
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
    }
    let body: Value = post_json(req).await?;
    let data = body["data"]
        .as_array()
        .ok_or_else(|| AppError::Provider("resposta de embedding inválida".into()))?;
    Ok(data.iter().map(|d| to_vec_f32(&d["embedding"])).collect())
}

async fn oai_chat(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &[ChatMessage],
    key_required: bool,
    temperature: f32,
) -> AppResult<String> {
    if key_required && api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key da OpenAI não configurada".into()));
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut req = client
        .post(url)
        .json(&json!({ "model": model, "messages": msgs, "temperature": temperature }));
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
    }
    let body: Value = post_json(req).await?;
    Ok(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

// ---- Ollama (local) -------------------------------------------------------

/// Shared Ollama generation options. `num_ctx` is only sent when configured
/// (> 0); otherwise Ollama uses the model's own default.
fn ollama_options(cfg: &RagConfig) -> Value {
    let mut opts = json!({ "temperature": cfg.temperature });
    if cfg.ollama_num_ctx > 0 {
        opts["num_ctx"] = json!(cfg.ollama_num_ctx);
    }
    opts
}

async fn ollama_embed(
    client: &reqwest::Client,
    cfg: &RagConfig,
    inputs: &[String],
) -> AppResult<Vec<Vec<f32>>> {
    let base = cfg.ollama_endpoint.trim_end_matches('/');
    let url = format!("{base}/api/embed");
    let req = client
        .post(&url)
        .json(&json!({ "model": cfg.embedding_model, "input": inputs }));
    let body: Value = post_json(req).await?;
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
    let req = client.post(&url).json(&json!({
        "model": cfg.llm_model,
        "messages": msgs,
        "stream": false,
        "options": ollama_options(cfg),
    }));
    let body: Value = post_json(req).await?;
    let content = body["message"]["content"].as_str().unwrap_or_default();
    let thinking = body["message"]["thinking"].as_str().unwrap_or_default();
    // Fold any reasoning into a <think> block so callers that strip it (title /
    // extraction) behave the same for thinking and non-thinking models.
    if thinking.is_empty() {
        Ok(content.to_string())
    } else {
        Ok(format!("<think>{thinking}</think>{content}"))
    }
}

// ---- helpers --------------------------------------------------------------

/// Send a request and parse the JSON body, retrying transient failures (HTTP
/// 429 rate limits and 5xx) with exponential backoff. Quota-exhausted and auth
/// errors are surfaced immediately with a friendly, actionable message.
async fn post_json(req: reqwest::RequestBuilder) -> AppResult<Value> {
    let mut attempt: u32 = 0;
    loop {
        // Rebuild the request per attempt; a body stream can't be reused.
        let this = req
            .try_clone()
            .ok_or_else(|| AppError::Provider("não foi possível clonar a requisição".into()))?;
        let resp = this.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if status.is_success() {
            return serde_json::from_str(&text).map_err(AppError::from);
        }

        let code = status.as_u16();

        // Retry rate limits (non-quota) and server errors with backoff.
        let retryable = is_retryable(code, &text);
        if retryable && attempt < MAX_RETRIES {
            let backoff = Duration::from_millis(500u64 * 2u64.pow(attempt));
            tokio::time::sleep(backoff).await;
            attempt += 1;
            continue;
        }
        return Err(provider_error(code, &text, attempt + 1));
    }
}

/// Whether an HTTP failure is worth retrying: non-quota 429s and 5xx.
fn is_retryable(code: u16, body: &str) -> bool {
    let low = body.to_lowercase();
    let quota = low.contains("insufficient_quota") || low.contains("exceeded your current quota");
    (code == 429 && !quota) || (500..=599).contains(&code)
}

/// Map an HTTP error into a friendly, actionable message.
fn provider_error(code: u16, text: &str, attempts: u32) -> AppError {
    let low = text.to_lowercase();
    if code == 429 && (low.contains("insufficient_quota") || low.contains("exceeded your current quota")) {
        return AppError::Provider(
            "cota da API esgotada — verifique créditos/billing do provedor, \
             ou use o Ollama local (sem cota) nas Configurações."
                .into(),
        );
    }
    if code == 401 || code == 403 {
        return AppError::Provider(
            "credenciais rejeitadas (401/403) — confira a API key nas Configurações.".into(),
        );
    }
    if code == 429 {
        return AppError::Provider(format!(
            "limite de requisições atingido (rate limit) após {attempts} tentativas — \
             aguarde alguns instantes e tente de novo."
        ));
    }
    AppError::Provider(format!("{code}: {text}"))
}

// ---- Streaming chat -------------------------------------------------------

/// Stream a chat completion, invoking `on_token` for each text delta as it
/// arrives. Returns the full concatenated text once the stream ends.
pub async fn chat_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
    cancel: &AtomicBool,
    on_token: F,
) -> AppResult<String> {
    match cfg.llm_provider.as_str() {
        "openai" => {
            oai_chat_stream(client, &cfg.openai_base_url, &cfg.openai_api_key, &cfg.llm_model, messages, true, cfg.temperature, cancel, on_token).await
        }
        "vllm" => {
            oai_chat_stream(client, &cfg.vllm_base_url, &cfg.vllm_api_key, &cfg.llm_model, messages, false, cfg.temperature, cancel, on_token).await
        }
        "ollama" => ollama_chat_stream(client, cfg, messages, cancel, on_token).await,
        other => Err(AppError::Provider(format!(
            "provedor de LLM desconhecido: {other}"
        ))),
    }
}

async fn stream_status_guard(resp: reqwest::Response) -> AppResult<reqwest::Response> {
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(provider_error(status.as_u16(), &text, 1));
    }
    Ok(resp)
}

/// OpenAI / vLLM server-sent-events: lines `data: {json}` with the delta at
/// `choices[0].delta.content`, terminated by `data: [DONE]`.
async fn oai_chat_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &[ChatMessage],
    key_required: bool,
    temperature: f32,
    cancel: &AtomicBool,
    mut on_token: F,
) -> AppResult<String> {
    if key_required && api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key da OpenAI não configurada".into()));
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut req = client
        .post(url)
        .json(&json!({ "model": model, "messages": msgs, "stream": true, "temperature": temperature }));
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = stream_status_guard(req.send().await?).await?;

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Ok(full);
        }
        buf.push_str(&String::from_utf8_lossy(&chunk?));
        // Process complete lines; keep the trailing partial in `buf`.
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            buf.drain(..=nl);
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" {
                return Ok(full);
            }
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if let Some(tok) = v["choices"][0]["delta"]["content"].as_str() {
                    if !tok.is_empty() {
                        full.push_str(tok);
                        on_token(tok);
                    }
                }
            }
        }
    }
    Ok(full)
}

/// Ollama streams newline-delimited JSON objects, each with
/// `message.content` and a final `done: true`.
async fn ollama_chat_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
    cancel: &AtomicBool,
    mut on_token: F,
) -> AppResult<String> {
    let base = cfg.ollama_endpoint.trim_end_matches('/');
    let url = format!("{base}/api/chat");
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let req = client.post(&url).json(&json!({
        "model": cfg.llm_model,
        "messages": msgs,
        "stream": true,
        "options": ollama_options(cfg),
    }));
    let resp = stream_status_guard(req.send().await?).await?;

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();
    // Thinking-capable models (e.g. deepseek-r1, qwen3, some gemma builds) stream
    // their reasoning in a separate `message.thinking` field while `content`
    // stays empty. Surface it wrapped in <think>…</think> so the UI shows the
    // reasoning instead of hanging on the loading dots.
    let mut in_think = false;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            if in_think {
                full.push_str("</think>");
                on_token("</think>");
            }
            return Ok(full);
        }
        buf.push_str(&String::from_utf8_lossy(&chunk?));
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            buf.drain(..=nl);
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if let Some(th) = v["message"]["thinking"].as_str() {
                    if !th.is_empty() {
                        if !in_think {
                            full.push_str("<think>");
                            on_token("<think>");
                            in_think = true;
                        }
                        full.push_str(th);
                        on_token(th);
                    }
                }
                if let Some(tok) = v["message"]["content"].as_str() {
                    if !tok.is_empty() {
                        if in_think {
                            full.push_str("</think>");
                            on_token("</think>");
                            in_think = false;
                        }
                        full.push_str(tok);
                        on_token(tok);
                    }
                }
                if v["done"].as_bool().unwrap_or(false) {
                    if in_think {
                        full.push_str("</think>");
                        on_token("</think>");
                    }
                    return Ok(full);
                }
            }
        }
    }
    if in_think {
        on_token("</think>");
    }
    Ok(full)
}

fn to_vec_f32(v: &Value) -> Vec<f32> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_f64().map(|n| n as f32)).collect())
        .unwrap_or_default()
}
