# VoicePen 发布操作手册

本手册描述如何生成一个可复核但尚未公开的 GitHub Draft Release。Updater 产物始终使用项目密钥签名；macOS 安装产物采用 ad-hoc code seal 且不做 Apple 公证。流水线永远不会自动发布正式版本；正式发布仍需要项目负责人在复核后明确授权并在 GitHub 界面手工执行。

## 1. GitHub 仓库设置

在仓库 Settings → Environments 中创建名为 `release` 的 Environment：

- 配置 required reviewer；签名 job 只有在批准后才能读取 Environment secrets。
- 将 deployment branches 限制为 `main`。
- 可用时禁止管理员绕过保护规则。只有存在另一位维护者能够审批时才启用 prevent self-review；单维护者仓库启用它会让发布永久无法继续。
- 不要让来自 fork 的 Pull Request 使用这个 Environment。

工作流只接受从 `main` 手工触发的 `workflow_dispatch`。`GITHUB_TOKEN` 仅获得创建 Draft Release 和资产所需的 `contents: write`；普通 CI 只有 `contents: read`，且不读取任何签名材料。Draft 先用精确 commit SHA 作为 `target_commitish`，正式 Publish 时 GitHub 才创建对应 tag。

## 2. Updater 信任材料

在隔离环境运行 Tauri signer 生成 updater keypair。私钥不得进入仓库、日志、普通 workflow artifact 或构建缓存，并应建立离线加密备份。

在 `release` Environment 配置：

| 类型 | 名称 | 内容 |
| --- | --- | --- |
| Variable | `VOICEPEN_UPDATER_PUBKEY` | Tauri updater 公钥全文；它会通过 Rust `option_env!` 编译进三个目标 |
| Secret | `TAURI_SIGNING_PRIVATE_KEY` | 与公钥配对的 updater 私钥全文 |
| Secret | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码，必须为非空值 |

公钥不是秘密，但属于应用信任根。任何轮换都必须先设计让旧客户端信任新根的迁移版本，不能直接替换。

## 3. macOS ad-hoc 分发

项目明确不使用 Apple Developer ID 和 notarization。`tauri.conf.json` 将 macOS `signingIdentity` 固定为 `-`，Tauri 对 `.app` 和 Updater app 进行 ad-hoc code seal；DMG 是未签名的分发容器。ad-hoc seal 能让系统验证应用代码结构未在打包后被随意改变，但不提供 Apple 验证的开发者身份或恶意软件公证结论。

macOS job 会对源 `.app`、实际 OTA `.app.tar.gz` 解包内容和 DMG 挂载后的 `.app` 执行 `codesign --verify`，断言主程序名为 `voicepen`，并拒绝出现 Authority 或有效 TeamIdentifier；DMG 容器本身必须保持未签名。同时用 `lipo` 要求 Apple Silicon 产物仅为 `arm64`、Intel 产物仅为 `x86_64`。Updater 资产仍必须通过项目 minisign 信任链验证。

首次打开预期会被 Gatekeeper 拦截。测试者只能使用 Finder 和“系统设置 → 隐私与安全性 → 仍要打开”的系统界面明确放行；不得关闭 Gatekeeper、执行 `xattr`、使用 `sudo` 或自动清除 quarantine。Apple 未公证这些产物，只应从项目官方 GitHub Release 下载。

CI 全绿后，可以用仓库中的发布前检查脚本读取对应版本的 `docs/releases/<version>.md` 并触发 Draft：

```bash
scripts/start-macos-release.sh --confirm
```

脚本会拒绝脏工作区、非 `main`、未同步远端、版本不一致、已存在 tag、缺少 updater Secret/Variable 或 HEAD CI 未通过，并把核准的完整 commit SHA 传给 workflow；workflow 在任何发布写入前再次要求 `GITHUB_SHA` 完全一致。`--confirm` 仅授权创建 Draft，workflow 仍需 `release` Environment 审批且不会 Publish。

## 4. Windows 签名材料

当前流程采用 Tauri 官方说明的可信 PFX/证书存储签名路径，不使用未确定的 `TAURI_CONFIG` 环境变量。在 Windows runner 中，流水线导入 PFX，并生成仅存在于 runner 的 `src-tauri/windows-signing.json`，随后通过 Tauri CLI 的 `--config` 参数合并签名配置。

在 `release` Environment 配置：

| 类型 | 名称 | 内容 |
| --- | --- | --- |
| Secret | `WINDOWS_CERTIFICATE` | 可信代码签名 `.pfx` 文件的 base64 全文 |
| Secret | `WINDOWS_CERTIFICATE_PASSWORD` | PFX 导出密码 |
| Variable | `WINDOWS_CERTIFICATE_THUMBPRINT` | 证书 thumbprint；流水线会核对实际导入证书 |
| Variable | `WINDOWS_TIMESTAMP_URL` | 证书签发方提供的 HTTPS RFC 3161 时间戳 URL |

在获得可信 Windows 代码签名证书前，Windows Release job 必须失败，不能降级为未签名安装包。Updater 的 minisign 签名不能替代 Authenticode。

构建后 workflow 会对 NSIS 与 MSI 安装器执行 `Get-AuthenticodeSignature`，要求状态为 `Valid`、签名证书 thumbprint 等于导入证书且存在时间戳；任一条件不满足都会阻止 Draft 完成。

## 5. 创建 Draft Release

发布前先完成以下准备：

