use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::Color,
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const DEFAULT_SHORTCUT: &str = "Alt+Shift+V";
const DEFAULT_PROMPT: &str =
    "请将下面的语音转写文本润色成自然、清晰、可直接发送的中文。保留原意，不扩写，不加入新信息，不解释，只输出润色后的正文。";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub stt_base_url: String,
    #[serde(default)]
    pub stt_api_key: String,
    #[serde(default = "default_stt_model")]
    pub stt_model: String,
    #[serde(default)]
    pub llm_base_url: String,
    #[serde(default)]
    pub llm_api_key: String,
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    #[serde(default = "default_prompt")]
    pub polish_prompt: String,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    #[serde(default)]
    pub auto_paste: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
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
    fn normalize(mut self) -> Self {
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

    fn is_configured(&self) -> bool {
        !self.stt_base_url.trim().is_empty()
            && !self.stt_api_key.trim().is_empty()
            && !self.stt_model.trim().is_empty()
            && !self.llm_base_url.trim().is_empty()
            && !self.llm_api_key.trim().is_empty()
            && !self.llm_model.trim().is_empty()
    }

    fn validate(&self) -> Result<(), String> {
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

#[derive(Debug, Clone, Serialize)]
pub struct ConfigPayload {
    pub config: AppConfig,
    pub configured: bool,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    pub stage: String,
    pub message: String,
    pub configured: bool,
    pub shortcut: String,
    pub auto_paste: bool,
    pub theme: String,
    pub transcript: Option<String>,
    pub polished: Option<String>,
}

impl RuntimeSnapshot {
    fn idle(config: &AppConfig) -> Self {
        Self {
            stage: "idle".to_string(),
            message: if config.is_configured() {
                "待命".to_string()
            } else {
                "请先配置".to_string()
            },
            configured: config.is_configured(),
            shortcut: config.shortcut.clone(),
            auto_paste: config.auto_paste,
            theme: config.theme.clone(),
            transcript: None,
            polished: None,
        }
    }
}

struct VoicePenState {
    config: Mutex<AppConfig>,
    recorder: Mutex<Recorder>,
    runtime: Mutex<RuntimeSnapshot>,
    registered_shortcut: Mutex<Option<String>>,
    client: reqwest::Client,
    config_path: PathBuf,
}

type SharedState = Arc<VoicePenState>;

#[derive(Default)]
struct Recorder {
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<cpal::Stream>,
    sample_rate: u32,
    is_recording: bool,
}

struct RecordedWav {
    bytes: Vec<u8>,
    duration_ms: u64,
}

impl Recorder {
    fn start(&mut self) -> Result<(), String> {
        if self.is_recording {
            return Err("已在录音中，再按一次快捷键会停止录音。".to_string());
        }

        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("未找到麦克风输入设备")?;
        let supported_config = device
            .default_input_config()
            .map_err(|e| format!("获取麦克风默认配置失败：{e}"))?;
        let sample_rate = supported_config.sample_rate();
        let channels = supported_config.channels() as usize;
        let stream_config: cpal::StreamConfig = supported_config.clone().into();
        let samples = Arc::new(Mutex::new(Vec::<f32>::new()));

        let stream = match supported_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let samples = Arc::clone(&samples);
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        push_mono_samples(&samples, data, channels, |s| s);
                    },
                    |err| eprintln!("[VoicePen] audio input error: {err}"),
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let samples = Arc::clone(&samples);
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        push_mono_samples(&samples, data, channels, |s| {
                            s as f32 / i16::MAX as f32
                        });
                    },
                    |err| eprintln!("[VoicePen] audio input error: {err}"),
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let samples = Arc::clone(&samples);
                device.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        push_mono_samples(&samples, data, channels, |s| {
                            (s as f32 - 32768.0) / 32768.0
                        });
                    },
                    |err| eprintln!("[VoicePen] audio input error: {err}"),
                    None,
                )
            }
            other => return Err(format!("暂不支持当前麦克风采样格式：{other:?}")),
        }
        .map_err(|e| format!("创建录音流失败：{e}"))?;

        stream
            .play()
            .map_err(|e| format!("启动录音失败：{e}"))?;

        self.samples = samples;
        self.stream = Some(stream);
        self.sample_rate = sample_rate;
        self.is_recording = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<RecordedWav, String> {
        if !self.is_recording {
            return Err("当前没有正在进行的录音。".to_string());
        }

        drop(self.stream.take());
        self.is_recording = false;

        let raw_samples = self
            .samples
            .lock()
            .map_err(|e| format!("读取录音数据失败：{e}"))?
            .clone();

        if self.sample_rate == 0 || raw_samples.is_empty() {
            return Err("录音数据为空，请检查麦克风权限。".to_string());
        }

        let duration_ms = ((raw_samples.len() as f64 / self.sample_rate as f64) * 1000.0) as u64;
        if duration_ms < 250 {
            return Err("录音太短，请至少说半秒左右。".to_string());
        }

        let bytes = encode_wav(&raw_samples, self.sample_rate)?;
        Ok(RecordedWav { bytes, duration_ms })
    }
}

