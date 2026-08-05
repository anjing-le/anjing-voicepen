import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, ConfigPayload, DiagnosticResult, RuntimeSnapshot } from "./types";

export function getConfig(): Promise<ConfigPayload> {
  return invoke("get_config");
}

export function saveConfig(config: AppConfig): Promise<ConfigPayload> {
  return invoke("save_config", { config });
}

export function testSttConnection(config: AppConfig): Promise<DiagnosticResult> {
  return invoke("test_stt_connection", { config });
}

export function testLlmConnection(config: AppConfig): Promise<DiagnosticResult> {
  return invoke("test_llm_connection", { config });
}

export function getRuntimeSnapshot(): Promise<RuntimeSnapshot> {
  return invoke("get_runtime_snapshot");
}

export function showSettings(): Promise<void> {
  return invoke("show_settings");
}

export function hideSettings(): Promise<void> {
  return invoke("hide_settings");
}

export function copyText(text: string): Promise<void> {
  return invoke("copy_text", { text });
}

export function pasteClipboard(): Promise<void> {
  return invoke("paste_clipboard");
}

export function openMicrophoneSettings(): Promise<void> {
  return invoke("open_microphone_settings");
}

export function openAccessibilitySettings(): Promise<void> {
  return invoke("open_accessibility_settings");
}
