<p align="center">
  <img src="./assets/voicepen-cover.png" alt="VoicePen cover" width="860" />
</p>

<h1 align="center">VoicePen · 声笔</h1>

<p align="center">
  A tiny desktop voice pen: press a shortcut, speak, polish, paste.
</p>

<p align="center">
  <strong>语音输入 → 转写 → 润色 → 复制 / 粘贴</strong>
</p>

---

VoicePen 是一个开源 MVP 桌面客户端，面向“少打字，但文字更清楚”的日常输入场景。它不是聊天助手、桌宠、知识库，也不做复杂工作台；它只做一件事：把你说的话变成可以直接发送的文字。

## 核心体验

1. 启动后桌面出现一个很轻的半透明悬浮入口。
2. 第一次点击悬浮入口，进入必要配置。
3. 保存 STT / LLM 的 Base URL、API Key、Model、Prompt、快捷键和皮肤。
4. 按 `Alt+Shift+V` 开始录音，再按一次停止。
5. VoicePen 调用 STT 转写，再调用 LLM 润色。
6. 润色结果写入剪贴板，可选自动粘贴到当前输入框。

## 功能范围

- Tauri v2 + React + TypeScript
- Rust 侧负责全局快捷键、录音、剪贴板、托盘、配置文件、浮窗
- React 侧负责首次配置、设置页、状态浮窗、皮肤系统
- OpenAI-compatible `/v1/audio/transcriptions`
- OpenAI-compatible `/v1/chat/completions`
- 本地 JSON 配置，无账号、无云同步、无数据库
- 皮肤：`light` / `dark` / `system`
- 主题扩展预留：`name`、`colors.background`、`colors.text`、`colors.accent`、`radius`

## 本地配置

配置保存到系统配置目录：

- macOS: `~/Library/Application Support/VoicePen/config.json`
- Windows: `%APPDATA%/VoicePen/config.json`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/VoicePen/config.json`

仓库提供 [config.example.json](./config.example.json)，不要提交真实 API Key。

必要配置项：

```json
{
  "stt_base_url": "https://api.openai.com",
  "stt_api_key": "sk-your-stt-key",
  "stt_model": "whisper-1",
  "llm_base_url": "https://api.openai.com",
  "llm_api_key": "sk-your-llm-key",
  "llm_model": "gpt-4o-mini",
  "polish_prompt": "请将下面的语音转写文本润色成自然、清晰、可直接发送的中文。保留原意，不扩写，不加入新信息，不解释，只输出润色后的正文。",
  "shortcut": "Alt+Shift+V",
  "auto_paste": false,
  "theme": "system"
}
```

## 开发

环境要求：

- Node.js 18+
- Rust stable
- Tauri v2 对应平台依赖

安装依赖：

```bash
npm install
```

启动开发版：

```bash
npm run tauri:dev
```

前端构建：

```bash
npm run build
```

Rust 检查：

```bash
cd src-tauri
cargo check
```

## 打包

构建桌面安装包：

```bash
npm run tauri:build
```

产物位置：

```text
src-tauri/target/release/bundle/
```

macOS 首次录音会请求麦克风权限。自动粘贴依赖系统模拟按键能力，macOS 可能需要在系统设置中授予辅助功能权限；如果自动粘贴失败，VoicePen 仍会保证把润色结果复制到剪贴板。

## 仓库命名

推荐仓库名：`anjing-voicepen`

“VoicePen / 声笔”比 `anjing-typeless` 更独立，也更有画面感：声音落到笔尖，变成可发送的文字。`type less` 可以作为一句话描述，但不进入仓库名，避免和 Typeless 这个对标产品绑定过深。

## 许可证

[MIT](./LICENSE)
