use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

const DEFAULT_SHORTCUT: &str = "Alt+Shift+V";
const DEFAULT_PROMPT: &str =
    "请将下面的语音转写文本润色成自然、清晰、可直接发送的中文。保留原意，不扩写，不加入新信息，不解释，只输出润色后的正文。";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AppConfig {
    #[serde(default)]
    pub(crate) stt_base_url: String,
    #[serde(default)]
    pub(crate) stt_api_key: String,
    #[serde(default = "default_stt_model")]
    pub(crate) stt_model: String,
    #[serde(default)]
    pub(crate) llm_base_url: String,
    #[serde(default)]
    pub(crate) llm_api_key: String,
    #[serde(default = "default_llm_model")]
    pub(crate) llm_model: String,
    #[serde(default = "default_prompt")]
    pub(crate) polish_prompt: String,
    #[serde(default = "default_shortcut")]
    pub(crate) shortcut: String,
    #[serde(default)]
    pub(crate) auto_paste: bool,
    #[serde(default = "default_theme")]
    pub(crate) theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            stt_base_url: String::new(),
            stt_api_key: String::new(),
            stt_model: default_stt_model(),
            llm_base_url: String::new(),
            llm_api_key: String::new(),
            llm_model: default_llm_model(),
            polish_prompt: default_prompt(),
            shortcut: default_shortcut(),
            auto_paste: false,
            theme: default_theme(),
        }
    }
}

impl AppConfig {
    pub(crate) fn normalize(mut self) -> Self {
        self.stt_base_url = normalize_base_url(&self.stt_base_url);
        self.llm_base_url = normalize_base_url(&self.llm_base_url);
        self.stt_api_key = self.stt_api_key.trim().to_string();
        self.llm_api_key = self.llm_api_key.trim().to_string();
        self.stt_model = self.stt_model.trim().to_string();
        self.llm_model = self.llm_model.trim().to_string();
        self.shortcut = self.shortcut.trim().to_string();
        if self.shortcut.is_empty() {
            self.shortcut = default_shortcut();
        }
        if self.polish_prompt.trim().is_empty() {
            self.polish_prompt = default_prompt();
        } else {
            self.polish_prompt = self.polish_prompt.trim().to_string();
        }
        if !matches!(self.theme.as_str(), "light" | "dark" | "system") {
            self.theme = default_theme();
        }
        self
    }

    pub(crate) fn is_configured(&self) -> bool {
        !self.stt_base_url.trim().is_empty()
            && !self.stt_api_key.trim().is_empty()
            && !self.stt_model.trim().is_empty()
            && !self.llm_base_url.trim().is_empty()
            && !self.llm_api_key.trim().is_empty()
            && !self.llm_model.trim().is_empty()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let mut missing = Vec::new();
        if self.stt_base_url.is_empty() {
            missing.push("STT Base URL");
        }
        if self.stt_api_key.is_empty() {
            missing.push("STT API Key");
        }
        if self.stt_model.is_empty() {
            missing.push("STT Model");
        }
        if self.llm_base_url.is_empty() {
            missing.push("LLM Base URL");
        }
        if self.llm_api_key.is_empty() {
            missing.push("LLM API Key");
        }
        if self.llm_model.is_empty() {
            missing.push("LLM Model");
        }
        if !missing.is_empty() {
            return Err(format!("请补充必要配置：{}", missing.join("、")));
        }
        self.shortcut
            .parse::<Shortcut>()
            .map_err(|e| format!("快捷键格式无效：{e}"))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ConfigPayload {
    pub(crate) config: AppConfig,
    pub(crate) configured: bool,
    pub(crate) config_path: String,
}

impl ConfigPayload {
    pub(crate) fn new(config: AppConfig, path: &Path) -> Self {
        Self {
            configured: config.is_configured(),
            config,
            config_path: path.display().to_string(),
        }
    }
}

fn default_stt_model() -> String {
    String::new()
}

fn default_llm_model() -> String {
    String::new()
}

fn default_prompt() -> String {
    DEFAULT_PROMPT.to_string()
}

fn default_shortcut() -> String {
    DEFAULT_SHORTCUT.to_string()
}

fn default_theme() -> String {
    "system".to_string()
}

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn config_dir() -> Result<PathBuf, String> {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .ok_or("无法获取系统配置目录")?;
    Ok(base.join("VoicePen"))
}

pub(crate) fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("config.json"))
}

pub(crate) fn read_config_file(path: &Path) -> Result<AppConfig, String> {
    recover_backup_if_needed(path)?;
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(AppConfig::default());
    };
    Ok(serde_json::from_str::<AppConfig>(&raw)
        .map(AppConfig::normalize)
        .unwrap_or_default())
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.voicepen-backup")
}

fn recover_backup_if_needed(path: &Path) -> Result<(), String> {
    let backup = backup_path(path);
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| format!("清理旧配置备份失败：{error}"))?;
        }
        return Ok(());
    }
    if backup.exists() {
        fs::rename(&backup, path).map_err(|error| format!("恢复配置备份失败：{error}"))?;
    }
    Ok(())
}

pub(crate) fn write_config_file(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }

    let data = serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败：{e}"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let temp_path = path.with_file_name(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));
    let mut temp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|e| format!("创建临时配置文件失败：{e}"))?;
    if let Err(error) = temp_file.write_all(data.as_bytes()) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("写入临时配置文件失败：{error}"));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        if let Err(error) = temp_file.set_permissions(permissions) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("限制配置文件权限失败：{error}"));
        }
    }

    if let Err(error) = temp_file.sync_all() {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("同步配置文件失败：{error}"));
    }
    drop(temp_file);

    commit_temp_file(&temp_path, path)
}