fn push_mono_samples<T, F>(
    samples: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    convert: F,
) where
    T: Copy,
    F: Fn(T) -> f32,
{
    if let Ok(mut collected) = samples.lock() {
        if channels <= 1 {
            collected.extend(data.iter().map(|sample| convert(*sample)));
            return;
        }

        for frame in data.chunks(channels) {
            let sum = frame.iter().map(|sample| convert(*sample)).sum::<f32>();
            collected.push(sum / frame.len() as f32);
        }
    }
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|e| format!("WAV 初始化失败：{e}"))?;
        for sample in samples {
            let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer
                .write_sample(pcm)
                .map_err(|e| format!("写入 WAV 失败：{e}"))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("完成 WAV 文件失败：{e}"))?;
    }

    Ok(cursor.into_inner())
}

#[tauri::command]
fn get_config(state: State<'_, SharedState>) -> Result<ConfigPayload, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("读取配置失败：{e}"))?
        .clone();
    Ok(config_payload(&state, config))
}

#[tauri::command]
fn save_config(
    app: AppHandle,
    state: State<'_, SharedState>,
    config: AppConfig,
) -> Result<ConfigPayload, String> {
    let config = config.normalize();
    config.validate()?;

    write_config_file(&state.config_path, &config)?;
    {
        let mut current = state
            .config
            .lock()
            .map_err(|e| format!("保存配置失败：{e}"))?;
        *current = config.clone();
    }

    register_shortcut(&app, &state, &config.shortcut)?;
    let snapshot = RuntimeSnapshot::idle(&config);
    set_runtime_snapshot(&app, &state, snapshot.clone());
    let _ = app.emit("voicepen-config-saved", snapshot);

    Ok(config_payload(&state, config))
}

#[tauri::command]
fn get_runtime_snapshot(state: State<'_, SharedState>) -> Result<RuntimeSnapshot, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("读取状态失败：{e}"))?
        .clone();
    let mut snapshot = state
        .runtime
        .lock()
        .map_err(|e| format!("读取状态失败：{e}"))?
        .clone();
    snapshot.configured = config.is_configured();
    snapshot.shortcut = config.shortcut;
    snapshot.auto_paste = config.auto_paste;
    snapshot.theme = config.theme;
    Ok(snapshot)
}

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}

