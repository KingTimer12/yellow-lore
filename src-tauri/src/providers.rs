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
            "gemini" => {
                oai_embed(client, &cfg.gemini_base_url, &cfg.gemini_api_key, &cfg.embedding_model, batch, true).await
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

/// Generate a chat completion for the FINAL, user-facing answer. Reasoning is
/// gated by `cfg.show_thinking` (off by default).
pub async fn chat(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
) -> AppResult<String> {
    chat_impl(client, cfg, messages, &cfg.llm_model, cfg.show_thinking).await
}

/// Generate a chat completion for an INTERNAL RAG step (rerank, grading, dedup,
/// entity extraction, title). Reasoning is DISABLED to cut latency — these steps
/// return short structured output where deliberation only wastes tokens/time.
/// `model` allows a dedicated extraction model. On Ollama this sends
/// `think: false`, on Gemini `reasoning_effort: "none"`; OpenAI/vLLM have no
/// standard toggle, so the prompts also carry a `/no_think` hint for models that
/// honor it.
pub async fn chat_internal(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
    model: &str,
) -> AppResult<String> {
    chat_impl(client, cfg, messages, model, false).await
}

async fn chat_impl(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
    model: &str,
    think: bool,
) -> AppResult<String> {
    match cfg.llm_provider.as_str() {
        "openai" => {
            oai_chat(client, &cfg.openai_base_url, &cfg.openai_api_key, model, messages, true, cfg.temperature, None).await
        }
        "vllm" => {
            oai_chat(client, &cfg.vllm_base_url, &cfg.vllm_api_key, model, messages, false, cfg.temperature, None).await
        }
        "gemini" => {
            oai_chat(client, &cfg.gemini_base_url, &cfg.gemini_api_key, model, messages, true, cfg.temperature, gemini_reasoning(think)).await
        }
        "ollama" => ollama_chat(client, cfg, messages, model, think).await,
        other => Err(AppError::Provider(format!(
            "provedor de LLM desconhecido: {other}"
        ))),
    }
}

// ---- OpenAI-compatible (OpenAI + vLLM + Gemini) ---------------------------
//
// vLLM and Gemini both serve the same `/chat/completions` and `/embeddings`
// schema as OpenAI; only the base URL differs (and vLLM's key is optional), so
// all three share these functions. `key_required` distinguishes a hosted API
// (needs a key) from a local vLLM.

/// Gemini 2.5 models reason by default and there is no `think` flag in the
/// compatibility layer — `reasoning_effort: "none"` is how you turn it off.
/// Omitted when reasoning is wanted so the model keeps its own default budget.
fn gemini_reasoning(think: bool) -> Option<&'static str> {
    if think { None } else { Some("none") }
}

async fn oai_embed(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    inputs: &[String],
    key_required: bool,
) -> AppResult<Vec<Vec<f32>>> {
    if key_required && api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key do provedor não configurada".into()));
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
    reasoning_effort: Option<&str>,
) -> AppResult<String> {
    if key_required && api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key do provedor não configurada".into()));
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut body = json!({ "model": model, "messages": msgs, "temperature": temperature });
    if let Some(effort) = reasoning_effort {
        body["reasoning_effort"] = json!(effort);
    }
    let mut req = client.post(url).json(&body);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
    }
    let body: Value = post_json(req).await?;
    let msg = &body["choices"][0]["message"];
    let content = msg["content"].as_str().unwrap_or_default();
    let thinking = msg["reasoning_content"].as_str().unwrap_or_default();
    if thinking.is_empty() {
        Ok(content.to_string())
    } else {
        Ok(format!("<think>{thinking}</think>{content}"))
    }
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
    model: &str,
    think: bool,
) -> AppResult<String> {
    let base = cfg.ollama_endpoint.trim_end_matches('/');
    let url = format!("{base}/api/chat");
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut body = json!({
        "model": model,
        "messages": msgs,
        "stream": false,
        "options": ollama_options(cfg),
    });
    // Only send `think` when disabling it; omit otherwise so non-thinking models
    // (which reject the field on some Ollama builds) are unaffected.
    if !think {
        body["think"] = json!(false);
    }
    let req = client.post(&url).json(&body);
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
    (code == 429 && !is_quota(&low)) || (500..=599).contains(&code)
}

