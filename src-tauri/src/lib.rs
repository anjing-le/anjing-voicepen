use std::{
    io::Cursor,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::Color,
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

mod config;
mod diagnostics;
mod provider;
mod runtime;

use config::{config_path, read_config_file, write_config_file, AppConfig, ConfigPayload};
use diagnostics::{test_llm, test_stt, DiagnosticResult};
use provider::OpenAiCompatibleProvider;
use runtime::{OperationToken, RuntimeSnapshot, RuntimeStage, RuntimeState, ShortcutDecision};

struct VoicePenState {
    config: Mutex<AppConfig>,
    recorder: Mutex<Recorder>,
    runtime: Mutex<RuntimeState>,
    registered_shortcut: Mutex<Option<String>>,
    shortcut_action: Mutex<()>,
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
        let device = host
            .default_input_device()
            .ok_or("未找到可用麦克风，请检查设备连接与系统麦克风权限。")?;
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
                        push_mono_samples(&samples, data, channels, |s| s as f32 / i16::MAX as f32);
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

        stream.play().map_err(|e| format!("启动录音失败：{e}"))?;

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
            return Err("没有收到音频数据，请检查麦克风权限、默认输入设备或设备占用。".to_string());
        }

        let duration_ms = ((raw_samples.len() as f64 / self.sample_rate as f64) * 1000.0) as u64;
        if duration_ms < 250 {
            return Err("录音太短，请至少说半秒左右。".to_string());
        }

        let bytes = encode_wav(&raw_samples, self.sample_rate)?;
        Ok(RecordedWav { bytes, duration_ms })
    }
}

fn push_mono_samples<T, F>(samples: &Arc<Mutex<Vec<f32>>>, data: &[T], channels: usize, convert: F)
where
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

    let previous_config = state
        .config
        .lock()
        .map_err(|e| format!("保存配置失败：{e}"))?
        .clone();
    let mut registered = state
        .registered_shortcut
        .lock()
        .map_err(|e| format!("注册快捷键失败：{e}"))?;
    let shortcut_changed = registered.as_deref() != Some(config.shortcut.as_str());
    let previous_shortcut = registered.clone();
    let new_shortcut = config
        .shortcut
        .parse::<Shortcut>()
        .map_err(|e| format!("快捷键格式无效：{e}"))?;

    if shortcut_changed {
        if let Some(old_text) = previous_shortcut.as_deref() {
            let old_shortcut = old_text
                .parse::<Shortcut>()
                .map_err(|e| format!("旧快捷键状态无效：{e}"))?;
            app.global_shortcut()
                .unregister(old_shortcut)
                .map_err(|e| format!("停用旧快捷键 {old_text} 失败：{e}"))?;
        }
        if let Err(error) = app.global_shortcut().register(new_shortcut) {
            let rollback =
                restore_shortcut(&app, previous_shortcut.as_deref(), &config.shortcut, false);
            return Err(format!(
                "注册全局快捷键 {} 失败：{error}{}",
                config.shortcut, rollback
            ));
        }
    }

    if let Err(error) = write_config_file(&state.config_path, &config) {
        if shortcut_changed {
            let rollback =
                restore_shortcut(&app, previous_shortcut.as_deref(), &config.shortcut, true);
            return Err(format!("{error}{rollback}"));
        }
        return Err(error);
    }

    let update_result = state.config.lock().map(|mut current| {
        *current = config.clone();
    });
    if let Err(error) = update_result {
        let file_rollback = write_config_file(&state.config_path, &previous_config)
            .err()
            .map(|rollback| format!("；恢复旧配置文件失败：{rollback}"))
            .unwrap_or_default();
        let shortcut_rollback = if shortcut_changed {
            restore_shortcut(&app, previous_shortcut.as_deref(), &config.shortcut, true)
        } else {
            String::new()
        };
        return Err(format!(
            "保存配置失败：{error}{file_rollback}{shortcut_rollback}"
        ));
    }

    if shortcut_changed {
        *registered = Some(config.shortcut.clone());
    }
    drop(registered);

    let snapshot = state.runtime.lock().ok().map(|mut runtime| {
        runtime.update_config(
            config.is_configured(),
            config.shortcut.clone(),
            config.auto_paste,
            config.theme.clone(),
        );
        runtime.snapshot()
    });
    if let Some(snapshot) = snapshot {
        publish_snapshot(&app, snapshot.clone());
        let _ = app.emit("voicepen-config-saved", snapshot);
    }

    Ok(config_payload(&state, config))
}

