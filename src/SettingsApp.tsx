import { type FormEvent, useEffect, useMemo, useState } from "react";
import { Check, Clipboard, Copy, Moon, Save, Sun, WandSparkles, X } from "lucide-react";
import { getConfig, hideSettings, pasteClipboard, saveConfig } from "./tauri";
import { applyTheme } from "./theme";
import type { AppConfig, ThemeMode } from "./types";

const defaultConfig: AppConfig = {
  stt_base_url: "",
  stt_api_key: "",
  stt_model: "",
  llm_base_url: "",
  llm_api_key: "",
  llm_model: "",
  polish_prompt:
    "请将下面的语音转写文本润色成自然、清晰、可直接发送的中文。保留原意，不扩写，不加入新信息，不解释，只输出润色后的正文。",
  shortcut: "Alt+Shift+V",
  auto_paste: false,
  theme: "system",
};

const themeOptions: Array<{ value: ThemeMode; label: string; icon: typeof Sun }> = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: WandSparkles },
];

export default function SettingsApp() {
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [configured, setConfigured] = useState(false);
  const [configPath, setConfigPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    getConfig()
      .then((payload) => {
        setConfig(payload.config);
        setConfigured(payload.configured);
        setConfigPath(payload.config_path);
        applyTheme(payload.config.theme);
      })
      .catch((err) => setError(String(err)));
  }, []);

  useEffect(() => {
    applyTheme(config.theme);
  }, [config.theme]);

  const title = configured ? "设置" : "首次配置";
  const subtitle = useMemo(
    () => (configured ? "VoicePen 已就绪" : "填完必要项后即可用全局快捷键录音润色"),
    [configured],
  );

  function update<K extends keyof AppConfig>(key: K, value: AppConfig[K]) {
    setConfig((current) => ({ ...current, [key]: value }));
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setMessage("");
    setError("");
    try {
      const payload = await saveConfig(config);
      setConfig(payload.config);
      setConfigured(payload.configured);
      setConfigPath(payload.config_path);
      setMessage("已保存，可以直接使用快捷键。");
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function testPasteInterface() {
    setMessage("");
    setError("");
    try {
      await pasteClipboard();
      setMessage("已调用系统粘贴接口。");
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <main className="settings-shell">
      <form className="settings-panel" onSubmit={onSubmit}>
        <header className="settings-header">
          <div>
            <p className="eyebrow">VoicePen 声笔</p>
            <h1>{title}</h1>
            <p className="subtle">{subtitle}</p>
          </div>
          <button className="icon-button" type="button" onClick={() => void hideSettings()}>
            <X size={18} />
          </button>
        </header>

        <section className="section-grid">
          <div className="section-heading">
            <h2>STT</h2>
          </div>
          <label>
            <span>Base URL</span>
            <input
              value={config.stt_base_url}
              onChange={(event) => update("stt_base_url", event.target.value)}
              placeholder="https://api.openai.com"
              autoComplete="off"
            />
          </label>
          <label>
            <span>API Key</span>
            <input
              value={config.stt_api_key}
              onChange={(event) => update("stt_api_key", event.target.value)}
              placeholder="sk-..."
              type="password"
              autoComplete="off"
            />
          </label>
          <label>
            <span>Model</span>
            <input
              value={config.stt_model}
              onChange={(event) => update("stt_model", event.target.value)}
              placeholder="输入你的 STT Model"
              autoComplete="off"
            />
          </label>
        </section>

        <section className="section-grid">
          <div className="section-heading">
            <h2>LLM</h2>
          </div>
          <label>
            <span>Base URL</span>
            <input
              value={config.llm_base_url}
              onChange={(event) => update("llm_base_url", event.target.value)}
              placeholder="https://api.openai.com"
              autoComplete="off"
            />
          </label>
          <label>
            <span>API Key</span>
            <input
              value={config.llm_api_key}
              onChange={(event) => update("llm_api_key", event.target.value)}
              placeholder="sk-..."
              type="password"
              autoComplete="off"
            />
          </label>
          <label>
            <span>Model</span>
            <input
              value={config.llm_model}
              onChange={(event) => update("llm_model", event.target.value)}
              placeholder="输入你的 LLM Model"
              autoComplete="off"
            />
          </label>
        </section>

        <section className="section-grid prompt-section">
          <div className="section-heading">
            <h2>润色</h2>
          </div>
          <label className="span-3">
            <span>Prompt</span>
            <textarea
              value={config.polish_prompt}
              onChange={(event) => update("polish_prompt", event.target.value)}
              rows={4}
            />
          </label>
        </section>

        <section className="section-grid compact-section">
          <div className="section-heading">
            <h2>使用</h2>
          </div>
          <label>
            <span>快捷键</span>
            <input
              value={config.shortcut}
              onChange={(event) => update("shortcut", event.target.value)}
              placeholder="Alt+Shift+V"
              autoComplete="off"
            />
          </label>
          <label className="toggle-row">
            <span>自动粘贴</span>
            <input
              type="checkbox"
              checked={config.auto_paste}
              onChange={(event) => update("auto_paste", event.target.checked)}
            />
          </label>
          <button className="ghost-button" type="button" onClick={() => void testPasteInterface()}>
            <Clipboard size={16} />
            <span>测试粘贴接口</span>
          </button>
        </section>

        <section className="section-grid compact-section">
          <div className="section-heading">
            <h2>皮肤</h2>
          </div>
          <div className="theme-switch span-3">
            {themeOptions.map((item) => (
              <button
                className={config.theme === item.value ? "selected" : ""}
                type="button"
                key={item.value}
                onClick={() => update("theme", item.value)}
              >
                <item.icon size={16} />
                <span>{item.label}</span>
              </button>
            ))}
          </div>
        </section>

        {(message || error) && (
          <div className={`notice ${error ? "error" : "success"}`}>
            {error ? <X size={16} /> : <Check size={16} />}
            <span>{error || message}</span>
          </div>
        )}

        <footer className="settings-footer">
          <div className="path-line">
            <Copy size={14} />
            <span>{configPath || "config.json"}</span>
          </div>
          <button className="primary-button" type="submit" disabled={saving}>
            <Save size={17} />
            <span>{saving ? "保存中" : "保存配置"}</span>
          </button>
        </footer>
      </form>
    </main>
  );
}
