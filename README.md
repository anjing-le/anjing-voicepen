<p align="center">
  <img src="./assets/voicepen-cover.png" alt="VoicePen cover" width="860" />
</p>

<h1 align="center">VoicePen · 声笔</h1>

<p align="center">
  Press a shortcut, speak, polish, paste.
</p>

<p align="center">
  <strong>语音输入 → 转写 → 润色 → 复制 / 粘贴</strong>
</p>

---

VoicePen 是一个很轻的桌面语音输入工具。它只做一件事：把你说的话转成自然、清晰、可以直接发送的文字。

它不是聊天助手，不是桌宠，不是知识库，也不是复杂工作台。

项目以 macOS 和 Windows 为正式目标平台。AI 服务采用 BYOK（Bring Your Own Key）：用户选择并配置自己的 OpenAI-compatible STT / LLM 服务，不需要 VoicePen 账号或云端数据库。

> 当前仓库仍处于早期开发阶段，尚未发布经过双端验收的安装包。正式版本将通过 GitHub Releases 发布，并在应用内提供由用户确认的单通道更新。

## 1. 安装依赖

```bash
npm install
```

## 2. 启动开发版

```bash
npm run tauri:dev
```

启动后桌面会出现一个半透明小胶囊。第一次点击它，填写你自己的 STT / LLM 配置：

- STT Base URL / API Key / Model
- LLM Base URL / API Key / Model
- 润色 Prompt
- 快捷键，默认 `Alt+Shift+V`
- 是否自动粘贴
- 皮肤：`light` / `dark` / `system`

配置仅保存在本机，VoicePen 项目不会收集你的 API Key；发起 AI 请求时，Key 会发送给你配置的服务。切勿将真实 Key 提交到仓库或公开日志。

## 3. 开始使用

按一次 `Alt+Shift+V` 开始录音，再按一次停止录音。

VoicePen 会自动完成：

```text
录音 → STT 转写 → LLM 润色 → 复制到剪贴板
```

如果打开了自动粘贴，它会尽力粘贴到当前输入框。自动粘贴失败时，润色结果仍然会留在剪贴板里。

## 给 AI 的实现提示词

如果你想让 AI 继续维护这个项目，可以直接给它这段：

```text
这是一个 Tauri v2 + React + TypeScript 桌面 MVP，产品名 VoicePen / 声笔。
目标体验：全局快捷键启动录音，再按停止；调用 OpenAI-compatible /v1/audio/transcriptions 转写；调用 /v1/chat/completions 润色；结果复制到剪贴板，可选自动粘贴。
约束：只做语音输入润色，不做聊天助手、不做桌宠、不做知识库、不做复杂工作台。UI 要克制、轻量、工具感强。配置只保存在本地 JSON，不要账号、云同步或数据库。不要提交真实 API Key。
请优先维护现有结构：Rust 负责快捷键、录音、剪贴板、托盘、配置、浮窗；React 负责首次配置、设置页、状态浮窗和 light/dark/system 皮肤。
```

## 项目结构

```text
src/                 React UI
src-tauri/src/       Rust desktop capabilities
src-tauri/icons/     App icons
assets/              README assets
config.example.json  Local config shape, no real keys
```

## 项目方向与参与贡献

- [产品与工程章程](./docs/PROJECT_CHARTER.md)
- [架构边界](./docs/ARCHITECTURE.md)
- [发布与更新策略](./docs/RELEASE_STRATEGY.md)
- [发布操作手册](./docs/RELEASE_OPERATIONS.md)
- [macOS / Windows 验收矩阵](./docs/PLATFORM_ACCEPTANCE.md)
- [技术债与历史背景登记](./docs/TECHNICAL_DEBT.md)
- [贡献指南](./CONTRIBUTING.md)
- [变更记录](./CHANGELOG.md)

欢迎通过 Issue 报告问题、提出建议，或通过 Pull Request 贡献改进。产品保持轻量，新增功能需要符合“快捷说话、可靠成文”的核心方向。

## 常用检查

```bash
npm run build
cd src-tauri && cargo check
```

## License

[MIT](./LICENSE)
