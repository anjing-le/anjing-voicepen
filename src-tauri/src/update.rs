use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt};

const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_NOTES_CHARS: usize = 8_000;
const UPDATE_ENDPOINT: &str =
    "https://github.com/anjing-le/anjing-voicepen/releases/latest/download/latest.json";
const UPDATER_PUBKEY: Option<&str> = option_env!("VOICEPEN_UPDATER_PUBKEY");
pub const UPDATE_PROGRESS_EVENT: &str = "update-progress";
pub const UPDATE_STATUS_EVENT: &str = "update-status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStage {
    Idle,
    Checking,
    UpToDate,
    Available,
    Installing,
    RestartRequired,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateSnapshot {
    pub stage: UpdateStage,
    pub current_version: String,
    pub available_version: Option<String>,
    pub published_at: Option<String>,
    pub notes: Option<String>,
    pub message: String,
    pub can_install: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum UpdateProgress {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
        downloaded: u64,
    },
    Finished,
}

struct UpdateStateInner {
    snapshot: UpdateSnapshot,
    pending: Option<Update>,
    generation: u64,
}

impl UpdateStateInner {
    fn advance_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    fn replace_if_current(
        &mut self,
        generation: u64,
        snapshot: UpdateSnapshot,
        pending: Option<Update>,
    ) -> bool {
        if self.generation != generation {
            return false;
        }
        self.snapshot = snapshot;
        self.pending = pending;
        true
    }
}

pub struct UpdateState(Mutex<UpdateStateInner>);

impl UpdateState {
    pub fn new(current_version: impl Into<String>) -> Self {
        Self(Mutex::new(UpdateStateInner {
            snapshot: idle_snapshot(current_version.into()),
            pending: None,
            generation: 0,
        }))
    }
}

#[tauri::command]
pub fn get_update_snapshot(state: State<'_, UpdateState>) -> Result<UpdateSnapshot, String> {
    snapshot_from_state(&state)
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<UpdateSnapshot, String> {
    check_for_update_inner(&app, state.inner()).await
}

/// Starts a best-effort update check without delaying application setup. Any
/// failure is contained in `UpdateState` and never reaches the voice runtime.
pub fn spawn_startup_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<UpdateState>();
        let _ = check_for_update_inner(&app, state.inner()).await;
    });
}

