export type ThemeMode = "light" | "dark" | "system";

export interface ThemeJson {
  name: string;
  colors: {
    background: string;
    text: string;
    accent: string;
  };
  radius: number;
}

export interface AppConfig {
  stt_base_url: string;
  stt_api_key: string;
  stt_model: string;
  llm_base_url: string;
  llm_api_key: string;
  llm_model: string;
  polish_prompt: string;
  shortcut: string;
  auto_paste: boolean;
  theme: ThemeMode;
}

export interface ConfigPayload {
  config: AppConfig;
  configured: boolean;
  config_path: string;
}

export type DiagnosticService = "stt" | "llm";

export interface DiagnosticResult {
  service: DiagnosticService;
  success: boolean;
  message: string;
}

export type UpdateStage =
  | "idle"
  | "checking"
  | "up_to_date"
  | "available"
  | "installing"
  | "restart_required"
  | "error";

export interface UpdateSnapshot {
  stage: UpdateStage;
  current_version: string;
  available_version: string | null;
  published_at: string | null;
  notes: string | null;
  message: string;
  can_install: boolean;
}

export type UpdateProgress =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number; downloaded: number } }
  | { event: "Finished" };

export type RuntimeStage = "idle" | "recording" | "transcribing" | "polishing" | "done" | "error";

export interface RuntimeSnapshot {
  stage: RuntimeStage;
  message: string;
  configured: boolean;
  shortcut: string;
  auto_paste: boolean;
  theme: ThemeMode;
  transcript: string | null;
  polished: string | null;
}
