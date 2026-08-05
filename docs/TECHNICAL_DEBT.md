# 技术债与历史背景登记

本登记基于 `0.1.0`、两次初始提交的 MVP。当前没有旧 Release、迁移记录或长期用户兼容证据；对设计动机无法由仓库证明的内容标记为未知，不以推测代替事实。

状态只使用：`Needs Investigation`、`Awaiting Historical Context`、`Confirmed to Preserve`、`Compatibility Isolated`、`Repayment Planned`、`Repayment in Progress`、`Repaid`、`No Longer Applicable`。

| ID | Observation | Classification | Evidence | Unknown History | Current Risk | Compatibility Target | Temporary Decision | Exit Criteria | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TD-001 | Rust 核心能力集中在单个约千行 `lib.rs` | Technical Debt | 配置、Provider、Runtime 已拆分；Audio、Platform 和窗口仍在 `lib.rs` | 无证据表明单文件是长期设计 | 剩余平台修改仍耦合、测试困难 | 保留现有 Tauri command/event 和用户流程 | 按领域渐进拆分，不整体重写 | Audio、Provider、Config、Platform、Runtime、Update 边界清楚且回归通过 | Repayment in Progress |
| TD-002 | 录音与异步处理原先缺少统一执行权和严格状态转换 | Technical Debt | 已加入强类型 Runtime、operation token、快捷键串行锁和 completion lease；纯状态测试已覆盖重复输入与陈旧 token | 尚缺 Windows 真机压力验证 | 平台事件调度差异仍可能暴露未覆盖路径 | 保留按键开始/停止体验 | 在双端验收阶段补真实快捷键压力测试 | 重复输入、请求失败和恢复路径均有测试，任一时刻最多一条管线 | Repayment in Progress |
| TD-003 | API Key 仍以明文保存在本地 JSON | Technical Debt | 配置序列化包含 STT/LLM Key；Unix 临时文件在提交前设为 `0600`，权限失败会返回可见错误 | 现有用户是否依赖直接编辑配置未知 | 本机账户或备份仍可能泄露；Windows 文件 ACL 尚未验证 | 首期保持旧 JSON 可继续读取，BYOK 不变 | Unix 权限失败已可见；双端阶段验证 Windows ACL，不在 OTA 前强制迁移钥匙串 | 完成跨平台安全存储方案、版本化迁移、失败回退和真实设备验证 | Repayment in Progress |
| TD-004 | 双端能力尚无真实安装/权限证据 | Unknown Cause / Unverified Capability | 已建立 `PLATFORM_ACCEPTANCE.md`，但所有目标仍为未测；仓库无签名 Release | Windows 与 Intel Mac 是否运行过未知 | 编译成功被误报为产品支持；权限和粘贴可能失败 | 目标矩阵为 macOS arm64/x64、Windows x64 | 按矩阵积累自动构建与人工证据，报告中区分完成度 | 三个目标均有签名安装、录音、快捷键、粘贴、OTA 的可追溯证据 | Repayment in Progress |
| TD-005 | 已有核心单元测试，但尚无 CI 与完整平台/集成测试 | Technical Debt | 配置、Provider、Runtime、Diagnostics 当前共有 26 个 Rust 单元测试；仓库仍无 GitHub Actions workflow | 无 | 直接 push `main` 时测试不会自动拦截；真实平台回归仍依赖人工 | 保留 `npm run build`、Rust test/check/clippy 本地门禁 | 扩充协调与平台测试，并让 main push 自动执行 | CI 对每次 push 执行前端构建、Rust 检查/测试并稳定通过 | Repayment in Progress |
| TD-006 | 已有 OTA 应用内流程，但尚无真实签名 Release、公证、代码签名或回滚验证 | Technical Debt | Rust Updater 使用固定 GitHub endpoint、编译期公钥、用户确认和纯文本 notes；尚无 workflow/Release/真机升级证据 | 签名账号、证书和密钥保管条件未知 | 无法安全分发；仅凭本地 UI/测试不能证明真实升级安全 | GitHub Releases 单通道、用户确认更新 | 未注入公钥的构建在联网前停止；正式发布需单独批准 | 三平台产物按矩阵构建；签名/公证通过；旧版升级和失败保留验证完成 | Repayment in Progress |
| TD-007 | 配置与系统快捷键无法组成真正的跨系统原子事务 | Technical Debt | 已改为先验证并切换快捷键、临时文件提交配置，失败时显式补偿并报告补偿错误 | 各平台快捷键插件极端失败语义仍缺真机证据 | 补偿 API 本身失败时可能残留未跟踪快捷键 | 成功时磁盘、内存和快捷键一致；失败时优先恢复旧状态并明确报告 | 双端阶段增加真实冲突快捷键和失败恢复验证 | 新快捷键注册失败时旧快捷键仍可用；补偿失败有可恢复指引和平台证据 | Repayment in Progress |
| TD-008 | 录音样本持续存于内存且无时长上限 | Technical Debt | `Recorder` 累积样本直至用户停止 | 预期最长使用时长未知 | 误触后长时间录音造成内存与大请求风险 | 正常短语音输入不受影响 | 先测量并确定轻量工具合理上限与提示 | 有产品认可的限制、临界提示、停止行为与测试 | Needs Investigation |
| TD-009 | 第三方 HTTP 错误正文曾直接展示 | Technical Debt | Provider 现在只展示错误类别与 HTTP 状态，不再读取或反射非成功响应正文；解析失败也不回显原文 | 无 | 已关闭原始正文泄露路径 | 用户仍获得超时、连接、HTTP、响应格式等错误类别 | 保持默认不反射第三方正文 | 安全错误边界与测试持续通过 | Repaid |
| TD-010 | Linux 分支存在但不属于正式产品矩阵 | Historical Compatibility (scope unknown) | `trigger_paste` 有 Linux 实现，章程只承诺 macOS/Windows | Linux 是否已有用户未知 | 无意承担未验证的平台承诺或重构时误删可用代码 | 保留现有代码但不承诺发行 | 不因双端工作主动删除；新架构将其隔离为非验收实现 | 出现明确支持决策后纳入矩阵，或取得兼容影响证据后有计划移除 | Compatibility Isolated |
| TD-011 | 配置模型未建立显式版本与迁移框架 | Technical Debt | serde 默认值提供部分兼容，但无 schema/version 或迁移记录 | 尚无 Release 用户，未来变化范围未知 | OTA 后字段变化可能导致启动或配置丢失 | `0.1.0` JSON 配置应继续可读 | 在首次破坏性模型变化前引入迁移边界 | 至少存在旧版夹具、迁移测试、失败保留/备份策略 | Repayment Planned |
| TD-012 | `macOSPrivateApi` 已启用但必要性和长期影响未记录 | Unknown Cause | Tauri 配置和依赖 feature 均启用该选项 | 哪个窗口行为必须依赖私有 API 未知 | 公证、兼容性或未来升级带来不明确风险 | 当前无边框透明浮窗行为 | 在改变前做只读追踪与平台验证 | 明确依赖点和理由后登记保留，或验证移除不影响体验 | Needs Investigation |
| TD-013 | WebView 已启用最小 CSP，但尚缺 macOS/Windows 打包 WebView 的真实回归 | Technical Debt | CSP 仅允许本地资源与 Tauri IPC；Provider 和 updater 网络均由 Rust 发起 | 两个平台 WebView 对生产 CSP 的细节差异尚未实测 | CSP 配置错误可能使设置或浮窗在特定系统失效 | 现有本地 UI、IPC 与 Rust 网络边界保持可用 | 自动构建验证配置，Stage 5 在签名安装包中回归全部 UI/IPC | 三目标生产安装包的设置、浮窗、Provider 与 OTA 均通过 CSP 下真机验证 | Repayment in Progress |
| TD-014 | Rust 与前端原先分别维护无约束状态字符串 | Technical Debt | Rust 已使用显式序列化枚举并测试全部前端状态；内部 `Completing` 映射为稳定的 `polishing` | 尚未引入自动生成 TS 类型 | 手工 TS 联合类型仍可能在未来新增状态时漂移 | 保留现有 `idle/recording/transcribing/polishing/done/error` 用户语义 | 当前用契约测试约束，未来评估类型生成 | Rust 枚举与前端契约持续通过测试，未知值有安全回退 | Compatibility Isolated |
| TD-015 | 三处应用版本由人工分别维护 | Technical Debt | `package.json`、`Cargo.toml`、`tauri.conf.json` 均为 `0.1.0`，无同步检查 | 无 | Release、安装包和更新清单版本漂移，导致 OTA 判断或产物命名错误 | 三处版本及 tag/Release 必须一致 | 在发布流水线前增加一致性检查 | CI 自动拒绝版本不一致，发布产物与 tag/清单也经过校验 | Repayment Planned |

## 维护规则

- 发现新债务时必须填写风险、兼容边界、临时决策与可验证的退出条件，不能只写“以后重构”。
- 还债提交应更新状态和证据；尚未获得真实设备或生产证据时不得标记 `Repaid`。
- 未知历史若影响外部行为，默认保留行为、增加特征测试并将兼容性隔离，直到证据或产品决策足够。
