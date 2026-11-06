//! The prompt upsampler (PUP) client: an OpenAI-compatible `/v1/chat/completions`
//! endpoint — in Kokon's deployment a llama-server sidecar behind the gateway,
//! loaded on demand.
//!
//! This replaces the in-engine LLM (`/sdcpp/v1/text_gen`): the sidecar is the only
//! source of upsampler tokens (decision D-FB), because stock engines carry no LLM
//! at all. A turn here is exactly what `enhancer::request` builds for the engine
//! API — the same system prompt, user text and reference images — so the upsampler
//! writes captions the image model was trained on, whatever engine paints with them.
//!
//! There is deliberately no "use the raw prompt" fallback for a configured VLM: a
//! model trained on upsampler captions degrades measurably on a user's own words,
//! so an unreachable VLM hard-fails the job with an actionable message instead.

use std::time::Duration;

use serde::Deserialize;

use crate::config::VlmEndpoint;
use crate::error::{AppError, ErrorCode};
use crate::sdcpp::TextGenRequest;

/// f32 sampling values get promoted to f64 with representation noise on the wire
/// (0.95 -> 0.949999988079071); round them back to the digits the config actually has.
fn round4(x: f32) -> f64 {
    (x as f64 * 10000.0).round() / 10000.0
}

/// One completed upsampler turn.
#[derive(Debug, Clone, PartialEq)]
pub struct VlmReply {
    pub text: String,
    /// "stop", "length", …
    pub finish_reason: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

pub struct VlmClient {
    base: String,
    model: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl VlmClient {
    pub fn new(endpoint: &VlmEndpoint) -> Self {
        let http = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(3))
            .build()
            .expect("http client");
        let key = endpoint.api_key.trim();
        Self {
            base: endpoint.entrypoint.trim_end_matches('/').to_string(),
            model: endpoint.model.clone(),
            api_key: (!key.is_empty()).then(|| key.to_string()),
            http,
        }
    }

    /// Where requests go: named in the error message.
    pub fn entrypoint(&self) -> &str {
        &self.base
    }

    /// One non-streaming chat turn: the system prompt, the user text and, for
    /// edits and compositions, the reference images shown before it.
    ///
    /// Every error here is a hard job failure (D-FB): the reply carries the
    /// endpoint and the reason, so the user (or the operator) can act on it.
    pub async fn chat(&self, req: &TextGenRequest, timeout: Duration) -> Result<VlmReply, AppError> {
        // The engine API shows the reference images before the prompt; keep the same
        // order in the chat turn.
        let mut parts: Vec<serde_json::Value> = req
            .images
            .iter()
            .map(|img| {
                serde_json::json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:image/png;base64,{img}") },
                })
            })
            .collect();
        parts.push(serde_json::json!({ "type": "text", "text": req.prompt }));
        let messages: Vec<serde_json::Value> = if req.system_prompt.trim().is_empty() {
            vec![serde_json::json!({ "role": "user", "content": parts })]
        } else {
            vec![
                serde_json::json!({ "role": "system", "content": req.system_prompt }),
                serde_json::json!({ "role": "user", "content": parts }),
            ]
        };
        let mut body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "messages": messages,
            "max_tokens": req.max_tokens,
            "temperature": round4(req.sampling.temperature),
            "top_p": round4(req.sampling.top_p),
            "top_k": req.sampling.top_k,
            "presence_penalty": round4(req.sampling.presence_penalty),
        });
        if req.sampling.seed >= 0 {
            body["seed"] = serde_json::json!(req.sampling.seed);
        }

        let mut builder = self
            .http
            .post(format!("{}/v1/chat/completions", self.base))
            .json(&body)
            .timeout(timeout);
        if let Some(key) = &self.api_key {
            builder = builder.header("Authorization", format!("Bearer {key}"));
        }

        let res = match builder.send().await {
            Ok(res) => res,
            Err(e) => {
                let why = if e.is_connect() {
                    "no server is listening"
                } else if e.is_timeout() {
                    "it took too long (a cold load can take a couple of minutes)"
                } else {
                    &e.to_string()
                };
                return Err(Self::down(self, why));
            }
        };
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            let excerpt: String = body.chars().take(200).collect();
            return Err(Self::down(
                self,
                &format!("it answered {status}: {excerpt}"),
            ));
        }
        let parsed: ChatResponse =
            res.json().await.map_err(|e| Self::down(self, &format!("its reply was unparseable ({e})")))?;
        let choice = parsed.choices.into_iter().next().ok_or_else(|| {
            Self::down(self, "its reply carried no answer")
        })?;
        Ok(VlmReply {
            text: choice.message.content_string(),
            finish_reason: choice.finish_reason.unwrap_or_default(),
            prompt_tokens: parsed.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            completion_tokens: parsed.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
        })
    }

    fn down(client: &VlmClient, why: &str) -> AppError {
        AppError::new(
            ErrorCode::Upsampler,
            format!("The prompt upsampler at {} is not answering ({why}).", client.base),
        )
        .suggest(
            "It is a separate model that loads on demand and can take a couple of minutes the first time: \
             wait a minute and try again. If it keeps failing, check the gateway's log.",
        )
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

/// `content` is a plain string on most servers; some wrap it in a parts array.
#[derive(Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<serde_json::Value>,
}