#[tauri::command]
fn hide_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.hide().map_err(|e| format!("隐藏设置窗口失败：{e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    write_clipboard(&text)
}

#[tauri::command]
fn paste_clipboard() -> Result<(), String> {
    trigger_paste()
}

fn config_payload(state: &SharedState, config: AppConfig) -> ConfigPayload {
    ConfigPayload {
        configured: config.is_configured(),
        config,
        config_path: state.config_path.display().to_string(),
    }
}

fn config_dir() -> Result<PathBuf, String> {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .ok_or("无法获取系统配置目录")?;
    Ok(base.join("VoicePen"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("config.json"))
}

fn read_config_file(path: &PathBuf) -> AppConfig {
    let Ok(raw) = fs::read_to_string(path) else {
        return AppConfig::default();
    };
    serde_json::from_str::<AppConfig>(&raw)
        .map(AppConfig::normalize)
        .unwrap_or_default()
}

fn write_config_file(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败：{e}"))?;
    }

    let data = serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败：{e}"))?;
    fs::write(path, data).map_err(|e| format!("写入配置文件失败：{e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(path, permissions);
    }

    Ok(())
}

fn register_shortcut(
    app: &AppHandle,
    state: &SharedState,
    shortcut_text: &str,
) -> Result<(), String> {
    let shortcut = shortcut_text
        .parse::<Shortcut>()
        .map_err(|e| format!("快捷键格式无效：{e}"))?;

    let mut registered = state
        .registered_shortcut
        .lock()
        .map_err(|e| format!("注册快捷键失败：{e}"))?;

    if registered.as_deref() == Some(shortcut_text) {
        return Ok(());
    }

    if let Some(old_text) = registered.take() {
        if let Ok(old_shortcut) = old_text.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(old_shortcut);
        }
    }

    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("注册全局快捷键 {shortcut_text} 失败：{e}"))?;
    *registered = Some(shortcut_text.to_string());
    Ok(())
}

fn handle_shortcut(app: AppHandle, state: SharedState) {
    let config = match state.config.lock() {
        Ok(config) => config.clone(),
        Err(err) => {
            emit_status(
                &app,
                &state,
                "error",
                format!("读取配置失败：{err}"),
                None,
                None,
            );
            return;
        }
    };

    if !config.is_configured() {
        emit_status(&app, &state, "error", "请先完成 VoicePen 配置。", None, None);
        let _ = show_settings_window(&app);
        return;
    }

    let mut recorder = match state.recorder.lock() {
        Ok(recorder) => recorder,
        Err(err) => {
            emit_status(
                &app,
                &state,
                "error",
                format!("访问录音状态失败：{err}"),
                None,
                None,
            );
            return;
        }
    };

    if !recorder.is_recording {
        match recorder.start() {
            Ok(()) => emit_status(&app, &state, "recording", "录音中，再按一次停止。", None, None),
            Err(err) => emit_status(&app, &state, "error", err, None, None),
        }
        return;
    }

    let recorded = match recorder.stop() {
        Ok(recorded) => recorded,
        Err(err) => {
            emit_status(&app, &state, "error", err, None, None);
            return;
        }
    };
    drop(recorder);

    emit_status(
        &app,
        &state,
        "transcribing",
        format!("正在转写 {:.1} 秒语音。", recorded.duration_ms as f64 / 1000.0),
        None,
        None,
    );

    tauri::async_runtime::spawn(run_voice_pipeline(app, state, config, recorded.bytes));
}

async fn run_voice_pipeline(
    app: AppHandle,
    state: SharedState,
    config: AppConfig,
    wav_bytes: Vec<u8>,
) {
    let transcript = match transcribe_openai(&state.client, &config, wav_bytes).await {
        Ok(text) => text,
        Err(err) => {
            emit_status(&app, &state, "error", err, None, None);
            return;
        }
    };

    emit_status(
        &app,
        &state,
        "polishing",
        "正在润色文字。",
        Some(transcript.clone()),
        None,
    );

    let polished = match polish_openai(&state.client, &config, &transcript).await {
        Ok(text) => text,
        Err(err) => {
            emit_status(&app, &state, "error", err, Some(transcript), None);
            return;
        }
    };

    if let Err(err) = write_clipboard(&polished) {
        emit_status(
            &app,
            &state,
            "error",
            err,
            Some(transcript),
            Some(polished),
        );
        return;
    }

    let mut message = "已复制到剪贴板。".to_string();
    if config.auto_paste {
        match trigger_paste() {
            Ok(()) => message = "已复制并尝试粘贴。".to_string(),
            Err(err) => message = format!("已复制到剪贴板；自动粘贴失败：{err}"),
        }
    }

    emit_status(
        &app,
        &state,
        "done",
        message,
        Some(transcript),
        Some(polished),
    );
}

