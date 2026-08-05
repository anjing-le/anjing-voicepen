use std::fmt;

use reqwest::Client;
use serde::Deserialize;

/// Minimal boundary around the OpenAI-compatible endpoints used by VoicePen.
pub(crate) struct OpenAiCompatibleProvider<'a> {
    client: &'a Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderErrorKind {
    Timeout,
    Connection,
    Http,
    InvalidResponse,
    EmptyResponse,
    Request,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderError {
    pub(crate) kind: ProviderErrorKind,
    message: String,
}

impl ProviderError {
    fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub(crate) fn user_message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProviderError {}

impl<'a> OpenAiCompatibleProvider<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub(crate) async fn transcribe(
        &self,
        base_url: &str,
        api_key: &str,
        model: &str,
        wav_bytes: Vec<u8>,
    ) -> Result<String, ProviderError> {
        let file_part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("voicepen.wav")
            .mime_str("audio/wav")
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::Request, "准备音频请求失败，请重试。")
            })?;
        let form = reqwest::multipart::Form::new()
            .text("model", model.to_owned())
            .part("file", file_part);

        let response = self
            .client
            .post(openai_endpoint(base_url, "audio/transcriptions"))
            .bearer_auth(api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|error| request_error("STT 转写请求失败", &error))?;

        if !response.status().is_success() {
            return Err(http_error("STT 转写失败", response));
        }

        let bytes = response.bytes().await.map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::InvalidResponse,
                "读取 STT 响应失败，请重试。",
            )
        })?;
        parse_stt_response(&bytes)
    }

    pub(crate) async fn polish(
        &self,
        base_url: &str,
        api_key: &str,
        model: &str,
        prompt: &str,
        transcript: &str,
    ) -> Result<String, ProviderError> {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": prompt },
                { "role": "user", "content": transcript }
            ],
            "temperature": 0.2
        });

        let response = self
            .client
            .post(openai_endpoint(base_url, "chat/completions"))
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| request_error("LLM 润色请求失败", &error))?;

        if !response.status().is_success() {
            return Err(http_error("LLM 润色失败", response));
        }

        let bytes = response.bytes().await.map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::InvalidResponse,
                "读取 LLM 响应失败，请重试。",
            )
        })?;
        parse_chat_response(&bytes)
    }

    pub(crate) async fn test_completion(
        &self,
        base_url: &str,
        api_key: &str,
        model: &str,
    ) -> Result<String, ProviderError> {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": "这是连接测试。只回复 OK。" },
                { "role": "user", "content": "ping" }
            ],
            "temperature": 0.2
        });
        let response = self
            .client
            .post(openai_endpoint(base_url, "chat/completions"))
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| request_error("LLM 连接测试失败", &error))?;
        if !response.status().is_success() {
            return Err(http_error("LLM 连接测试失败", response));
        }
        let bytes = response.bytes().await.map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::InvalidResponse,
                "读取 LLM 测试响应失败，请重试。",
            )
        })?;
        parse_chat_response(&bytes)
    }
}

#[derive(Debug, Deserialize)]
struct SttResponse {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

fn parse_stt_response(bytes: &[u8]) -> Result<String, ProviderError> {
    let payload: SttResponse = serde_json::from_slice(bytes).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::InvalidResponse,
            "解析 STT 响应失败，服务返回格式不兼容。",
        )
    })?;
    non_empty_text(
        payload.text.as_deref(),
        "STT 返回了空文本，请检查模型或音频质量。",
    )
}

fn parse_chat_response(bytes: &[u8]) -> Result<String, ProviderError> {
    let payload: ChatCompletionResponse = serde_json::from_slice(bytes).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::InvalidResponse,
            "解析 LLM 响应失败，服务返回格式不兼容。",
        )
    })?;
    non_empty_text(
        payload
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref()),
        "LLM 返回了空文本，请检查模型配置。",
    )
}

fn non_empty_text(value: Option<&str>, empty_message: &str) -> Result<String, ProviderError> {
    let text = value.unwrap_or_default().trim();
    if text.is_empty() {
        Err(ProviderError::new(
            ProviderErrorKind::EmptyResponse,
            empty_message,
        ))
    } else {
        Ok(text.to_owned())
    }
}

pub(crate) fn openai_endpoint(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{path}")
    } else {
        format!("{base}/v1/{path}")
    }
}

fn request_error(prefix: &str, error: &reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::new(
            ProviderErrorKind::Timeout,
            format!("{prefix}：请求超时，请检查网络或服务状态。"),
        )
    } else if error.is_connect() {
        ProviderError::new(
            ProviderErrorKind::Connection,
            format!("{prefix}：连接失败，请检查 Base URL。"),
        )
    } else {
        // reqwest errors may contain the request URL. Avoid reflecting it because
        // compatible services sometimes put credentials in URL query parameters.
        ProviderError::new(
            ProviderErrorKind::Request,
            format!("{prefix}：请求未完成，请检查配置后重试。"),
        )
    }
}

fn http_error(prefix: &str, response: reqwest::Response) -> ProviderError {
    let status = response.status();
    ProviderError::new(
        ProviderErrorKind::Http,
        format!("{prefix}（HTTP {status}）。"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_accepts_root_or_v1_base_url() {
        assert_eq!(
            openai_endpoint(" https://api.example.com/ ", "audio/transcriptions"),
            "https://api.example.com/v1/audio/transcriptions"
        );
        assert_eq!(
            openai_endpoint("https://api.example.com/v1/", "chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn http_errors_do_not_reflect_provider_response_bodies() {
        let message = ProviderError::new(
            ProviderErrorKind::Http,
            "LLM 润色失败（HTTP 401 Unauthorized）。",
        );
        assert_eq!(message.kind, ProviderErrorKind::Http);
        assert!(!message.user_message().contains("secret provider body"));
    }

    #[test]
    fn parses_and_trims_stt_response() {
        assert_eq!(
            parse_stt_response(br#"{"text":"  hello world  "}"#).unwrap(),
            "hello world"
        );
        assert_eq!(
            parse_stt_response(br#"{"text":" "}"#).unwrap_err().kind,
            ProviderErrorKind::EmptyResponse
        );
    }

    #[test]
    fn parses_first_chat_choice() {
        let response = br#"{"choices":[{"message":{"content":"  polished  "}}]}"#;
        assert_eq!(parse_chat_response(response).unwrap(), "polished");
        assert_eq!(
            parse_chat_response(br#"{"choices":[]}"#).unwrap_err().kind,
            ProviderErrorKind::EmptyResponse
        );
    }

    #[test]
    fn invalid_json_does_not_echo_provider_body() {
        let error = parse_chat_response(b"secret invalid response").unwrap_err();
        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(!error.to_string().contains("secret"));
    }
}
