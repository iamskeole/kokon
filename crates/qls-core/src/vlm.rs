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
    /// Length of the model's reasoning block, when the server carries it in a
    /// separate field (this llama.cpp build does, with thinking enabled): kept
    /// as a length only — the block itself is never shown, logged or stored.
    pub reasoning_chars: usize,
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

    /// Construct with the service's gateway key as an **inherited default**:
    /// an endpoint that names no key of its own presents the gateway key
    /// (the one-door topology — the service's gateway client and every VLM
    /// entrypoint sit behind the same keyed gateway, one admission key); a
    /// non-empty endpoint key always wins (a per-endpoint key is an
    /// override, e.g. for a VLM served by a different, separately keyed
    /// server). Both empty = keyless, as with [`Self::new`].
    pub fn with_inherited_key(endpoint: &VlmEndpoint, inherited: Option<&str>) -> Self {
        let own = endpoint.api_key.trim();
        if !own.is_empty() {
            return Self::new(endpoint);
        }
        match inherited.map(str::trim).filter(|k| !k.is_empty()) {
            Some(k) => {
                let mut e = endpoint.clone();
                e.api_key = k.to_string();
                Self::new(&e)
            }
            None => Self::new(endpoint),
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
            reasoning_chars: choice.message.reasoning_chars(),
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
    /// This llama.cpp build carries the model's reasoning block in its own field
    /// (absent when it thinks nothing); other servers may not, or may inline it.
    #[serde(default)]
    reasoning_content: Option<serde_json::Value>,
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

    fn reasoning_chars(&self) -> usize {
        match &self.reasoning_content {
            Some(serde_json::Value::String(s)) => s.len(),
            _ => 0,
        }
    }
}

/// The model's reasoning block, when a server inlines it in the answer text
/// (some builds do; this box's carries it in the separate field instead).
/// Returns the answer with the block removed; empty when nothing usable is
/// left. Never called on text that must keep its shape (the BFL path reads the
/// raw text as-is) — this is the native decode's hygiene step only.
pub fn strip_think(text: &str) -> String {
    // The model's reasoning-block tags (the angle-bracketed "think" and
    // "close-think" markers this llama.cpp build emits), written in escape
    // form so they stay exact through any text handling.
    const OPEN: &str = "\u{003c}think\u{003e}";
    const CLOSE: &str = "\u{003c}/think\u{003e}";
    let mut out = text;
    loop {
        let Some(start) = out.find(OPEN) else {
            return out.trim().to_string();
        };
        let after = &out[start + OPEN.len()..];
        let Some(end) = after.find(CLOSE) else {
            // No closing tag: everything from the opener on is reasoning.
            return out[..start].trim().to_string();
        };
        out = &after[end + CLOSE.len()..];
    }
}

/// A decoded native PE reply: the rewritten prompt plus the door's suggested
/// output ratio, when it gave one.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeReply {
    /// The envelope's `rewritten_prompt`, as the image model gets it.
    pub prompt: String,
    /// The door's size suggestion: the `wh_ratio` it chose ("3:2") or the
    /// reference it follows ("follows <image1>"). Recorded on the job view;
    /// the planner keeps size authority (the user's selection wins).
    pub suggestion: Option<String>,
}