#[cfg(not(target_os = "windows"))]
fn commit_temp_file(temp_path: &Path, path: &Path) -> Result<(), String> {
    fs::rename(temp_path, path).map_err(|error| {
        let _ = fs::remove_file(temp_path);
        format!("提交配置文件失败：{error}")
    })
}

#[cfg(target_os = "windows")]
fn commit_temp_file(temp_path: &Path, path: &Path) -> Result<(), String> {
    let backup_path = backup_path(path);
    let had_previous = path.exists();
    if had_previous {
        let _ = fs::remove_file(&backup_path);
        fs::rename(path, &backup_path).map_err(|error| {
            let _ = fs::remove_file(temp_path);
            format!("备份旧配置文件失败：{error}")
        })?;
    }

    if let Err(error) = fs::rename(temp_path, path) {
        let recovery_error = if had_previous {
            fs::rename(&backup_path, path)
                .err()
                .map(|recovery| format!("；恢复旧配置失败：{recovery}"))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let _ = fs::remove_file(temp_path);
        return Err(format!("提交配置文件失败：{error}{recovery_error}"));
    }
    if had_previous {
        let _ = fs::remove_file(backup_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn configured() -> AppConfig {
        AppConfig {
            stt_base_url: "https://stt.example/v1".into(),
            stt_api_key: "stt-secret".into(),
            stt_model: "whisper".into(),
            llm_base_url: "https://llm.example/v1".into(),
            llm_api_key: "llm-secret".into(),
            llm_model: "chat".into(),
            ..AppConfig::default()
        }
    }

    fn temporary_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "voicepen-config-test-{}-{nonce}",
                std::process::id()
            ))
            .join(name)
    }

    #[test]
    fn default_config_preserves_existing_defaults() {
        let config = AppConfig::default();

        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(config.polish_prompt, DEFAULT_PROMPT);
        assert_eq!(config.theme, "system");
        assert!(!config.auto_paste);
        assert!(!config.is_configured());
    }

    #[test]
    fn normalize_trims_values_and_repairs_optional_choices() {
        let config = AppConfig {
            stt_base_url: " https://stt.example/v1/// ".into(),
            stt_api_key: " stt-secret ".into(),
            stt_model: " whisper ".into(),
            llm_base_url: " https://llm.example/v1/ ".into(),
            llm_api_key: " llm-secret ".into(),
            llm_model: " chat ".into(),
            polish_prompt: "  ".into(),
            shortcut: "  ".into(),
            auto_paste: true,
            theme: "unknown".into(),
        }
        .normalize();

        assert_eq!(config.stt_base_url, "https://stt.example/v1");
        assert_eq!(config.llm_base_url, "https://llm.example/v1");
        assert_eq!(config.stt_api_key, "stt-secret");
        assert_eq!(config.stt_model, "whisper");
        assert_eq!(config.polish_prompt, DEFAULT_PROMPT);
        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(config.theme, "system");
        assert!(config.auto_paste);
    }

    #[test]
    fn validation_reports_missing_fields_and_invalid_shortcut() {
        let missing = AppConfig::default().validate().unwrap_err();
        assert!(missing.contains("STT Base URL"));
        assert!(missing.contains("LLM API Key"));

        let invalid = AppConfig {
            shortcut: "definitely not a shortcut".into(),
            ..configured()
        }
        .validate()
        .unwrap_err();
        assert!(invalid.starts_with("快捷键格式无效："));

        assert!(configured().is_configured());
        assert!(configured().validate().is_ok());
    }

    #[test]
    fn older_json_gets_defaults_for_fields_added_later() {
        let config: AppConfig = serde_json::from_str(
            r#"{
                "stt_base_url": "https://stt.example",
                "stt_api_key": "stt-secret",
                "stt_model": "whisper",
                "llm_base_url": "https://llm.example",
                "llm_api_key": "llm-secret",
                "llm_model": "chat"
            }"#,
        )
        .unwrap();

        assert_eq!(config.polish_prompt, DEFAULT_PROMPT);
        assert_eq!(config.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(config.theme, "system");
        assert!(!config.auto_paste);
    }

    #[test]
    fn read_and_write_round_trip_and_missing_or_invalid_files_fall_back() {
        let path = temporary_path("nested/config.json");
        let config = configured();

        write_config_file(&path, &config).unwrap();
        assert_eq!(read_config_file(&path).unwrap(), config);

        fs::write(&path, "not json").unwrap();
        assert_eq!(read_config_file(&path).unwrap(), AppConfig::default());
        assert_eq!(
            read_config_file(&path.with_file_name("missing.json")).unwrap(),
            AppConfig::default()
        );

        let test_root = path.parent().unwrap().parent().unwrap();
        fs::remove_dir_all(test_root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn written_config_is_owner_readable_and_writable_only() {
        use std::os::unix::fs::PermissionsExt;

        let path = temporary_path("config.json");
        write_config_file(&path, &configured()).unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_primary_config_recovers_an_interrupted_windows_backup() {
        let path = temporary_path("config.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let backup = backup_path(&path);
        fs::write(&backup, serde_json::to_vec(&configured()).unwrap()).unwrap();

        assert_eq!(read_config_file(&path).unwrap(), configured());
        assert!(path.exists());
        assert!(!backup.exists());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
