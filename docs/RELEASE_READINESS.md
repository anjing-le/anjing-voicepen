# VoicePen 首次发布准备状态

本页是从当前代码状态到真实双端 OTA 验收的门禁清单。它不包含任何私钥或证书内容。

## 已完成

- `main` 的 macOS/Windows CI 已通过。
- 单正式通道的应用内更新检查、更新要点展示、用户确认和签名验证边界已实现。
- 手工触发、受 `release` Environment 保护且只创建 Draft 的三目标构建流程已实现。
- 版本、Release 绑定、资产名称、更新清单和 updater 签名验证脚本已实现。
- 发布操作手册、平台矩阵与干净电脑验收协议已建立。

这些项目只证明代码和流水线准备度，不等于签名产物或真实设备验收完成。

## GitHub 仓库设置待完成

- 创建 `release` Environment，限制为 `main` 并配置合适的审批规则。
- 将 Actions 策略收紧为只允许完整 commit SHA 固定的 Actions。
- 启用 Immutable Releases；视维护方式配置 `v*` tag 保护规则。
- 在 `release` Environment 配置发布工作流要求的 Variables 与 Secrets，名称以 [`RELEASE_OPERATIONS.md`](./RELEASE_OPERATIONS.md) 为准。

## 外部信任材料待完成

- 生成 updater keypair，离线加密备份私钥与密码，并确认公钥注入策略。
- 准备 Apple Developer ID Application 证书、App Store Connect 公证凭据和账号权限。
- 准备受 Windows 信任的代码签名证书、时间戳服务和安全保管方式。

任何私钥、P12、PFX、P8 或密码都不得提交仓库、粘贴到聊天或出现在普通 workflow artifact/log 中。

## 首次真实验收顺序

1. 完成上述仓库设置与签名材料配置。
2. 提交统一版本号和用户可读更新说明。
3. 获得明确授权后手工运行发布 workflow，生成基线版本 A 的 Draft。
4. 复核三平台签名、公证、资产、清单和 tag/commit 绑定；再次获得明确授权后 Publish。
5. 在目标干净电脑完成 A 的下载、安装、首次配置和核心闭环。
6. 用一个真实且可解释的改动准备更高版本 B，重复 Draft 复核与发布授权。
7. 在仍安装 A 的电脑执行 A → B OTA，并验证“稍后”、更新要点、签名安装、配置保留、网络失败和恢复。
8. 将证据写回 [`PLATFORM_ACCEPTANCE.md`](./PLATFORM_ACCEPTANCE.md)。

## 仍需单独授权的动作

- 创建或修改 GitHub Environment、Variables、Secrets 和仓库安全策略。
- 生成或导入真实签名私钥与证书。
- 运行候选发布 workflow、创建 tag/Draft Release。
- Publish Release 或使用任何付费签名能力。

日常阶段 commit/push 的持续授权不包含以上动作。