fn restore_shortcut(
    app: &AppHandle,
    old_text: Option<&str>,
    new_text: &str,
    new_is_registered: bool,
) -> String {
    let mut failures = Vec::new();
    if new_is_registered {
        if let Ok(new_shortcut) = new_text.parse::<Shortcut>() {
            if let Err(error) = app.global_shortcut().unregister(new_shortcut) {
                failures.push(format!("停用新快捷键失败：{error}"));
            }
        }
    }
    if let Some(old_text) = old_text {
        match old_text.parse::<Shortcut>() {
            Ok(old_shortcut) => {
                if let Err(error) = app.global_shortcut().register(old_shortcut) {
                    failures.push(format!("恢复旧快捷键 {old_text} 失败：{error}"));
                }
            }
            Err(error) => failures.push(format!("解析旧快捷键失败：{error}")),
        }
    }
    if failures.is_empty() {
        String::new()
    } else {
        format!("；{}", failures.join("；"))
    }
}

#[tauri::command]
fn get_runtime_snapshot(state: State<'_, SharedState>) -> Result<RuntimeSnapshot, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("读取状态失败：{e}"))?
        .clone();
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|e| format!("读取状态失败：{e}"))?;
    runtime.update_config(
        config.is_configured(),
        config.shortcut,
        config.auto_paste,
        config.theme,
    );
    Ok(runtime.snapshot())
}

#[tauri::command]
fn show_settings(app: AppHandle) -> Result<(), String> {
    show_settings_window(&app)
}

#[tauri::command]
fn hide_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window
            .hide()
            .map_err(|e| format!("隐藏设置窗口失败：{e}"))?;
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

#[tauri::command]
fn open_microphone_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return open_system_settings(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
    );

    #[cfg(target_os = "windows")]
    return open_system_settings("ms-settings:privacy-microphone");

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Err("请在系统设置中手动打开麦克风权限。".to_string())
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return open_system_settings(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    );

    #[cfg(not(target_os = "macos"))]
    Err("当前平台不需要 macOS 辅助功能设置。".to_string())
}

#[cfg(target_os = "macos")]
fn open_system_settings(target: &str) -> Result<(), String> {
    let status = Command::new("open")
        .arg(target)
        .status()
        .map_err(|error| format!("打开系统设置失败：{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("系统设置未能打开，请手动前往隐私与安全性设置。".to_string())
    }
}

#[cfg(target_os = "windows")]
fn open_system_settings(target: &str) -> Result<(), String> {
    let status = Command::new("explorer.exe")
        .arg(target)
        .status()
        .map_err(|error| format!("打开系统设置失败：{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("系统设置未能打开，请手动前往麦克风隐私设置。".to_string())
    }
}

#[tauri::command]
async fn test_stt_connection(
    state: State<'_, SharedState>,
    config: AppConfig,
) -> Result<DiagnosticResult, String> {
    Ok(test_stt(&state.client, &config.normalize()).await)
}

#[tauri::command]
async fn test_llm_connection(
    state: State<'_, SharedState>,
    config: AppConfig,
) -> Result<DiagnosticResult, String> {
    Ok(test_llm(&state.client, &config.normalize()).await)
}

fn config_payload(state: &SharedState, config: AppConfig) -> ConfigPayload {
    ConfigPayload::new(config, &state.config_path)
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

    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("注册全局快捷键 {shortcut_text} 失败：{e}"))?;

    if let Some(old_text) = registered.as_deref() {
        if let Ok(old_shortcut) = old_text.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(old_shortcut);
        }
    }
    *registered = Some(shortcut_text.to_string());
    Ok(())
}