/// Decode a native PE door's answer: an optional reasoning block, then a JSON
/// envelope `{"rewritten_prompt": …, "wh_ratio": … | "ratio_follow": …}`. The
/// envelope is the fine-tune's output contract (conditioned on its own system
/// prompt); its two ratio fields are mutually exclusive at *value* level, so
/// both may be present and one of them empty.
///
/// None when the text is not a usable envelope (the caller retries once and
/// then keeps the raw text — the door ran, it just answered out of contract).
pub fn decode_native(raw: &str) -> Option<NativeReply> {
    let text = strip_think(raw);
    let start = text.find('{')?;
    // The answer is the object, written bare; be tolerant of a trailing
    // fragment by trimming back to the last closing brace if the strict parse
    // of the whole tail fails.
    let tail = &text[start..];
    let obj = serde_json::from_str::<serde_json::Value>(tail).ok().or_else(|| {
        let end = tail.rfind('}')?;
        serde_json::from_str::<serde_json::Value>(&tail[..=end]).ok()
    })?;
    let prompt = obj
        .get("rewritten_prompt")?
        .as_str()?
        .trim()
        .to_string();
    if prompt.is_empty() {
        return None;
    }
    let field = |key: &str| -> String {
        obj.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
    };
    let wh = field("wh_ratio");
    let follow = field("ratio_follow");
    let suggestion = if !wh.is_empty() {
        Some(wh)
    } else if !follow.is_empty() {
        Some(format!("follows {follow}"))
    } else {
        None
    };
    Some(NativeReply { prompt, suggestion })
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

/// Like [`mock_chat`] but answers several consecutive requests on the same
/// port with the bodies in order (the last one repeats): for the paths that
/// call the upsampler twice (retries).
#[cfg(test)]
pub async fn mock_chat_several(
    status: u16,
    bodies: &[&str],
) -> (u16, tokio::sync::mpsc::UnboundedReceiver<String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let bodies: Vec<String> = bodies.iter().map(|b| b.to_string()).collect();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut i = 0;
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
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
            let body = bodies[i.min(bodies.len() - 1)].clone();
            let head = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            if tx.send(String::from_utf8_lossy(&raw).into_owned()).is_err() {
                break;
            }
            i += 1;
        }
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
            entrypoint: format!("http://127.0.0.1:{port}/upstream/kokon-pup-flux-image-3-klein"),
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
        assert!(raw.contains("POST /upstream/kokon-pup-flux-image-3-klein/v1/chat/completions"));
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
    async fn an_empty_endpoint_key_inherits_the_gateway_key() {
        // The one-door topology: the service's key to the gateway applies
        // to any VLM entrypoint behind the same gateway that names no key
        // of its own.
        let body = r#"{"choices":[{"message":{"content":"ok"}}]}"#;
        let (port, seen) = mock_chat(200, body).await;
        let client = VlmClient::with_inherited_key(&endpoint(port, ""), Some("gwkey"));
        client.chat(&req("hi", vec![]), Duration::from_secs(10)).await.unwrap();
        let raw = seen.await.unwrap();
        assert!(raw.contains("authorization: Bearer gwkey"), "{}", raw);
    }

    #[tokio::test]
    async fn an_endpoint_key_beats_the_inherited_one() {
        let body = r#"{"choices":[{"message":{"content":"ok"}}]}"#;
        let (port, seen) = mock_chat(200, body).await;
        let client = VlmClient::with_inherited_key(&endpoint(port, "sekret"), Some("gwkey"));
        client.chat(&req("hi", vec![]), Duration::from_secs(10)).await.unwrap();
        let raw = seen.await.unwrap();
        assert!(raw.contains("authorization: Bearer sekret"), "{}", raw);
        assert!(!raw.contains("gwkey"), "{}", raw);
    }

    #[tokio::test]
    async fn no_key_anywhere_stays_keyless() {
        let body = r#"{"choices":[{"message":{"content":"ok"}}]}"#;
        let (port, seen) = mock_chat(200, body).await;
        let client = VlmClient::with_inherited_key(&endpoint(port, ""), None);
        client.chat(&req("hi", vec![]), Duration::from_secs(10)).await.unwrap();
        let raw = seen.await.unwrap();
        assert!(!raw.to_ascii_lowercase().contains("authorization"), "{}", raw);
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

    // --- The native PE contract (schema 14) ---------------------------------

    // The reasoning tags, in escape form (the file's convention: the model's
    // reasoning-block markers stay exact through any text handling).
    const TH_OPEN: &str = "\u{003c}think\u{003e}";
    const TH_CLOSE: &str = "\u{003c}/think\u{003e}";

    #[test]
    fn strip_think_removes_an_inline_block_and_keeps_everything_around_it() {
        let raw = format!("{TH_OPEN} pondering {TH_CLOSE} the answer");
        assert_eq!(strip_think(&raw), "the answer");
        // No tag at all: the text is untouched.
        assert_eq!(strip_think("plain words"), "plain words");
        // An opened block with no close: the whole tail is reasoning.
        let tail = format!("a head {TH_OPEN} still pondering");
        assert_eq!(strip_think(&tail), "a head");
        // A block at the very start.
        let lead = format!("{TH_OPEN} thinking {TH_CLOSE}{TH_OPEN} more {TH_CLOSE} done");
        assert_eq!(strip_think(&lead), "done");
    }

    #[test]
    fn decode_native_parses_the_envelope_with_its_reasoning_block() {
        let raw = format!(
            "{TH_OPEN} a long reasoning block {TH_CLOSE}\n{{\"rewritten_prompt\":\"a studio portrait of a red mug\",\"wh_ratio\":\"3:2\"}}"
        );
        let n = decode_native(&raw).expect("in-contract reply decodes");
        assert_eq!(n.prompt, "a studio portrait of a red mug");
        assert_eq!(n.suggestion.as_deref(), Some("3:2"));
    }

    #[test]
    fn decode_native_takes_ratio_follow_when_wh_ratio_is_empty() {
        // Value-level mutual exclusion: both keys present, one of them empty.
        let raw = r#"{"rewritten_prompt":"p","wh_ratio":"","ratio_follow":"image1"}"#;
        let n = decode_native(raw).unwrap();
        assert_eq!(n.suggestion.as_deref(), Some("follows image1"));
        // And the inverse: a filled wh_ratio wins over a filled follow.
        let raw = r#"{"rewritten_prompt":"p","wh_ratio":"4:5","ratio_follow":"image1"}"#;
        assert_eq!(decode_native(raw).unwrap().suggestion.as_deref(), Some("4:5"));
        // Neither filled: no suggestion, the prompt still decodes.
        let raw = r#"{"rewritten_prompt":"p"}"#;
        let n = decode_native(raw).unwrap();
        assert_eq!(n.suggestion, None);
    }

    #[test]
    fn decode_native_rejects_prose_and_an_empty_prompt() {
        // The no-system-prompt answer shape: rich plain text, no envelope.
        assert!(decode_native("A vivid red mug, well lit, on a wooden table").is_none());
        // An envelope with an empty rewritten_prompt is not a usable answer.
        assert!(decode_native(r#"{"rewritten_prompt":"  ","wh_ratio":"1:1"}"#).is_none());
        // Braces inside prose without the key: still not an envelope.
        assert!(decode_native("it looks like { a mug } and more").is_none());
    }

    #[test]
    fn decode_native_tolerates_a_trailing_fragment() {
        // The fine-tune's greedy decoding can spill a fragment past the
        // object: the strict parse fails, the fallback trims to the last brace.
        let raw = r#"{"rewritten_prompt":"p","wh_ratio":"1:1"} and then some words"#;
        let n = decode_native(raw).expect("the fallback trims the tail");
        assert_eq!(n.prompt, "p");
        assert_eq!(n.suggestion.as_deref(), Some("1:1"));
    }

    #[tokio::test]
    async fn a_separate_reasoning_field_is_counted_not_concated_to_the_text() {
        // This box's llama.cpp carries the thinking in its own field; the
        // answer text must come back clean of it, with its size recorded.
        let body = format!(
            r#"{{"choices":[{{"message":{{"role":"assistant","content":"the clean answer","reasoning_content":"{} ..."}}}}]}}"#,
            "t".repeat(128)
        );
        let (port, _seen) = mock_chat(200, &body).await;
        let client = VlmClient::new(&endpoint(port, ""));
        let reply = client.chat(&req("hi", vec![]), Duration::from_secs(10)).await.unwrap();
        assert_eq!(reply.text, "the clean answer");
        assert!(reply.reasoning_chars > 100, "the field's size is recorded");
    }

    #[tokio::test]
    async fn an_inline_think_block_in_content_is_not_concated_either() {
        // Servers that inline the block in `content`: the text field is the raw
        // content (the caller's decode strips it); only the separate field is
        // special-cased, so the raw shape stays inspectable.
        let body = format!(
            r#"{{"choices":[{{"message":{{"content":"a head {} and more"}}}}]}}"#,
            TH_OPEN
        );
        let (port, _seen) = mock_chat(200, &body).await;
        let client = VlmClient::new(&endpoint(port, ""));
        let reply = client.chat(&req("hi", vec![]), Duration::from_secs(10)).await.unwrap();
        assert_eq!(reply.text, format!("a head {TH_OPEN} and more"));
        assert_eq!(strip_think(&reply.text), "a head", "the unclosed block's tail is reasoning");
    }
}