/// Map an HTTP error into a friendly, actionable message.
/// Quota exhaustion, across provider dialects. Gemini answers 429 with
/// `RESOURCE_EXHAUSTED` for BOTH a spent quota and a plain per-minute rate limit,
/// so that code alone can't decide — only the explicit quota wording counts,
/// leaving rate limits retryable.
fn is_quota(low: &str) -> bool {
    low.contains("insufficient_quota") || low.contains("exceeded your current quota")
}

fn provider_error(code: u16, text: &str, attempts: u32) -> AppError {
    let low = text.to_lowercase();
    if code == 429 && is_quota(&low) {
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
    think: bool,
    cancel: &AtomicBool,
    on_token: F,
) -> AppResult<String> {
    match cfg.llm_provider.as_str() {
        "openai" => {
            oai_chat_stream(client, &cfg.openai_base_url, &cfg.openai_api_key, &cfg.llm_model, messages, true, cfg.temperature, None, cancel, on_token).await
        }
        "vllm" => {
            oai_chat_stream(client, &cfg.vllm_base_url, &cfg.vllm_api_key, &cfg.llm_model, messages, false, cfg.temperature, None, cancel, on_token).await
        }
        "gemini" => {
            oai_chat_stream(client, &cfg.gemini_base_url, &cfg.gemini_api_key, &cfg.llm_model, messages, true, cfg.temperature, gemini_reasoning(think), cancel, on_token).await
        }
        "ollama" => ollama_chat_stream(client, cfg, messages, think, cancel, on_token).await,
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
    reasoning_effort: Option<&str>,
    cancel: &AtomicBool,
    mut on_token: F,
) -> AppResult<String> {
    if key_required && api_key.trim().is_empty() {
        return Err(AppError::Provider("API Key do provedor não configurada".into()));
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut body =
        json!({ "model": model, "messages": msgs, "stream": true, "temperature": temperature });
    if let Some(effort) = reasoning_effort {
        body["reasoning_effort"] = json!(effort);
    }
    let mut req = client.post(url).json(&body);
    if !api_key.trim().is_empty() {
        req = req.bearer_auth(api_key);
    }
    let resp = stream_status_guard(req.send().await?).await?;

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();
    // Reasoning models behind an OpenAI-compatible façade (vLLM/deepseek, Gemini)
    // put their reasoning in `delta.reasoning_content` while `content` stays
    // empty. Wrap it in <think>…</think> so the UI renders it in the collapsible
    // block instead of sitting on the loading dots — same shape as Ollama.
    let mut in_think = false;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            if in_think {
                on_token("</think>");
                full.push_str("</think>");
            }
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
                if in_think {
                    on_token("</think>");
                    full.push_str("</think>");
                }
                return Ok(full);
            }
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                let delta = &v["choices"][0]["delta"];
                if let Some(th) = delta["reasoning_content"].as_str() {
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
                if let Some(tok) = delta["content"].as_str() {
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
            }
        }
    }
    if in_think {
        on_token("</think>");
        full.push_str("</think>");
    }
    Ok(full)
}

/// Ollama streams newline-delimited JSON objects, each with
/// `message.content` and a final `done: true`.
async fn ollama_chat_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    cfg: &RagConfig,
    messages: &[ChatMessage],
    think: bool,
    cancel: &AtomicBool,
    mut on_token: F,
) -> AppResult<String> {
    let base = cfg.ollama_endpoint.trim_end_matches('/');
    let url = format!("{base}/api/chat");
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut body = json!({
        "model": cfg.llm_model,
        "messages": msgs,
        "stream": true,
        "options": ollama_options(cfg),
    });
    if !think {
        body["think"] = json!(false);
    }
    let req = client.post(&url).json(&body);
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