fn handle_shortcut(app: AppHandle, state: SharedState) {
    let _action_guard = match state.shortcut_action.lock() {
        Ok(guard) => guard,
        Err(err) => {
            reject_runtime(&app, &state, format!("访问快捷键状态失败：{err}"));
            return;
        }
    };
    let config = match state.config.lock() {
        Ok(config) => config.clone(),
        Err(err) => {
            reject_runtime(&app, &state, format!("读取配置失败：{err}"));
            return;
        }
    };

    if !config.is_configured() {
        reject_runtime(&app, &state, "请先完成 VoicePen 配置。");
        let _ = show_settings_window(&app);
        return;
    }

    let decision = match state.runtime.lock() {
        Ok(mut runtime) => runtime.handle_shortcut(),
        Err(err) => {
            reject_runtime(&app, &state, format!("访问运行状态失败：{err}"));
            return;
        }
    };

    match decision {
        ShortcutDecision::IgnoreBusy(stage) => {
            let message = match stage {
                RuntimeStage::Transcribing => "正在转写，请稍候。",
                RuntimeStage::Polishing => "正在润色，请稍候。",
                _ => "VoicePen 正在处理，请稍候。",
            };
            publish_current_runtime(&app, &state, Some(message));
        }
        ShortcutDecision::StartRecording(token) => {
            let result = state
                .recorder
                .lock()
                .map_err(|e| format!("访问录音状态失败：{e}"))
                .and_then(|mut recorder| recorder.start());
            match result {
                Ok(()) => publish_current_runtime(&app, &state, None),
                Err(error) => fail_operation(&app, &state, token, error, None, None),
            }
        }
        ShortcutDecision::StopRecording(token) => {
            let recorded = match state
                .recorder
                .lock()
                .map_err(|e| format!("访问录音状态失败：{e}"))
                .and_then(|mut recorder| recorder.stop())
            {
                Ok(recorded) => recorded,
                Err(error) => {
                    fail_operation(&app, &state, token, error, None, None);
                    return;
                }
            };

            let message = format!(
                "正在转写 {:.1} 秒语音。",
                recorded.duration_ms as f64 / 1000.0
            );
            let transitioned = state
                .runtime
                .lock()
                .ok()
                .and_then(|mut runtime| runtime.begin_transcribing(token, message).ok())
                .is_some();
            if !transitioned {
                return;
            }
            publish_current_runtime(&app, &state, None);
            tauri::async_runtime::spawn(run_voice_pipeline(
                app,
                Arc::clone(&state),
                config,
                token,
                recorded.bytes,
            ));
        }
    }
}