#[derive(Debug, Deserialize)]
struct SttResponse {
    text: Option<String>,
}

async fn transcribe_openai(
    client: &reqwest::Client,
    config: &AppConfig,
    wav_bytes: Vec<u8>,
) -> Result<String, String> {
    let url = openai_endpoint(&config.stt_base_url, "audio/transcriptions");
    let file_part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("voicepen.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("准备音频请求失败：{e}"))?;
    let form = reqwest::multipart::Form::new()
        .text("model", config.stt_model.clone())
        .part("file", file_part);

    let response = client
        .post(url)
        .bearer_auth(&config.stt_api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| request_error("STT 转写请求失败", e))?;

    if !response.status().is_success() {
        return Err(http_error("STT 转写失败", response).await);
    }

    let payload = response
        .json::<SttResponse>()
        .await
        .map_err(|e| format!("解析 STT 响应失败：{e}"))?;
    let text = payload.text.unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err("STT 返回了空文本，请检查模型或音频质量。".to_string());
    }
    Ok(text)
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

async fn polish_openai(
    client: &reqwest::Client,
    config: &AppConfig,
    transcript: &str,
) -> Result<String, String> {
    let url = openai_endpoint(&config.llm_base_url, "chat/completions");
    let body = serde_json::json!({
        "model": config.llm_model,
        "messages": [
            { "role": "system", "content": config.polish_prompt },
            { "role": "user", "content": transcript }
        ],
        "temperature": 0.2
    });

    let response = client
        .post(url)
        .bearer_auth(&config.llm_api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| request_error("LLM 润色请求失败", e))?;

    if !response.status().is_success() {
        return Err(http_error("LLM 润色失败", response).await);
    }

    let payload = response
        .json::<ChatCompletionResponse>()
        .await
        .map_err(|e| format!("解析 LLM 响应失败：{e}"))?;
    let text = payload
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .unwrap_or_default()
        .trim()
        .to_string();
    if text.is_empty() {
        return Err("LLM 返回了空文本，请检查模型配置。".to_string());
    }
    Ok(text)
}

fn openai_endpoint(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{path}")
    } else {
        format!("{base}/v1/{path}")
    }
}

fn request_error(prefix: &str, err: reqwest::Error) -> String {
    if err.is_timeout() {
        format!("{prefix}：请求超时，请检查网络或服务状态。")
    } else if err.is_connect() {
        format!("{prefix}：连接失败，请检查 Base URL。")
    } else {
        format!("{prefix}：{err}")
    }
}

async fn http_error(prefix: &str, response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let body = body.trim();
    if body.is_empty() {
        format!("{prefix}（HTTP {status}）。")
    } else {
        format!("{prefix}（HTTP {status}）：{body}")
    }
}

fn write_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("剪贴板访问失败：{e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("写入剪贴板失败：{e}"))
}

