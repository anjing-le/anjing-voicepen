# 技术债与历史背景登记

本登记基于 `0.1.0`、两次初始提交的 MVP。当前没有旧 Release、迁移记录或长期用户兼容证据；对设计动机无法由仓库证明的内容标记为未知，不以推测代替事实。

状态只使用：`Needs Investigation`、`Awaiting Historical Context`、`Confirmed to Preserve`、`Compatibility Isolated`、`Repayment Planned`、`Repayment in Progress`、`Repaid`、`No Longer Applicable`。

| ID | Observation | Classification | Evidence | Unknown History | Current Risk | Compatibility Target | Temporary Decision | Exit Criteria | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| TD-001 | Rust 核心能力集中在单个约千行 `lib.rs` | Technical Debt | 录音、AI、配置、平台和窗口逻辑均在同一文件 | 无证据表明这是长期设计 | 修改耦合、平台条件分支扩散、测试困难 | 保留现有 Tauri command/event 和用户流程 | 按领域渐进拆分，不整体重写 | Audio、Provider、Config、Platform、Runtime、Update 边界清楚且回归通过 | Repayment Planned |
| TD-002 | 录音与异步处理缺少统一执行权和严格状态转换 | Technical Debt | 录音锁与运行快照分离；处理任务启动后快捷键仍可进入处理逻辑 | 尚无重复触发的真实用户数据 | 并发请求、结果覆盖、状态错乱 | 保留按键开始/停止体验 | OTA 前建立显式状态机和并发门禁 | 重复输入、请求失败和恢复路径均有测试，任一时刻最多一条管线 | Repayment Planned |
| TD-003 | API Key 明文保存在本地 JSON；Unix 权限收紧失败被忽略 | Technical Debt | 配置序列化包含 STT/LLM Key；写入后尝试设为 `0600`，但 `set_permissions` 结果被丢弃 | 现有用户是否依赖直接编辑配置未知 | 本机账户或备份泄露；权限失败仍报告保存成功；Windows 文件 ACL 未证明 | 首期保持旧 JSON 可继续读取，BYOK 不变 | 文档明确风险；近期让权限失败可见，不在 OTA 之前强制迁移钥匙串 | 权限失败不再静默；完成跨平台安全存储方案、版本化迁移、失败回退和真实设备验证 | Repayment Planned |
| TD-004 | 双端能力尚无真实安装/权限证据 | Unknown Cause / Unverified Capability | 代码含 macOS、Windows 分支；无 CI、Release 或验收记录 | Windows 与 Intel Mac 是否运行过未知 | 编译成功被误报为产品支持；权限和粘贴可能失败 | 目标矩阵为 macOS arm64/x64、Windows x64 | 建立自动构建与人工验收清单，报告中区分完成度 | 三个目标均有签名安装、录音、快捷键、粘贴、OTA 的可追溯证据 | Needs Investigation |
| TD-005 | 没有自动测试与 CI | Technical Debt | 仓库无测试套件和 workflow | 无 | 直接 push `main` 时回归无自动拦截 | 保留当前 `npm run build`、`cargo check` 基线 | 先覆盖纯逻辑和核心状态；main push 自动检查 | CI 对每次 push 执行前端构建、Rust 检查/测试并稳定通过 | Repayment Planned |
| TD-006 | 没有安装包签名、公证、OTA 或回滚验证 | Technical Debt | Tauri bundling 已启用，但无 updater、workflow、Release | 签名账号、证书和密钥保管条件未知 | 无法安全分发；更新失败可能损坏体验 | GitHub Releases 单通道、用户确认更新 | 先设计信任链，正式发布需单独批准 | 三平台产物按矩阵构建；签名/公证通过；旧版升级和失败保留验证完成 | Repayment Planned |
| TD-007 | 保存新配置时先写磁盘和内存，再注销旧快捷键；新快捷键注册失败不恢复 | Temporary Patch / Technical Debt | `save_config` 在 `register_shortcut` 前持久化并替换运行配置，后者先注销旧值再注册新值 | 是否曾因平台限制刻意如此未知 | 失败后磁盘、内存和系统注册状态可能不一致，且本次进程失去可用快捷键 | 新配置成功返回时磁盘、内存和快捷键必须一致；失败时旧配置与快捷键继续可用 | 在可靠性阶段实现事务式切换或完整回滚 | 新快捷键注册失败时磁盘、内存和系统注册均恢复旧状态，且有单元测试与平台验证 | Repayment Planned |
| TD-008 | 录音样本持续存于内存且无时长上限 | Technical Debt | `Recorder` 累积样本直至用户停止 | 预期最长使用时长未知 | 误触后长时间录音造成内存与大请求风险 | 正常短语音输入不受影响 | 先测量并确定轻量工具合理上限与提示 | 有产品认可的限制、临界提示、停止行为与测试 | Needs Investigation |
| TD-009 | 第三方 HTTP 错误正文可能直接展示 | Technical Debt | `http_error` 读取服务响应正文并拼入错误 | 各 Provider 是否返回敏感数据未知 | 界面、日志或报告泄露服务细节/敏感数据 | 用户仍需获得可操作的错误类别 | 建立清洗、截断和结构化错误边界 | Key/授权头/非受控正文不会泄露，常见错误仍可定位且有测试 | Repayment Planned |
| TD-010 | Linux 分支存在但不属于正式产品矩阵 | Historical Compatibility (scope unknown) | `trigger_paste` 有 Linux 实现，章程只承诺 macOS/Windows | Linux 是否已有用户未知 | 无意承担未验证的平台承诺或重构时误删可用代码 | 保留现有代码但不承诺发行 | 不因双端工作主动删除；新架构将其隔离为非验收实现 | 出现明确支持决策后纳入矩阵，或取得兼容影响证据后有计划移除 | Compatibility Isolated |
| TD-011 | 配置模型未建立显式版本与迁移框架 | Technical Debt | serde 默认值提供部分兼容，但无 schema/version 或迁移记录 | 尚无 Release 用户，未来变化范围未知 | OTA 后字段变化可能导致启动或配置丢失 | `0.1.0` JSON 配置应继续可读 | 在首次破坏性模型变化前引入迁移边界 | 至少存在旧版夹具、迁移测试、失败保留/备份策略 | Repayment Planned |
| TD-012 | `macOSPrivateApi` 已启用但必要性和长期影响未记录 | Unknown Cause | Tauri 配置和依赖 feature 均启用该选项 | 哪个窗口行为必须依赖私有 API 未知 | 公证、兼容性或未来升级带来不明确风险 | 当前无边框透明浮窗行为 | 在改变前做只读追踪与平台验证 | 明确依赖点和理由后登记保留，或验证移除不影响体验 | Needs Investigation |
| TD-013 | WebView Content Security Policy 设置为 `null` | Technical Debt | `tauri.conf.json` 的 `app.security.csp` 明确为空 | 是否为早期开发便利而关闭未知 | 一旦界面出现注入，缺少 CSP 纵深防御；未来 updater UI 会扩大影响 | 现有本地 UI 与用户配置的 Provider 地址仍需工作 | OTA 接入前梳理连接目标并设置最小可行 CSP | 前端构建及 STT/LLM/OTA 网络边界在约束 CSP 下通过，且有配置说明 | Repayment Planned |
| TD-014 | Rust 发出的状态值缺少类型约束，前端使用独立字符串联合类型 | Technical Debt | Rust `stage` 接受任意 `String`；TypeScript 独立声明 `RuntimeStage` | 两端契约是否曾人工同步未知 | 新状态或拼写变化导致 UI 静默落入错误展示分支 | 保留现有 `idle/recording/transcribing/polishing/done/error` 用户语义 | 状态机阶段生成共享或可验证契约 | Rust 使用枚举/稳定序列化，前端契约有生成或契约测试，未知值有安全回退 | Repayment Planned |
| TD-015 | 三处应用版本由人工分别维护 | Technical Debt | `package.json`、`Cargo.toml`、`tauri.conf.json` 均为 `0.1.0`，无同步检查 | 无 | Release、安装包和更新清单版本漂移，导致 OTA 判断或产物命名错误 | 三处版本及 tag/Release 必须一致 | 在发布流水线前增加一致性检查 | CI 自动拒绝版本不一致，发布产物与 tag/清单也经过校验 | Repayment Planned |

## 维护规则

- 发现新债务时必须填写风险、兼容边界、临时决策与可验证的退出条件，不能只写“以后重构”。
- 还债提交应更新状态和证据；尚未获得真实设备或生产证据时不得标记 `Repaid`。
- 未知历史若影响外部行为，默认保留行为、增加特征测试并将兼容性隔离，直到证据或产品决策足够。