async fn run_voice_pipeline(
    app: AppHandle,
    state: SharedState,
    config: AppConfig,
    token: OperationToken,
    wav_bytes: Vec<u8>,
) {
    let provider = OpenAiCompatibleProvider::new(&state.client);
    let transcript = match provider
        .transcribe(
            &config.stt_base_url,
            &config.stt_api_key,
            &config.stt_model,
            wav_bytes,
        )
        .await
    {
        Ok(text) => text,
        Err(err) => {
            fail_operation(&app, &state, token, err.user_message(), None, None);
            return;
        }
    };

    let transitioned = state
        .runtime
        .lock()
        .ok()
        .and_then(|mut runtime| runtime.begin_polishing(token, transcript.clone()).ok())
        .is_some();
    if !transitioned {
        return;
    }
    publish_current_runtime(&app, &state, None);

    let polished = match provider
        .polish(
            &config.llm_base_url,
            &config.llm_api_key,
            &config.llm_model,
            &config.polish_prompt,
            &transcript,
        )
        .await
    {
        Ok(text) => text,
        Err(err) => {
            fail_operation(
                &app,
                &state,
                token,
                err.user_message(),
                Some(transcript),
                None,
            );
            return;
        }
    };

    {
        let mut runtime = match state.runtime.lock() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        if runtime.claim_completion(token).is_err() {
            return;
        }
    }

    if let Err(error) = write_clipboard(&polished) {
        fail_operation(
            &app,
            &state,
            token,
            error,
            Some(transcript.clone()),
            Some(polished.clone()),
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

    let snapshot = state.runtime.lock().ok().and_then(|mut runtime| {
        runtime.complete(token, message, polished).ok()?;
        Some(runtime.snapshot())
    });
    if let Some(snapshot) = snapshot {
        publish_snapshot(&app, snapshot);
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
        let mut command = Command::new("osascript");
        command.args([
            "-e",
            "tell application \"System Events\" to keystroke \"v\" using command down",
        ]);
        let status = run_command_with_timeout(&mut command, Duration::from_secs(10))?;
        if status.success() {
            return Ok(());
        }
        Err("macOS 拒绝模拟按键，请在系统设置中授予辅助功能权限。".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            "$wshell = New-Object -ComObject wscript.shell; $wshell.SendKeys('^v')",
        ]);
        let status = run_command_with_timeout(&mut command, Duration::from_secs(10))?;
        if status.success() {
            return Ok(());
        }
        return Err("Windows 模拟粘贴失败，请手动粘贴剪贴板内容。".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("sh");
        command.args(["-lc", "command -v xdotool >/dev/null && xdotool key ctrl+v"]);
        let status = run_command_with_timeout(&mut command, Duration::from_secs(10))?;
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

fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<std::process::ExitStatus, String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法触发系统粘贴：{error}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                let cleanup = terminate_child(&mut child);
                return Err(format!("等待系统粘贴失败：{error}{cleanup}"));
            }
        }
        if Instant::now() >= deadline {
            let cleanup = terminate_child(&mut child);
            return Err(format!("系统粘贴响应超时，请手动粘贴剪贴板内容。{cleanup}"));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn terminate_child(child: &mut std::process::Child) -> String {
    match child.kill() {
        Ok(()) => child
            .wait()
            .err()
            .map(|error| format!("；回收粘贴进程失败：{error}"))
            .unwrap_or_default(),
        Err(error) => format!("；终止粘贴进程失败：{error}"),
    }
}

fn fail_operation(
    app: &AppHandle,
    state: &SharedState,
    token: OperationToken,
    message: impl Into<String>,
    transcript: Option<String>,
    polished: Option<String>,
) {
    let changed = state
        .runtime
        .lock()
        .ok()
        .and_then(|mut runtime| runtime.fail(token, message, transcript, polished).ok())
        .is_some();
    if changed {
        publish_current_runtime(app, state, None);
    }
}

fn reject_runtime(app: &AppHandle, state: &SharedState, message: impl Into<String>) {
    if let Ok(mut runtime) = state.runtime.lock() {
        runtime.reject(message);
    }
    publish_current_runtime(app, state, None);
}

fn publish_current_runtime(app: &AppHandle, state: &SharedState, message_override: Option<&str>) {
    let Some(mut snapshot) = state.runtime.lock().ok().map(|runtime| runtime.snapshot()) else {
        return;
    };
    if let Some(message) = message_override {
        snapshot.message = message.to_string();
    }
    publish_snapshot(app, snapshot);
}

fn publish_snapshot(app: &AppHandle, snapshot: RuntimeSnapshot) {
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
        let window = WebviewWindowBuilder::new(app, "settings", frontend_url("settings"))
            .title("VoicePen 设置")
            .inner_size(720.0, 760.0)
            .min_inner_size(620.0, 620.0)
            .center()
            .decorations(true)
            .transparent(false)
            .resizable(true)
            .build()
            .map_err(|e| format!("创建设置窗口失败：{e}"))?;
        let window_to_hide = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_to_hide.hide();
            }
        });
        window
    };

    window
        .show()
        .map_err(|e| format!("显示设置窗口失败：{e}"))?;
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
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("missing app icon"),
        )
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
    let (config, config_load_error) = match read_config_file(&config_path) {
        Ok(config) => (config, None),
        Err(error) => (AppConfig::default(), Some(error)),
    };
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .expect("无法初始化 HTTP 客户端");
    let state = Arc::new(VoicePenState {
        runtime: Mutex::new(RuntimeState::new(
            config.is_configured(),
            config.shortcut.clone(),
            config.auto_paste,
            config.theme.clone(),
        )),
        config: Mutex::new(config),
        recorder: Mutex::new(Recorder::default()),
        registered_shortcut: Mutex::new(None),
        shortcut_action: Mutex::new(()),
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
            paste_clipboard,
            test_stt_connection,
            test_llm_connection,
            open_microphone_settings,
            open_accessibility_settings
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
            let mut startup_errors = Vec::new();
            if let Err(error) = register_shortcut(&handle, &state, &config.shortcut) {
                startup_errors.push(error);
            }
            if let Some(error) = config_load_error.as_deref() {
                startup_errors.push(format!("恢复本地配置失败：{error}。请检查配置目录后重试。"));
            }
            if !startup_errors.is_empty() {
                reject_runtime(&handle, &state, startup_errors.join("；"));
                let _ = show_settings_window(&handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("VoicePen failed to run");
}
