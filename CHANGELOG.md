# Changelog

VoicePen 使用语义化版本号。正式版本的用户可见更新要点以本文件和对应 GitHub Release 为准。

## [Unreleased]

### Added

- 建立产品方向、架构、发布策略、技术债和贡献规范。
- 增加配置、AI Provider 和运行状态机模块及 Rust 单元测试。

### Changed

- 语音处理使用强类型状态和 operation token，处理期间不会启动第二条管线。
- 配置写入改为临时文件提交，并为 Windows 中断写入保留恢复路径。
- 快捷键切换失败时尝试恢复旧配置与旧快捷键。
- 自动粘贴进程增加 10 秒超时，避免无限等待。

### Security

- 第三方 STT/LLM 非成功响应不再将原始正文直接展示给用户。
- Unix 配置文件权限设置失败会明确报错，不再静默忽略。

## [0.1.0] - Development baseline

`0.1.0` 是当前代码版本，尚未创建对应的正式 Git tag 或 GitHub Release。

### Added

- 全局快捷键控制录音。
- OpenAI-compatible STT 转写与 LLM 润色。
- 剪贴板复制和可选自动粘贴。
- 设置窗口、状态浮窗、系统托盘与本地配置。
