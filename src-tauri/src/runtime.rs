use serde::Serialize;
use std::fmt;

/// The stable runtime contract exposed to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum RuntimeStage {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "recording")]
    Recording,
    #[serde(rename = "transcribing")]
    Transcribing,
    #[serde(rename = "polishing")]
    Polishing,
    #[serde(rename = "polishing")]
    Completing,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error,
}

impl RuntimeStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Transcribing => "transcribing",
            Self::Polishing | Self::Completing => "polishing",
            Self::Done => "done",
            Self::Error => "error",
        }
    }
}

/// Identifies the one recording/processing operation that currently owns the runtime.
///
/// Pipeline callbacks must present this token before changing state. Resetting the
/// runtime or starting a later operation invalidates every older token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationToken(u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RuntimeSnapshot {
    pub(crate) stage: RuntimeStage,
    pub(crate) message: String,
    pub(crate) configured: bool,
    pub(crate) shortcut: String,
    pub(crate) auto_paste: bool,
    pub(crate) theme: String,
    pub(crate) transcript: Option<String>,
    pub(crate) polished: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutDecision {
    StartRecording(OperationToken),
    StopRecording(OperationToken),
    IgnoreBusy(RuntimeStage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransitionError {
    InvalidTransition {
        from: RuntimeStage,
        attempted: &'static str,
    },
    StaleOperation,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition { from, attempted } => write!(
                formatter,
                "cannot {attempted} while runtime stage is {}",
                from.as_str()
            ),
            Self::StaleOperation => formatter.write_str("operation no longer owns the runtime"),
        }
    }
}

impl std::error::Error for TransitionError {}

/// Owns the voice pipeline's state and grants execution rights to exactly one operation.
#[derive(Debug)]
pub(crate) struct RuntimeState {
    snapshot: RuntimeSnapshot,
    generation: u64,
    owner: Option<OperationToken>,
}

impl RuntimeState {
    pub(crate) fn new(configured: bool, shortcut: String, auto_paste: bool, theme: String) -> Self {
        Self {
            snapshot: RuntimeSnapshot {
                stage: RuntimeStage::Idle,
                message: idle_message(configured).to_string(),
                configured,
                shortcut,
                auto_paste,
                theme,
                transcript: None,
                polished: None,
            },
            generation: 0,
            owner: None,
        }
    }

    pub(crate) fn snapshot(&self) -> RuntimeSnapshot {
        self.snapshot.clone()
    }

    /// Applies non-operational configuration without changing execution ownership.
    pub(crate) fn update_config(
        &mut self,
        configured: bool,
        shortcut: String,
        auto_paste: bool,
        theme: String,
    ) {
        self.snapshot.configured = configured;
        self.snapshot.shortcut = shortcut;
        self.snapshot.auto_paste = auto_paste;
        self.snapshot.theme = theme;
        if self.snapshot.stage == RuntimeStage::Idle {
            self.snapshot.message = idle_message(configured).to_string();
        }
    }