1. 在 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 设置完全相同的稳定语义化版本，例如 `0.2.0`。
2. 确认版本提交已经位于 `main`，工作区没有密钥、证书或意外构建产物。
3. 准备面向用户的更新说明，包含实际存在的新增、改进、修复和注意事项；这段文字会直接进入 `latest.json.notes` 并显示在应用内。
4. 在 Actions → Build release draft → Run workflow，从 `main` 运行，输入不带 `v` 的版本、非空更新说明、范围和已核准的 40 位 `main` commit SHA；workflow 会拒绝 SHA 与实际运行提交不一致的请求。优先使用下述检查脚本避免手工参数漂移。
5. 明确选择 `release_scope`：默认 `all` 会串行构建 macOS Apple Silicon、macOS Intel、Windows x64；`macos` 只构建并验证两个 macOS 架构，供先行完成 Mac 干净机验收。审批 `release` Environment 后等待所选目标完成。

流水线使用固定的 `tauri-action v1.0.0` commit 创建以 `v<version>` 为标签名、精确 commit SHA 为目标的 Draft Release，上传安装包、updater 包、签名及 `latest.json`；正式 Publish 时 GitHub 创建 tag。最后一步会把 action 生成的清单规范化为发布后不可变的 tag URL，执行带相同 `--scope` 的 `npm run verify:release`，逐一核验实际资产、`.sig` 与内联签名，覆盖 Draft 中的清单并再次验证。`all` 仍对三个目标执行硬门禁；`macos` 必须完整包含两个 Mac 架构，且不会把 Windows 未构建状态伪装为通过。任何失败都不得手工拼凑未签名资产继续发布。

`release_scope` 只控制该版本包含的平台，不会降低 updater 安全要求。两个范围都要求 updater 私钥/公钥匹配；Mac 产物必须符合固定 ad-hoc seal 与架构约束；选择 `all` 时 Windows Authenticode 材料仍是不可绕过的硬门禁。工作流始终只生成 Draft，范围选择不授予 Publish 权限。

如果同名 tag 或 Release 已存在，流水线只允许复用与本次请求完全绑定的对象：已存在的 tag 解引用后必须等于当前 `GITHUB_SHA`；Release 必须仍是 Draft，`target_commitish` 必须是该 SHA，且名称、版本和更新说明必须逐字一致。已发布 Release、指向其他提交的 tag 或不同说明都会硬失败。最终清单阶段会再次检查这些绑定。

## 6. Draft 人工复核门禁

在 GitHub Draft Release 中逐项确认：

- Draft 的 `target_commitish` 是预期的 `main` commit；Publish 后 tag 必须解析到同一 commit，且三处源版本、tag、Release 标题和 `latest.json.version` 一致。
- `latest.json.notes` 是批准的更新说明。`macos` 范围必须且只能包含两个 Darwin 主键；`all` 范围必须且只能包含两个 Darwin 与 `windows-x86_64` 三个主键。
- 所选范围的 URL 均指向同一不可变 `v<version>` Release，签名是内联内容而不是 URL。
- 两个 macOS 安装资产和 `.app.tar.gz` updater 资产完整；workflow 中源 app、解包后的 updater app 与 DMG 内 app 的 ad-hoc seal、主程序名和目标架构检查全部成功；没有 Developer ID Authority/TeamIdentifier，并在真实机器记录 Gatekeeper 放行过程。
- 选择 `all` 时，Windows NSIS/MSI 安装器的 Authenticode 状态为 Valid，时间戳存在且 signer 与配置证书一致；`windows-x86_64` 清单项按约定指向 NSIS updater。选择 `macos` 时 Draft 不得包含遗留 Windows 安装资产。
- Updater `.sig` 与清单内联签名一致，没有私钥、PFX、密码或临时配置成为 Release 资产。
- 所选范围的 updater 资产会由仓库中的 Rust verifier 使用内置公钥同款 minisign 算法做密码学验证；只比较文件名或 `.sig` 文本不算通过。
- 所有 workflow jobs 成功；如启用 GitHub artifact attestations，证明对象必须是最终签名后的字节。

Artifact attestations 已登记为后续供应链增强，本阶段明确延期。它不能替代 updater minisign 或 Windows Authenticode，也不能把 macOS ad-hoc 产物表述为 Apple 已验证。

Draft 状态不会进入应用使用的 `/releases/latest/download/latest.json` 正式端点。只有在上述证据集中复核完毕并获得项目负责人明确的“发布此版本”授权后，才可以在 GitHub Release 页面点击 Publish release。不要把 Draft 改成 prerelease；项目只有一个正式通道。

## 7. 发布后与 Stage 5 干净机验收

正式发布后立即检查生产 `latest.json` 可访问且仍通过 `npm run verify:release`。然后在隔离的干净环境执行：

- macOS Apple Silicon：下载、Gatekeeper 首次启动、麦克风/辅助功能权限、完整语音链路、从上一版本 OTA、配置保留。
- macOS Intel：执行相同安装与升级路径，不能用交叉编译成功代替真实 Intel 验收。
- Windows x64：下载、SmartScreen 表现、安装、麦克风/快捷键/自动粘贴、从上一版本 OTA、配置保留和卸载。
- 三端验证“稍后更新”、网络中断、签名错误、清单平台缺失和安装失败的恢复行为。

如果发布后发现严重问题，不得原地替换同名资产；记录已知问题并发布更高语义化版本的修复 Release。Stage 5 的真实设备证据完成前，不得宣称生产级双端 OTA 已验收。
