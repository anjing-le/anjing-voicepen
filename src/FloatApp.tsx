import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertCircle, Check, CircleArrowDown, Loader2, Mic, Settings, Sparkles } from "lucide-react";
import { getRuntimeSnapshot, getUpdateSnapshot, showSettings } from "./tauri";
import { applyTheme } from "./theme";
import type { RuntimeSnapshot, RuntimeStage, UpdateSnapshot } from "./types";

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
  const [updateSnapshot, setUpdateSnapshot] = useState<UpdateSnapshot | null>(null);

  useEffect(() => {
    let disposed = false;
    let stopListeningUpdate: (() => void) | undefined;
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
    void (async () => {
      let receivedUpdateEvent = false;
      try {
        const unlisten = await listen<UpdateSnapshot>("update-status", (event) => {
          if (disposed) return;
          receivedUpdateEvent = true;
          setUpdateSnapshot(event.payload);
        });
        if (disposed) {
          unlisten();
          return;
        }
        stopListeningUpdate = unlisten;

        const next = await getUpdateSnapshot();
        if (!disposed && !receivedUpdateEvent) setUpdateSnapshot(next);
      } catch {
        // Update availability is supplementary; keep the voice runtime usable.
      }
    })();

    return () => {
      disposed = true;
      unlistenStatus.then((unlisten) => unlisten());
      unlistenConfig.then((unlisten) => unlisten());
      stopListeningUpdate?.();
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
    const updateAvailable = updateSnapshot?.stage === "available";
    const showUpdate = updateAvailable && (stage === "idle" || !configured);
    if (showUpdate) {
      return {
        stage,
        configured,
        Icon: CircleArrowDown,
        label: "可更新",
        detail: `v${updateSnapshot.available_version ?? "新版"}`,
        updateAvailable: true,
      };
    }
    const Icon = iconFor(stage, configured);
    const label = configured ? stageLabel[stage] : "配置";
    const detail = configured ? current?.shortcut ?? "Alt+Shift+V" : "VoicePen";
    return { stage, configured, Icon, label, detail, updateAvailable: false };
  }, [snapshot, updateSnapshot]);

  return (
    <button
      className={`float-pill is-${view.stage} ${view.configured ? "is-configured" : "is-empty"} ${view.updateAvailable ? "has-update" : ""}`}
      onClick={() => void showSettings()}
      aria-label={view.updateAvailable ? "发现新版本，打开 VoicePen 设置查看" : "打开 VoicePen 设置"}
      title={view.updateAvailable ? "发现新版本，点击查看更新要点" : snapshot?.message || "VoicePen"}
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