    /// Converts a shortcut press into an explicit action without allowing a second pipeline.
    pub(crate) fn handle_shortcut(&mut self) -> ShortcutDecision {
        match self.snapshot.stage {
            RuntimeStage::Idle => ShortcutDecision::StartRecording(self.grant_recording()),
            RuntimeStage::Recording => ShortcutDecision::StopRecording(
                self.owner
                    .expect("recording stage must always have an operation owner"),
            ),
            RuntimeStage::Transcribing | RuntimeStage::Polishing | RuntimeStage::Completing => {
                ShortcutDecision::IgnoreBusy(self.snapshot.stage)
            }
            RuntimeStage::Done | RuntimeStage::Error => {
                self.return_to_idle();
                ShortcutDecision::StartRecording(self.grant_recording())
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn try_start_recording(&mut self) -> Result<OperationToken, TransitionError> {
        if self.snapshot.stage != RuntimeStage::Idle {
            return Err(TransitionError::InvalidTransition {
                from: self.snapshot.stage,
                attempted: "start recording",
            });
        }
        Ok(self.grant_recording())
    }

    fn grant_recording(&mut self) -> OperationToken {
        self.generation = self.generation.wrapping_add(1);
        let token = OperationToken(self.generation);
        self.owner = Some(token);
        self.replace_content(
            RuntimeStage::Recording,
            "录音中，再按一次停止。",
            None,
            None,
        );
        token
    }

    pub(crate) fn begin_transcribing(
        &mut self,
        token: OperationToken,
        message: impl Into<String>,
    ) -> Result<(), TransitionError> {
        self.require(token, RuntimeStage::Recording, "begin transcribing")?;
        self.replace_content(RuntimeStage::Transcribing, message, None, None);
        Ok(())
    }

    pub(crate) fn begin_polishing(
        &mut self,
        token: OperationToken,
        transcript: String,
    ) -> Result<(), TransitionError> {
        self.require(token, RuntimeStage::Transcribing, "begin polishing")?;
        self.replace_content(
            RuntimeStage::Polishing,
            "正在润色文字。",
            Some(transcript),
            None,
        );
        Ok(())
    }

    pub(crate) fn complete(
        &mut self,
        token: OperationToken,
        message: impl Into<String>,
        polished: String,
    ) -> Result<(), TransitionError> {
        self.require(token, RuntimeStage::Completing, "complete operation")?;
        self.snapshot.stage = RuntimeStage::Done;
        self.snapshot.message = message.into();
        self.snapshot.polished = Some(polished);
        self.owner = None;
        Ok(())
    }

    pub(crate) fn claim_completion(
        &mut self,
        token: OperationToken,
    ) -> Result<(), TransitionError> {
        self.require(token, RuntimeStage::Polishing, "claim completion")?;
        self.snapshot.stage = RuntimeStage::Completing;
        Ok(())
    }

    /// Ends the current operation while preserving any useful partial result supplied by
    /// the caller (for example, a transcript when polishing fails).
    pub(crate) fn fail(
        &mut self,
        token: OperationToken,
        message: impl Into<String>,
        transcript: Option<String>,
        polished: Option<String>,
    ) -> Result<(), TransitionError> {
        self.require_owner(token)?;
        self.replace_content(RuntimeStage::Error, message, transcript, polished);
        self.owner = None;
        Ok(())
    }

    /// Records an error that happened before an operation could acquire or retain ownership.
    pub(crate) fn reject(&mut self, message: impl Into<String>) {
        self.generation = self.generation.wrapping_add(1);
        self.owner = None;
        self.replace_content(RuntimeStage::Error, message, None, None);
    }

    /// Clears terminal/active state and invalidates outstanding asynchronous callbacks.
    pub(crate) fn return_to_idle(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.owner = None;
        self.replace_content(
            RuntimeStage::Idle,
            idle_message(self.snapshot.configured),
            None,
            None,
        );
    }

    fn require(
        &self,
        token: OperationToken,
        expected: RuntimeStage,
        attempted: &'static str,
    ) -> Result<(), TransitionError> {
        self.require_owner(token)?;
        if self.snapshot.stage != expected {
            return Err(TransitionError::InvalidTransition {
                from: self.snapshot.stage,
                attempted,
            });
        }
        Ok(())
    }

    fn require_owner(&self, token: OperationToken) -> Result<(), TransitionError> {
        if self.owner == Some(token) {
            Ok(())
        } else {
            Err(TransitionError::StaleOperation)
        }
    }

    fn replace_content(
        &mut self,
        stage: RuntimeStage,
        message: impl Into<String>,
        transcript: Option<String>,
        polished: Option<String>,
    ) {
        self.snapshot.stage = stage;
        self.snapshot.message = message.into();
        self.snapshot.transcript = transcript;
        self.snapshot.polished = polished;
    }
}

fn idle_message(configured: bool) -> &'static str {
    if configured {
        "待命"
    } else {
        "请先配置"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> RuntimeState {
        RuntimeState::new(true, "Alt+Space".into(), true, "system".into())
    }

    #[test]
    fn normal_pipeline_preserves_results_and_serialized_stage_names() {
        let mut runtime = runtime();
        let token = match runtime.handle_shortcut() {
            ShortcutDecision::StartRecording(token) => token,
            decision => panic!("unexpected decision: {decision:?}"),
        };
        assert_eq!(runtime.snapshot().stage.as_str(), "recording");
        assert_eq!(
            runtime.handle_shortcut(),
            ShortcutDecision::StopRecording(token)
        );

        runtime.begin_transcribing(token, "正在转写").unwrap();
        assert_eq!(runtime.snapshot().stage.as_str(), "transcribing");
        runtime.begin_polishing(token, "原文".into()).unwrap();
        assert_eq!(runtime.snapshot().stage.as_str(), "polishing");
        runtime.claim_completion(token).unwrap();
        runtime.complete(token, "完成", "润色结果".into()).unwrap();

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.stage.as_str(), "done");
        assert_eq!(snapshot.transcript.as_deref(), Some("原文"));
        assert_eq!(snapshot.polished.as_deref(), Some("润色结果"));
    }

    #[test]
    fn shortcut_is_ignored_during_processing() {
        let mut runtime = runtime();
        let token = runtime.try_start_recording().unwrap();
        runtime.begin_transcribing(token, "转写").unwrap();
        assert_eq!(
            runtime.handle_shortcut(),
            ShortcutDecision::IgnoreBusy(RuntimeStage::Transcribing)
        );
        runtime.begin_polishing(token, "原文".into()).unwrap();
        assert_eq!(
            runtime.handle_shortcut(),
            ShortcutDecision::IgnoreBusy(RuntimeStage::Polishing)
        );
        runtime.claim_completion(token).unwrap();
        assert_eq!(
            runtime.handle_shortcut(),
            ShortcutDecision::IgnoreBusy(RuntimeStage::Completing)
        );
    }

    #[test]
    fn invalid_transition_does_not_mutate_snapshot() {
        let mut runtime = runtime();
        let token = runtime.try_start_recording().unwrap();
        let before = runtime.snapshot();
        assert!(matches!(
            runtime.try_start_recording(),
            Err(TransitionError::InvalidTransition {
                from: RuntimeStage::Recording,
                ..
            })
        ));
        let error = runtime
            .begin_polishing(token, "过早的结果".into())
            .unwrap_err();
        assert!(matches!(
            error,
            TransitionError::InvalidTransition {
                from: RuntimeStage::Recording,
                ..
            }
        ));
        assert_eq!(runtime.snapshot(), before);
    }

    #[test]
    fn every_stage_has_the_stable_frontend_serialization() {
        let cases = [
            (RuntimeStage::Idle, "\"idle\""),
            (RuntimeStage::Recording, "\"recording\""),
            (RuntimeStage::Transcribing, "\"transcribing\""),
            (RuntimeStage::Polishing, "\"polishing\""),
            (RuntimeStage::Completing, "\"polishing\""),
            (RuntimeStage::Done, "\"done\""),
            (RuntimeStage::Error, "\"error\""),
        ];
        for (stage, expected) in cases {
            assert_eq!(serde_json::to_string(&stage).unwrap(), expected);
        }
    }

    #[test]
    fn stale_token_cannot_overwrite_a_later_operation() {
        let mut runtime = runtime();
        let stale = runtime.try_start_recording().unwrap();
        runtime.return_to_idle();
        let current = runtime.try_start_recording().unwrap();

        assert_eq!(
            runtime.fail(stale, "旧错误", None, None),
            Err(TransitionError::StaleOperation)
        );
        assert_eq!(runtime.snapshot().stage, RuntimeStage::Recording);
        runtime.begin_transcribing(current, "当前转写").unwrap();
    }

    #[test]
    fn stale_token_cannot_authorize_completion_side_effects() {
        let mut runtime = runtime();
        let stale = runtime.try_start_recording().unwrap();
        runtime.begin_transcribing(stale, "转写").unwrap();
        runtime.begin_polishing(stale, "旧原文".into()).unwrap();
        runtime.return_to_idle();

        assert_eq!(
            runtime.claim_completion(stale),
            Err(TransitionError::StaleOperation)
        );
    }

    #[test]
    fn error_preserves_partial_result_and_can_recover_to_idle() {
        let mut runtime = runtime();
        let token = runtime.try_start_recording().unwrap();
        runtime.begin_transcribing(token, "转写").unwrap();
        runtime.begin_polishing(token, "保留的原文".into()).unwrap();
        runtime
            .fail(token, "润色失败", Some("保留的原文".into()), None)
            .unwrap();

        assert_eq!(runtime.snapshot().stage, RuntimeStage::Error);
        assert_eq!(runtime.snapshot().transcript.as_deref(), Some("保留的原文"));
        runtime.return_to_idle();
        assert_eq!(runtime.snapshot().stage, RuntimeStage::Idle);
        assert_eq!(runtime.snapshot().message, "待命");
        assert_eq!(runtime.snapshot().transcript, None);
    }

    #[test]
    fn terminal_state_accepts_the_next_recording_without_a_timer_race() {
        let mut runtime = runtime();
        let token = runtime.try_start_recording().unwrap();
        runtime.fail(token, "失败", None, None).unwrap();

        let next = match runtime.handle_shortcut() {
            ShortcutDecision::StartRecording(token) => token,
            decision => panic!("unexpected decision: {decision:?}"),
        };
        assert_ne!(next, token);
        assert_eq!(runtime.snapshot().stage, RuntimeStage::Recording);
    }

    #[test]
    fn configuration_updates_do_not_release_a_busy_operation() {
        let mut runtime = runtime();
        let token = runtime.try_start_recording().unwrap();
        runtime.update_config(false, "Ctrl+Space".into(), false, "dark".into());
        assert_eq!(runtime.snapshot().stage, RuntimeStage::Recording);
        runtime.begin_transcribing(token, "仍由原操作拥有").unwrap();
    }

    #[test]
    fn rejected_preflight_error_invalidates_any_stale_owner() {
        let mut runtime = runtime();
        let stale = runtime.try_start_recording().unwrap();
        runtime.reject("配置错误");

        assert_eq!(runtime.snapshot().stage, RuntimeStage::Error);
        assert_eq!(
            runtime.fail(stale, "旧操作", None, None),
            Err(TransitionError::StaleOperation)
        );
    }
}
