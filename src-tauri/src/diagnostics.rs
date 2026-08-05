use reqwest::{Client, StatusCode, Url};
use serde::Serialize;

use crate::{
    config::AppConfig,
    provider::{openai_endpoint, OpenAiCompatibleProvider, ProviderErrorKind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DiagnosticService {
    Stt,
    Llm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DiagnosticResult {
    pub(crate) service: DiagnosticService,
    pub(crate) success: bool,
    pub(crate) message: String,
}

impl DiagnosticResult {
    fn success(service: DiagnosticService, message: impl Into<String>) -> Self {
        Self {
            service,
            success: true,
            message: message.into(),
        }
    }

    fn failure(service: DiagnosticService, message: impl Into<String>) -> Self {
        Self {
            service,
            success: false,
            message: message.into(),
        }
    }
}

/// Performs a read-only OpenAI-compatible `/models` probe.
///
/// This deliberately does not submit fake or recorded audio. A successful result
/// proves only that the base URL is reachable and accepted this request. It does
/// not prove that the credential is enforced, the model exists, or transcription works.
pub(crate) async fn test_stt(client: &Client, config: &AppConfig) -> DiagnosticResult {
    if let Err(message) =
        validate_service_config(&config.stt_base_url, &config.stt_api_key, &config.stt_model)
    {
        return DiagnosticResult::failure(DiagnosticService::Stt, message);
    }

    let response = client
        .get(openai_endpoint(&config.stt_base_url, "models"))
        .bearer_auth(config.stt_api_key.trim())
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => DiagnosticResult::success(
            DiagnosticService::Stt,
            "基础端点可达且请求已被接受；尚未验证凭据、模型或转写能力，请再进行一次真实录音。",
        ),
        Ok(response) => {
            DiagnosticResult::failure(DiagnosticService::Stt, http_failure(response.status()))
        }
        Err(error) if error.is_timeout() => {
            DiagnosticResult::failure(DiagnosticService::Stt, "连接超时，请检查网络或服务状态。")
        }
        Err(error) if error.is_connect() => DiagnosticResult::failure(
            DiagnosticService::Stt,
            "无法连接服务，请检查 Base URL 和网络。",
        ),
        Err(_) => {
            DiagnosticResult::failure(DiagnosticService::Stt, "连接测试未完成，请检查配置后重试。")
        }
    }
}

/// Makes one intentionally tiny chat-completion request after an explicit user
/// action. It can consume a small amount of provider quota.
pub(crate) async fn test_llm(client: &Client, config: &AppConfig) -> DiagnosticResult {
    if let Err(message) =
        validate_service_config(&config.llm_base_url, &config.llm_api_key, &config.llm_model)
    {
        return DiagnosticResult::failure(DiagnosticService::Llm, message);
    }

    let provider = OpenAiCompatibleProvider::new(client);
    match provider
        .test_completion(
            config.llm_base_url.trim(),
            config.llm_api_key.trim(),
            config.llm_model.trim(),
        )
        .await
    {
        Ok(_) => {
            DiagnosticResult::success(DiagnosticService::Llm, "连接成功，模型已返回有效响应。")
        }
        Err(error) => {
            DiagnosticResult::failure(DiagnosticService::Llm, provider_failure(error.kind))
        }
    }
}

fn validate_service_config(base_url: &str, api_key: &str, model: &str) -> Result<(), &'static str> {
    let base_url = base_url.trim();
    let api_key = api_key.trim();
    let model = model.trim();

    if base_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Err("请先填写完整的 Base URL、API Key 和模型。");
    }
    if api_key.chars().any(char::is_control) || model.chars().any(char::is_control) {
        return Err("API Key 或模型格式无效。");
    }

    let url = Url::parse(base_url).map_err(|_| "Base URL 格式无效。")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Base URL 必须是有效的 HTTP 或 HTTPS 地址。");
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Base URL 不应包含凭据、查询参数或片段。");
    }
    Ok(())
}

fn http_failure(status: StatusCode) -> &'static str {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "鉴权失败，请检查 API Key 或服务权限。",
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
            "服务不支持基础连接探测，请确认 Base URL；仍可通过真实录音验证 STT。"
        }
        StatusCode::TOO_MANY_REQUESTS => "请求受限，请稍后重试或检查服务额度。",
        status if status.is_server_error() => "服务暂时不可用，请稍后重试。",
        _ => "服务拒绝了连接测试，请检查配置。",
    }
}

fn provider_failure(kind: ProviderErrorKind) -> &'static str {
    match kind {
        ProviderErrorKind::Timeout => "连接超时，请检查网络或服务状态。",
        ProviderErrorKind::Connection => "无法连接服务，请检查 Base URL 和网络。",
        ProviderErrorKind::Http => "服务拒绝请求，请检查 API Key、模型或额度。",
        ProviderErrorKind::InvalidResponse => "服务响应格式不兼容。",
        ProviderErrorKind::EmptyResponse => "模型返回了空响应。",
        ProviderErrorKind::Request => "连接测试未完成，请检查配置后重试。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_safe_service_configuration() {
        assert!(validate_service_config("https://api.example.com/v1", "secret", "model").is_ok());
        assert!(validate_service_config("", "secret", "model").is_err());
        assert!(validate_service_config("ftp://api.example.com", "secret", "model").is_err());
        assert!(
            validate_service_config("https://secret@api.example.com", "secret", "model").is_err()
        );
        assert!(
            validate_service_config("https://api.example.com?key=secret", "secret", "model")
                .is_err()
        );
        assert!(validate_service_config("https://api.example.com", "bad\nkey", "model").is_err());
    }

    #[test]
    fn diagnostic_messages_never_include_provider_details() {
        let message = provider_failure(ProviderErrorKind::Http);
        assert!(!message.contains("secret"));
        assert!(!message.contains("provider body"));
    }

    #[test]
    fn classifies_common_http_failures_without_response_body() {
        assert!(http_failure(StatusCode::UNAUTHORIZED).contains("鉴权"));
        assert!(http_failure(StatusCode::NOT_FOUND).contains("基础连接探测"));
        assert!(http_failure(StatusCode::TOO_MANY_REQUESTS).contains("受限"));
        assert!(http_failure(StatusCode::INTERNAL_SERVER_ERROR).contains("暂时不可用"));
    }

    #[test]
    fn service_names_match_the_frontend_contract() {
        assert_eq!(
            serde_json::to_string(&DiagnosticService::Stt).unwrap(),
            "\"stt\""
        );
        assert_eq!(
            serde_json::to_string(&DiagnosticService::Llm).unwrap(),
            "\"llm\""
        );
    }
}
