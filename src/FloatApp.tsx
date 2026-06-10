import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertCircle, Check, Loader2, Mic, Settings, Sparkles } from "lucide-react";
import { getRuntimeSnapshot, showSettings } from "./tauri";
import { applyTheme } from "./theme";
import type { RuntimeSnapshot, RuntimeStage } from "./types";

const stageLabel: Record<RuntimeStage, string> = {
  idle: "待命",
  recording: "录音中",
  transcribing: "转写中",
  polishing: "润色中",
  done: "已复制",
  error: "出错",
};

function iconFor(stage: RuntimeStage, configured: boolean) {
  if (!configured) return Settings;
  if (stage === "recording") return Mic;
  if (stage === "transcribing" || stage === "polishing") return Loader2;
  if (stage === "done") return Check;
  if (stage === "error") return AlertCircle;
  return Sparkles;
}

export default function FloatApp() {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot | null>(null);

  useEffect(() => {
    let disposed = false;
    getRuntimeSnapshot()
      .then((next) => {
        if (disposed) return;
        applyTheme(next.theme);
        setSnapshot(next);
      })
      .catch(() => {});

    const unlistenStatus = listen<RuntimeSnapshot>("voicepen-status", (event) => {
      applyTheme(event.payload.theme);
      setSnapshot(event.payload);
    });
    const unlistenConfig = listen<RuntimeSnapshot>("voicepen-config-saved", (event) => {
      applyTheme(event.payload.theme);
      setSnapshot(event.payload);
    });

    return () => {
      disposed = true;
      unlistenStatus.then((unlisten) => unlisten());
      unlistenConfig.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (!snapshot || (snapshot.stage !== "done" && snapshot.stage !== "error")) return;
    const timer = window.setTimeout(() => {
      setSnapshot((current) =>
        current
          ? {
              ...current,
              stage: "idle",
              message: current.configured ? "待命" : "请先配置",
              transcript: null,
              polished: null,
            }
          : current,
      );
    }, 4200);
    return () => window.clearTimeout(timer);
  }, [snapshot]);

  const view = useMemo(() => {
    const current = snapshot;
    const stage = current?.stage ?? "idle";
    const configured = current?.configured ?? false;
    const Icon = iconFor(stage, configured);
    const label = configured ? stageLabel[stage] : "配置";
    const detail = configured ? current?.shortcut ?? "Alt+Shift+V" : "VoicePen";
    return { stage, configured, Icon, label, detail };
  }, [snapshot]);

  return (
    <button
      className={`float-pill is-${view.stage} ${view.configured ? "is-configured" : "is-empty"}`}
      onClick={() => void showSettings()}
      aria-label="打开 VoicePen 设置"
      title={snapshot?.message || "VoicePen"}
    >
      <span className="float-mark">
        <view.Icon size={17} strokeWidth={2.2} />
      </span>
      <span className="float-copy">
        <span className="float-label">{view.label}</span>
        <span className="float-detail">{view.detail}</span>
      </span>
    </button>
  );
}