impl ChatMessage {
    fn content_string(&self) -> String {
        match &self.content {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Array(parts)) => parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        }
    }
}

#[derive(Deserialize)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

/// A one-shot loopback server that answers any POST with `status` and `body`, capturing the
/// raw request. Shared with the `engine` tests.
#[cfg(test)]
pub async fn mock_chat(
    status: u16,
    body: &str,
) -> (u16, tokio::sync::oneshot::Receiver<String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let body = body.to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let Ok((mut sock, _)) = listener.accept().await else {
            return;
        };
        let mut raw = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = sock.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&raw);
            let end = text.find("\r\n\r\n").unwrap();
            let len = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if raw.len() >= end + 4 + len {
                break;
            }
        }
        let head = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = sock.write_all(head.as_bytes()).await;
        let _ = sock.write_all(body.as_bytes()).await;
        let _ = tx.send(String::from_utf8_lossy(&raw).into_owned());
    });
    (port, rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdcpp::TextSampling;

    fn req(prompt: &str, images: Vec<&str>) -> TextGenRequest {
        TextGenRequest {
            prompt: prompt.to_string(),
            system_prompt: "the system prompt".to_string(),
            images: images.into_iter().map(|s| s.to_string()).collect(),
            max_tokens: 3584,
            sampling: TextSampling {
                temperature: 0.0,
                top_p: 0.95,
                top_k: 20,
                presence_penalty: 1.5,
                seed: 42,
            },
            stream: false,
        }
    }

    fn endpoint(port: u16, key: &str) -> VlmEndpoint {
        VlmEndpoint {
            entrypoint: format!("http://127.0.0.1:{port}/upstream/kokon-pup"),
            model: "q35".into(),
            api_key: key.to_string(),
        }
    }

    #[tokio::test]
    async fn chat_sends_the_turn_and_parses_the_reply() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"a red mug on a table"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1047,"completion_tokens":12}}"#;
        let (port, seen) = mock_chat(200, body).await;
        let client = VlmClient::new(&endpoint(port, ""));
        let reply = client.chat(&req("a red mug", vec!["B64IMG"]), Duration::from_secs(10)).await.unwrap();
        assert_eq!(reply.text, "a red mug on a table");
        assert_eq!(reply.finish_reason, "stop");
        assert_eq!((reply.prompt_tokens, reply.completion_tokens), (1047, 12));

        let raw = seen.await.unwrap();
        assert!(raw.contains("POST /upstream/kokon-pup/v1/chat/completions"));
        assert!(!raw.to_ascii_lowercase().contains("authorization"), "keyless: no auth header");
        let sent: serde_json::Value = raw[raw.find('{').unwrap()..].parse().unwrap();
        assert_eq!(sent["model"], "q35");
        assert_eq!(sent["stream"], false);
        assert_eq!(sent["max_tokens"], 3584);
        assert_eq!(sent["temperature"], 0.0);
        assert_eq!(sent["top_p"], 0.95);
        assert_eq!(sent["presence_penalty"], 1.5);
        assert_eq!(sent["seed"], 42);
        assert_eq!(sent["messages"][0]["role"], "system");
        assert_eq!(sent["messages"][1]["role"], "user");
        assert_eq!(sent["messages"][1]["content"][0]["type"], "image_url");
        assert_eq!(
            sent["messages"][1]["content"][0]["image_url"]["url"],
            "data:image/png;base64,B64IMG"
        );
        assert_eq!(sent["messages"][1]["content"][1]["type"], "text");
        assert_eq!(sent["messages"][1]["content"][1]["text"], "a red mug");
    }

    #[tokio::test]
    async fn the_auth_header_is_sent_only_with_a_key() {
        let body = r#"{"choices":[{"message":{"content":"ok"}}]}"#;
        let (port, seen) = mock_chat(200, body).await;
        let client = VlmClient::new(&endpoint(port, "sekret"));
        client.chat(&req("hi", vec![]), Duration::from_secs(10)).await.unwrap();
        let raw = seen.await.unwrap();
        assert!(raw.contains("authorization: Bearer sekret"), "{}", raw);
    }

    #[tokio::test]
    async fn an_unreachable_upsampler_is_an_actionable_error() {
        // Port 1 never listens.
        let client = VlmClient::new(&endpoint(1, ""));
        let err = client.chat(&req("hi", vec![]), Duration::from_secs(5)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Upsampler);
        assert!(err.message.contains("127.0.0.1:1"), "{}", err.message);
        assert!(err.message.contains("no server is listening"), "{}", err.message);
        assert!(err.suggestion.is_some());
    }

    #[tokio::test]
    async fn a_server_error_carries_the_status_and_body() {
        let (port, _seen) = mock_chat(500, r#"{"error":"model still loading"}"#).await;
        let client = VlmClient::new(&endpoint(port, ""));
        let err = client.chat(&req("hi", vec![]), Duration::from_secs(5)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Upsampler);
        assert!(err.message.contains("500"), "{}", err.message);
        assert!(err.message.contains("model still loading"), "{}", err.message);
    }
}
