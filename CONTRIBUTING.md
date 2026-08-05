# Contributing to VoicePen

感谢你愿意帮助改进 VoicePen。项目的核心目标是提供轻量、可靠的桌面 AI 语音输入体验，而不是扩展成聊天助手、知识库或复杂工作台。

## 开始之前

1. 阅读 [项目章程](./docs/PROJECT_CHARTER.md) 和 [架构说明](./docs/ARCHITECTURE.md)。
2. Bug 和功能建议优先创建 Issue，描述系统版本、复现步骤、期望与实际行为。
3. 不要在 Issue、日志、截图、测试或提交中包含真实 API Key、证书或用户语音内容。

## 本地开发

```bash
npm install
npm run tauri:dev
```

提交改动前至少执行：

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

涉及录音、全局快捷键、自动粘贴、权限或更新的修改，还应在受影响的真实操作系统上手动验证。编译成功不能代替平台验收。

## Pull Request

- 一个 PR 只解决一个清晰问题。
- 说明用户影响、实现边界、验证结果和未覆盖平台。
- 行为变化需要更新 README 或相应 `docs/` 文档。
- 新技术债必须登记风险、临时边界和退出条件，不能只写“以后重构”。
- 不要提交 `node_modules`、`dist`、`target`、本地配置、日志、安装包或签名材料。

维护者会根据产品边界、双端兼容性、安全性和实际验证证据决定是否合并。
