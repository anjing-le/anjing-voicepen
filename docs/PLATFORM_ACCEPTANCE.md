# VoicePen 双端验收矩阵

本文件记录 macOS 与 Windows 的真实运行证据。条件编译、CI 构建和安装包生成均不能代替真机验收。

## 1. 记录规则

每个验收项只使用：`未测`、`通过`、`失败`、`阻塞`。证据至少记录：

- VoicePen 版本、commit 与安装方式（dev / unsigned / signed + notarized）
- OS 名称与完整版本、CPU 架构、设备类型
- 测试日期、执行人、操作步骤和证据链接
- 失败时的实际表现、恢复方式和关联 Issue

当前仓库尚无经过签名安装包的双端验收证据，因此以下目标平台均从 `未测` 开始。

实际执行时遵循 [`CLEAN_MACHINE_ACCEPTANCE.md`](./CLEAN_MACHINE_ACCEPTANCE.md)，也可在新 Codex 任务中使用 [`CLEAN_MACHINE_TEST_PROMPT.md`](./CLEAN_MACHINE_TEST_PROMPT.md) 逐步引导。完整 OTA 验收需要两个正式版本：先干净安装基线版本 A，再从 A 升级到更高版本 B。

## 2. 目标平台

| 平台 | 架构 | 当前状态 | 发布验收要求 |
| --- | --- | --- | --- |
| macOS | Apple Silicon | 未测 | 签名、公证安装；权限、核心闭环、升级与配置保留 |
| macOS | Intel | 未测 | 签名、公证安装；权限、核心闭环、升级与配置保留 |
| Windows 11 | x64 | 未测 | 代码签名安装；权限、核心闭环、升级与配置保留 |

Linux、Windows ARM、Mac App Store 与 Microsoft Store 不属于当前发布矩阵。

## 3. 通用生命周期

| 场景 | 预期 | macOS arm64 | macOS x64 | Windows x64 |
| --- | --- | --- | --- | --- |
| 首次启动 | 浮窗出现；未配置时可进入设置 | 未测 | 未测 | 未测 |
| UI 关闭按钮 | 设置窗口隐藏，托盘和浮窗继续工作 | 未测 | 未测 | 未测 |
| 原生标题栏关闭 | 与 UI 关闭一致，不退出后台进程 | 未测 | 未测 | 未测 |
| 托盘左键/菜单 | 能重新打开并聚焦设置 | 未测 | 未测 | 未测 |
| 快捷键被占用 | 应用不崩溃，展示错误并允许修改 | 未测 | 未测 | 未测 |
| 托盘退出 | 应用完整退出，不遗留录音/粘贴子进程 | 未测 | 未测 | 未测 |
| 录音或处理中退出 | 退出行为明确，不产生异常粘贴 | 未测 | 未测 | 未测 |
| 睡眠/锁屏恢复 | 快捷键、麦克风和托盘可恢复 | 未测 | 未测 | 未测 |

## 4. 麦克风与设备

每个平台验证：首次授权、允许、拒绝、系统中撤销后重试、无默认设备、设备忙、USB/蓝牙切换、运行中拔出、短录音和正常录音。失败提示必须区分“未找到设备”“没有音频数据”和服务请求失败，并给用户可执行的恢复路径。

- macOS：检查 Privacy & Security → Microphone 中的应用身份；开发版结果不能替代签名 `.app`。
- Windows：检查 Privacy & security → Microphone 中桌面应用访问开关；Win32 桌面应用不按 UWP capability 推断权限状态。

## 5. 快捷键与处理状态

验证默认组合、用户自定义组合、冲突/系统保留组合、快速连按、录音停止瞬间连按、转写/润色/completing 期间连按、多实例及睡眠恢复。任一时刻最多只能有一条语音处理管线，陈旧任务不得覆盖剪贴板或界面。

## 6. 剪贴板与自动粘贴

自动粘贴是可选能力；失败时润色结果必须仍在剪贴板。

- macOS：分别验证 Accessibility/Automation 未授权、拒绝、授权和撤销；在签名应用中确认系统显示的授权主体。
- Windows：验证 Notepad、浏览器、Office、管理员目标、不同完整性级别、PowerShell/COM 被策略阻止以及安全桌面边界。
- 两端：验证 10 秒子进程超时后应用仍能继续下一次录音。

设置页的“测试粘贴接口”只证明系统调用返回，不能证明能够粘贴回用户原先的目标窗口。

## 7. 窗口、托盘与显示器

验证浮窗不抢输入焦点；100%/125%/150% 缩放；外接屏、主屏切换、Dock/任务栏位置变化；深浅主题托盘图标；设置窗口隐藏/恢复；应用退出。浮窗当前只在启动时按主屏定位，多屏动态调整仍是待改进能力。

## 8. 安装、系统信任与升级

正式发布前分别完成：干净账户安装、Gatekeeper/SmartScreen 表现、首次权限、卸载、旧版到新版 OTA、取消更新、网络失败、签名失败、配置保留和故障恢复。只有三个目标平台的签名安装与核心闭环证据齐全，才能称为双端正式支持。

## 9. 官方参考

- [Apple NSMicrophoneUsageDescription](https://developer.apple.com/documentation/BundleResources/Information-Property-List/NSMicrophoneUsageDescription)
- [Apple 辅助功能授权](https://support.apple.com/guide/mac-help/allow-accessibility-apps-to-access-your-mac-mh43185/mac)
- [Apple 平台安全：Accessibility 与 Automation](https://support.apple.com/guide/security/controlling-app-access-to-files-secddd1d86a6/web)
- [Tauri macOS bundle](https://v2.tauri.app/distribute/macos-application-bundle/)
- [Tauri global shortcut](https://v2.tauri.app/plugin/global-shortcut/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Microsoft Windows microphone privacy](https://support.microsoft.com/en-us/windows/privacy/windows-camera-microphone-and-privacy)
- [Microsoft Settings URI](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings)