fn trigger_paste() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to keystroke \"v\" using command down",
            ])
            .status()
            .map_err(|e| format!("无法触发系统粘贴：{e}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("macOS 拒绝模拟按键，请在系统设置中授予辅助功能权限。".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('^v')",
            ])
            .status()
            .map_err(|e| format!("无法触发系统粘贴：{e}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("Windows 模拟粘贴失败，请手动粘贴剪贴板内容。".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        let status = Command::new("sh")
            .args(["-lc", "command -v xdotool >/dev/null && xdotool key ctrl+v"])
            .status()
            .map_err(|e| format!("无法触发系统粘贴：{e}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("Linux 自动粘贴需要安装 xdotool，请手动粘贴剪贴板内容。".to_string());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("当前平台暂未实现自动粘贴，请手动粘贴剪贴板内容。".to_string())
    }
}

fn emit_status(
    app: &AppHandle,
    state: &SharedState,
    stage: impl Into<String>,
    message: impl Into<String>,
    transcript: Option<String>,
    polished: Option<String>,
) {
    let config = state
        .config
        .lock()
        .map(|config| config.clone())
        .unwrap_or_default();
    let snapshot = RuntimeSnapshot {
        stage: stage.into(),
        message: message.into(),
        configured: config.is_configured(),
        shortcut: config.shortcut,
        auto_paste: config.auto_paste,
        theme: config.theme,
        transcript,
        polished,
    };
    set_runtime_snapshot(app, state, snapshot);
}

fn set_runtime_snapshot(app: &AppHandle, state: &SharedState, snapshot: RuntimeSnapshot) {
    if let Ok(mut runtime) = state.runtime.lock() {
        *runtime = snapshot.clone();
    }
    let _ = app.emit("voicepen-status", snapshot);
    show_float_window(app);
}

fn frontend_url(window: &str) -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External(
            format!("http://localhost:1422?window={window}")
                .parse()
                .expect("invalid dev url"),
        )
    } else {
        WebviewUrl::App(format!("index.html?window={window}").into())
    }
}

fn create_float_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window("float").is_some() {
        return Ok(());
    }

    let (x, y) = float_position(app);
    let window = WebviewWindowBuilder::new(app, "float", frontend_url("float"))
        .title("VoicePen")
        .inner_size(190.0, 58.0)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .build()
        .map_err(|e| format!("创建浮窗失败：{e}"))?;

    let _ = window.set_shadow(false);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    Ok(())
}

fn show_float_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("float") {
        let _ = window.show();
    }
}

fn float_position(app: &AppHandle) -> (f64, f64) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let pos = monitor.position();
        let width = size.width as f64 / scale;
        let x = pos.x as f64 / scale + width - 220.0;
        let y = pos.y as f64 / scale + 140.0;
        return (x.max(16.0), y.max(16.0));
    }
    (24.0, 160.0)
}

fn show_settings_window(app: &AppHandle) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window("settings") {
        window
    } else {
        WebviewWindowBuilder::new(app, "settings", frontend_url("settings"))
            .title("VoicePen 设置")
            .inner_size(720.0, 760.0)
            .min_inner_size(620.0, 620.0)
            .center()
            .decorations(true)
            .transparent(false)
            .resizable(true)
            .build()
            .map_err(|e| format!("创建设置窗口失败：{e}"))?
    };

    window.show().map_err(|e| format!("显示设置窗口失败：{e}"))?;
    window
        .set_focus()
        .map_err(|e| format!("聚焦设置窗口失败：{e}"))?;
    Ok(())
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text("show_settings", "打开设置")
        .separator()
        .text("quit", "退出 VoicePen")
        .build()?;

    TrayIconBuilder::with_id("voicepen_tray")
        .icon(app.default_window_icon().cloned().expect("missing app icon"))
        .icon_as_template(true)
        .tooltip("VoicePen")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_settings_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_settings" => {
                let _ = show_settings_window(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = config_path().expect("无法初始化配置路径");
    let config = read_config_file(&config_path);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .expect("无法初始化 HTTP 客户端");
    let state = Arc::new(VoicePenState {
        runtime: Mutex::new(RuntimeSnapshot::idle(&config)),
        config: Mutex::new(config),
        recorder: Mutex::new(Recorder::default()),
        registered_shortcut: Mutex::new(None),
        client,
        config_path,
    });
    let shortcut_state = Arc::clone(&state);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        handle_shortcut(app.clone(), Arc::clone(&shortcut_state));
                    }
                })
                .build(),
        )
        .manage(Arc::clone(&state))
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_runtime_snapshot,
            show_settings,
            hide_settings,
            copy_text,
            paste_clipboard
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            create_float_window(&handle).expect("创建 VoicePen 浮窗失败");
            create_tray(&handle)?;

            let config = state
                .config
                .lock()
                .map(|config| config.clone())
                .unwrap_or_default();
            register_shortcut(&handle, &state, &config.shortcut)
                .expect("注册 VoicePen 全局快捷键失败");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("VoicePen failed to run");
}