async fn check_for_update_inner(
    app: &AppHandle,
    state: &UpdateState,
) -> Result<UpdateSnapshot, String> {
    let (generation, checking_snapshot) = {
        let mut inner = state
            .0
            .lock()
            .map_err(|_| "读取更新状态失败，请重启应用后重试。".to_string())?;
        if matches!(
            inner.snapshot.stage,
            UpdateStage::Checking | UpdateStage::Installing
        ) {
            return Ok(inner.snapshot.clone());
        }
        let generation = inner.advance_generation();
        inner.pending = None;
        inner.snapshot.stage = UpdateStage::Checking;
        inner.snapshot.message = "正在检查更新…".to_string();
        inner.snapshot.can_install = false;
        (generation, inner.snapshot.clone())
    };
    emit_status(app, &checking_snapshot);

    let Some(pubkey) = configured_pubkey() else {
        let (snapshot, changed) = {
            let mut inner = state
                .0
                .lock()
                .map_err(|_| "保存更新状态失败，请重启应用后重试。".to_string())?;
            let current_version = inner.snapshot.current_version.clone();
            let error = error_snapshot(
                current_version,
                "当前构建未配置更新验证公钥，无法安全检查更新。".to_string(),
            );
            let changed = inner.replace_if_current(generation, error, None);
            (inner.snapshot.clone(), changed)
        };
        if changed {
            emit_status(app, &snapshot);
        }
        return Ok(snapshot);
    };

    let result = async {
        app.updater_builder()
            .endpoints(vec![UPDATE_ENDPOINT
                .parse()
                .map_err(UpdaterError::UrlParse)?])?
            .pubkey(pubkey)
            .timeout(CHECK_TIMEOUT)
            .build()?
            .check()
            .await
    }
    .await;

    let (snapshot, changed) = {
        let mut inner = state
            .0
            .lock()
            .map_err(|_| "保存更新状态失败，请重启应用后重试。".to_string())?;
        let current_version = inner.snapshot.current_version.clone();
        let (snapshot, pending) = match result {
            Ok(Some(update)) => (available_snapshot(&update), Some(update)),
            Ok(None) => (up_to_date_snapshot(current_version), None),
            Err(error) => (
                error_snapshot(current_version, update_error_message(&error)),
                None,
            ),
        };
        let changed = inner.replace_if_current(generation, snapshot, pending);
        (inner.snapshot.clone(), changed)
    };
    if changed {
        emit_status(app, &snapshot);
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<UpdateSnapshot, String> {
    let (update, generation, installing_snapshot) = {
        let mut inner = state
            .0
            .lock()
            .map_err(|_| "读取更新状态失败，请重启应用后重试。".to_string())?;
        if inner.snapshot.stage == UpdateStage::Installing {
            return Ok(inner.snapshot.clone());
        }
        if !can_begin_install(&inner) {
            inner.snapshot.message = "没有可安装的更新，请先检查更新。".to_string();
            inner.snapshot.can_install = false;
            let snapshot = inner.snapshot.clone();
            drop(inner);
            emit_status(&app, &snapshot);
            return Ok(snapshot);
        }
        let update = inner
            .pending
            .clone()
            .expect("install gate requires pending update");
        let generation = inner.advance_generation();
        inner.snapshot.stage = UpdateStage::Installing;
        inner.snapshot.message = "正在下载更新…".to_string();
        inner.snapshot.can_install = false;
        (update, generation, inner.snapshot.clone())
    };
    emit_status(&app, &installing_snapshot);

    let mut downloaded = 0_u64;
    let progress_app = app.clone();
    let finish_app = app.clone();
    let mut started = false;
    let result = update
        .download_and_install(
            move |chunk_length, content_length| {
                if !started {
                    started = true;
                    let _ = progress_app.emit(
                        UPDATE_PROGRESS_EVENT,
                        UpdateProgress::Started { content_length },
                    );
                }
                downloaded = downloaded.saturating_add(chunk_length as u64);
                let _ = progress_app.emit(
                    UPDATE_PROGRESS_EVENT,
                    UpdateProgress::Progress {
                        chunk_length,
                        downloaded,
                    },
                );
            },
            move || {
                let _ = finish_app.emit(UPDATE_PROGRESS_EVENT, UpdateProgress::Finished);
            },
        )
        .await;

    let (snapshot, changed) = {
        let mut inner = state
            .0
            .lock()
            .map_err(|_| "保存更新状态失败，请重启应用后重试。".to_string())?;
        let mut snapshot = inner.snapshot.clone();
        let pending = match result {
            Ok(()) => {
                snapshot.stage = UpdateStage::RestartRequired;
                snapshot.message = restart_required_message();
                snapshot.can_install = false;
                None
            }
            Err(error) => {
                snapshot.stage = UpdateStage::Error;
                snapshot.message = update_error_message(&error);
                snapshot.can_install = true;
                inner.pending.clone()
            }
        };
        let changed = inner.replace_if_current(generation, snapshot, pending);
        (inner.snapshot.clone(), changed)
    };
    if changed {
        emit_status(&app, &snapshot);
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn dismiss_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<UpdateSnapshot, String> {
    let mut inner = state
        .0
        .lock()
        .map_err(|_| "读取更新状态失败，请重启应用后重试。".to_string())?;
    if inner.snapshot.stage == UpdateStage::Installing {
        inner.snapshot.message = "更新正在安装，暂时不能关闭更新提示。".to_string();
        let snapshot = inner.snapshot.clone();
        drop(inner);
        emit_status(&app, &snapshot);
        return Ok(snapshot);
    }
    inner.advance_generation();
    let current_version = inner.snapshot.current_version.clone();
    inner.snapshot = idle_snapshot(current_version);
    inner.pending = None;
    let snapshot = inner.snapshot.clone();
    drop(inner);
    emit_status(&app, &snapshot);
    Ok(snapshot)
}

fn snapshot_from_state(state: &UpdateState) -> Result<UpdateSnapshot, String> {
    state
        .0
        .lock()
        .map(|inner| inner.snapshot.clone())
        .map_err(|_| "读取更新状态失败，请重启应用后重试。".to_string())
}

fn emit_status(app: &AppHandle, snapshot: &UpdateSnapshot) {
    let _ = app.emit(UPDATE_STATUS_EVENT, snapshot.clone());
}

fn install_stage_allowed(stage: UpdateStage, can_install: bool, has_pending: bool) -> bool {
    has_pending && (stage == UpdateStage::Available || (stage == UpdateStage::Error && can_install))
}

fn can_begin_install(inner: &UpdateStateInner) -> bool {
    install_stage_allowed(
        inner.snapshot.stage,
        inner.snapshot.can_install,
        inner.pending.is_some(),
    )
}

fn idle_snapshot(current_version: String) -> UpdateSnapshot {
    UpdateSnapshot {
        stage: UpdateStage::Idle,
        current_version,
        available_version: None,
        published_at: None,
        notes: None,
        message: "可检查 GitHub Releases 中的正式版本。".to_string(),
        can_install: false,
    }
}

fn up_to_date_snapshot(current_version: String) -> UpdateSnapshot {
    UpdateSnapshot {
        stage: UpdateStage::UpToDate,
        current_version,
        available_version: None,
        published_at: None,
        notes: None,
        message: "当前已是最新版本。".to_string(),
        can_install: false,
    }
}

fn available_snapshot(update: &Update) -> UpdateSnapshot {
    UpdateSnapshot {
        stage: UpdateStage::Available,
        current_version: update.current_version.clone(),
        available_version: Some(update.version.clone()),
        published_at: update.date.map(|date| date.to_string()),
        notes: sanitize_notes(update.body.as_deref()),
        message: format!("发现新版本 {}。", update.version),
        can_install: true,
    }
}

fn error_snapshot(current_version: String, message: String) -> UpdateSnapshot {
    UpdateSnapshot {
        stage: UpdateStage::Error,
        current_version,
        available_version: None,
        published_at: None,
        notes: None,
        message,
        can_install: false,
    }
}

fn configured_pubkey() -> Option<&'static str> {
    UPDATER_PUBKEY.map(str::trim).filter(|key| !key.is_empty())
}

fn restart_required_message() -> String {
    #[cfg(target_os = "macos")]
    {
        "更新已安装。请退出并重新打开 VoicePen 以使用新版本。".to_string()
    }
    #[cfg(not(target_os = "macos"))]
    {
        "更新已安装，重新打开 VoicePen 后生效。".to_string()
    }
}

fn sanitize_notes(notes: Option<&str>) -> Option<String> {
    let notes = notes?.trim();
    if notes.is_empty() {
        return None;
    }

    let mut sanitized = String::with_capacity(notes.len().min(MAX_NOTES_CHARS));
    for character in notes.chars().take(MAX_NOTES_CHARS) {
        if character == '\n' || character == '\t' || !character.is_control() {
            sanitized.push(character);
        }
    }
    let was_truncated = notes.chars().count() > MAX_NOTES_CHARS;
    let sanitized = sanitized.trim();
    if sanitized.is_empty() {
        None
    } else if was_truncated {
        Some(format!("{sanitized}\n\n…（更新说明已截断）"))
    } else {
        Some(sanitized.to_string())
    }
}

fn update_error_message(error: &UpdaterError) -> String {
    match error {
        UpdaterError::EmptyEndpoints => "更新服务尚未配置。".to_string(),
        UpdaterError::ReleaseNotFound => "暂时无法获取更新信息，请稍后重试。".to_string(),
        UpdaterError::UnsupportedArch | UpdaterError::UnsupportedOs => {
            "当前系统或处理器架构暂不支持自动更新。".to_string()
        }
        UpdaterError::TargetNotFound(_) | UpdaterError::TargetsNotFound(_) => {
            "新版本尚未提供适用于当前设备的安装包。".to_string()
        }
        UpdaterError::InsecureTransportProtocol => "更新服务地址不安全，已拒绝连接。".to_string(),
        UpdaterError::Minisign(_) | UpdaterError::Base64(_) | UpdaterError::SignatureUtf8(_) => {
            "更新包签名验证失败，已停止安装。".to_string()
        }
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => {
            "网络连接失败，请检查网络后重试。".to_string()
        }
        UpdaterError::AuthenticationFailed => "安装授权被取消或失败。".to_string(),
        UpdaterError::PackageInstallFailed | UpdaterError::InvalidUpdaterFormat => {
            "更新未完成，请确认当前版本可正常启动后重试。".to_string()
        }
        _ => "更新未完成，请确认当前版本可正常启动后重试。".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_snapshot_is_idle_and_not_installable() {
        let state = UpdateState::new("0.1.0");
        let snapshot = snapshot_from_state(&state).unwrap();

        assert_eq!(snapshot.stage, UpdateStage::Idle);
        assert_eq!(snapshot.current_version, "0.1.0");
        assert_eq!(snapshot.message, "可检查 GitHub Releases 中的正式版本。");
        assert!(!snapshot.can_install);
    }

    #[test]
    fn stale_generation_cannot_replace_a_newer_snapshot() {
        let mut inner = UpdateStateInner {
            snapshot: idle_snapshot("0.1.0".to_string()),
            pending: None,
            generation: 4,
        };
        let stale = up_to_date_snapshot("0.1.0".to_string());

        assert!(!inner.replace_if_current(3, stale, None));
        assert_eq!(inner.snapshot.stage, UpdateStage::Idle);
        assert_eq!(inner.generation, 4);
    }

    #[test]
    fn advancing_generation_invalidates_an_in_flight_check() {
        let mut inner = UpdateStateInner {
            snapshot: idle_snapshot("0.1.0".to_string()),
            pending: None,
            generation: 9,
        };
        let check_generation = inner.advance_generation();
        let dismiss_generation = inner.advance_generation();

        assert_ne!(check_generation, dismiss_generation);
        assert!(!inner.replace_if_current(
            check_generation,
            up_to_date_snapshot("0.1.0".to_string()),
            None,
        ));
    }

    #[test]
    fn install_gate_requires_authoritative_stage_and_pending_update() {
        assert!(install_stage_allowed(UpdateStage::Available, true, true));
        assert!(install_stage_allowed(UpdateStage::Error, true, true));

        for stage in [
            UpdateStage::Idle,
            UpdateStage::Checking,
            UpdateStage::UpToDate,
            UpdateStage::Installing,
            UpdateStage::RestartRequired,
        ] {
            assert!(!install_stage_allowed(stage, true, true));
        }
        assert!(!install_stage_allowed(UpdateStage::Available, true, false));
        assert!(!install_stage_allowed(UpdateStage::Error, false, true));
    }

    #[test]
    fn notes_are_trimmed_and_control_characters_are_removed() {
        assert_eq!(
            sanitize_notes(Some("  新增\u{0000}语音功能\n修复问题\t  ")),
            Some("新增语音功能\n修复问题".to_string())
        );
        assert_eq!(sanitize_notes(Some(" \r\u{0007} ")), None);
    }

    #[test]
    fn long_notes_are_unicode_safely_truncated() {
        let notes = "更".repeat(MAX_NOTES_CHARS + 5);
        let sanitized = sanitize_notes(Some(&notes)).unwrap();

        assert_eq!(
            sanitized.chars().take(MAX_NOTES_CHARS).count(),
            MAX_NOTES_CHARS
        );
        assert!(sanitized
            .chars()
            .take(MAX_NOTES_CHARS)
            .all(|character| character == '更'));
        assert!(sanitized.ends_with("…（更新说明已截断）"));
    }

    #[test]
    fn signature_errors_do_not_expose_upstream_details() {
        let error = UpdaterError::SignatureUtf8("secret signature body".to_string());
        let message = update_error_message(&error);

        assert_eq!(message, "更新包签名验证失败，已停止安装。");
        assert!(!message.contains("secret"));
    }

    #[test]
    fn update_endpoint_is_the_single_public_release_manifest() {
        assert_eq!(
            UPDATE_ENDPOINT,
            "https://github.com/anjing-le/anjing-voicepen/releases/latest/download/latest.json"
        );
        assert!(UPDATE_ENDPOINT.starts_with("https://"));
    }

    #[test]
    fn progress_events_match_the_frontend_contract() {
        assert_eq!(
            serde_json::to_value(UpdateProgress::Started {
                content_length: Some(2048),
            })
            .unwrap(),
            serde_json::json!({
                "event": "Started",
                "data": { "contentLength": 2048 }
            })
        );
        assert_eq!(
            serde_json::to_value(UpdateProgress::Progress {
                chunk_length: 512,
                downloaded: 1024,
            })
            .unwrap(),
            serde_json::json!({
                "event": "Progress",
                "data": { "chunkLength": 512, "downloaded": 1024 }
            })
        );
        assert_eq!(
            serde_json::to_value(UpdateProgress::Finished).unwrap(),
            serde_json::json!({ "event": "Finished" })
        );
    }
}
