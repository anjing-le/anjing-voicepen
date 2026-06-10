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
