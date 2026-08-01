# 多屏共享实现计划

> **给 Agent 的说明：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，按任务逐项执行。每一步使用复选框跟踪。

**目标：** 支持枚举多个显示器、以 AirPlay 风格选择一个显示器，并在共享中即时切换且保持 Viewer 连接。

**架构：** 保留现有单路 WebRTC/RTP 连接，将显示器元数据扩展为稳定 ID、主屏标记和缩略图；通过共享的活动目标和可替换采集句柄重启 HostPeer 的采集循环，不重新协商 WebRTC。UI 使用“整个屏幕 / 窗口或 App / 扩展显示器”三组来源，扩展显示器组显示列表与选中缩略图。

**技术栈：** Rust、Tauri 2、xcap、ScreenCaptureKit、str0m、Vue 3、TypeScript、Vitest、Vite。

---

### 任务 1：稳定显示器来源模型与枚举

**文件：**
- 修改：`src-tauri/src/webrtc/mod.rs` 的 `CaptureSourceInfo`、`enumerate_sources`
- 修改：`src-tauri/src/commands.rs` 的 `CaptureTargetArgs` 和枚举命令
- 修改：`src/utils/api.ts` 的 `CaptureSourceInfo` 类型
- 测试：`src-tauri/src/webrtc/mod.rs` 单元测试

- [ ] 增加 `source_id: String`、`is_primary: bool`、`preview: Option<String>` 字段；`id` 保留兼容现有窗口来源。
- [ ] macOS 显示器用 xcap 可获得的稳定标识（无法获得时使用 `screen:{index}`）填充 `source_id`，并把主屏信息写入 `is_primary`；窗口来源保持现有行为。
- [ ] 为 `CaptureTarget` 增加 `source_id: Option<String>`，将其从 `Copy` 改为 `Clone`，并更新所有目标读取处使用 `.clone()`；保留 `id` 作为当前 xcap/ScreenCaptureKit 索引回退。
- [ ] 添加测试：单屏/多屏结果字段完整、非 macOS 返回空列表、索引回退格式稳定。
- [ ] 运行 `cargo test --lib webrtc::tests` 和 `npx vue-tsc --noEmit`，确认类型同步。
- [ ] 提交：`feat: add stable multi-display source metadata`。

### 任务 2：ScreenCaptureKit 与 xcap 的显示器映射

**文件：**
- 修改：`src-tauri/src/webrtc/screencapturekit_capture.rs`
- 修改：`src-tauri/src/webrtc/mod.rs` 的 `capture_one_at_with_monitor` 与 ScreenKit 启动逻辑
- 测试：`src-tauri/src/webrtc/screencapturekit_capture.rs`（非 macOS 条件测试）

- [ ] 新增按稳定 `source_id` 查找显示器的函数，找不到时明确返回 `ScreenKitError::Unavailable`。
- [ ] ScreenCaptureKit 重新查询 `SCShareableContent` 后按 display ID 选择，不再假设 xcap 与 ScreenCaptureKit 的枚举顺序永远相同。
- [ ] xcap 路径优先按稳定 ID 找到显示器，旧环境回退到 `target.id`；窗口和测试图案路径不改变。
- [ ] 保留 ScreenCaptureKit 队列深度 `3`、最新帧策略和现有 IOSurface 编码路径。
- [ ] 运行 `cargo check --release`；macOS 上额外运行 `MACOSX_DEPLOYMENT_TARGET=13.0 RUSTFLAGS='-L /Library/Developer/CommandLineTools/usr/lib/swift/macosx' cargo check --features screenkit`。
- [ ] 提交：`feat: map selected displays across capture backends`。

### 任务 3：HostPeer 活动目标与即时切换

**文件：**
- 修改：`src-tauri/src/webrtc/host.rs`
- 修改：`src-tauri/src/signaling/handlers.rs`
- 修改：`src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`
- 测试：`src-tauri/src/webrtc/host.rs` 单元测试

- [ ] 在 `HostPeer` 增加 `active_target: Arc<Mutex<CaptureTarget>>` 和 `switch_target(new_target) -> Result<(), String>`；旧句柄停止后才替换，新目标启动失败时恢复旧目标。
- [ ] 把 `start_sharing` 的固定 `target` 改为从活动目标读取；采集线程通过 generation/通知机制退出旧循环并启动新循环，清空 `frame_rx` 中的旧编码帧。
- [ ] 连接、UDP socket、RTP writer、MID 和时间戳状态保持不变；新目标首个关键帧正常发送。
- [ ] `signaling::handlers::AppState` 保存当前 HostPeer 集合；`set_capture_target` 更新待启动目标并向所有活动 Peer 调用 `switch_target`。
- [ ] 新增测试覆盖：切换成功替换句柄、切换失败保留旧目标、旧帧被丢弃。
- [ ] 运行 `cargo test --lib webrtc::host::tests`。
- [ ] 提交：`feat: switch active display without renegotiation`。

### 任务 4：AirPlay 风格 Host 来源选择器

**文件：**
- 修改：`src/components/SourcePicker.vue`
- 修改：`src/components/HostShell.vue`
- 修改：`src/i18n/en.ts`、`src/i18n/zh-CN.ts`
- 修改：`src/utils/api.ts`
- 测试：新增 `tests/SourcePicker.spec.ts`

- [ ] 将来源 UI 改为三组：`整个屏幕`、`窗口或 App`、`扩展显示器`；屏幕组列出枚举到的显示器。
- [ ] 每个显示器条目显示缩略图、名称、分辨率和“主屏”标记；没有缩略图时显示稳定的占位图，不阻塞选择。
- [ ] 选中显示器时调用 `setCaptureTarget({ kind: 'screen', id, sourceId, quality })`；共享中直接触发即时切换。
- [ ] 枚举失败、权限不足、目标切换失败时显示中文和英文错误状态，并保持上一次有效选择。
- [ ] 添加响应式布局：窄窗口下显示器列表单列，选中预览仍不溢出；不改变现有质量控件语义。
- [ ] 运行 `npx vitest run`、`npx vue-tsc --noEmit` 和 `npm run build`。
- [ ] 提交：`feat: add AirPlay-style display picker`。

### 任务 5：缩略图生成与性能边界

**文件：**
- 修改：`src-tauri/src/webrtc/mod.rs` 的来源枚举辅助函数
- 修改：`src-tauri/src/commands.rs`，在 `enumerate_capture_sources` 返回缩略图数据
- 修改：`src/components/SourcePicker.vue`

- [ ] 缩略图只在枚举或用户选中时生成，限制最长边为 320px、JPEG/WebP 质量约 60，不进入视频采集循环。
- [ ] 生成失败不影响显示器条目和共享；UI 使用占位缩略图。
- [ ] 加入缓存键（稳定 `source_id` + 尺寸），避免每次轮询重新编码缩略图。
- [ ] 测试缩略图尺寸上限和空预览回退；确认来源枚举不会阻塞 Host 启动超过 500ms。
- [ ] 提交：`perf: bound display picker preview cost`。

### 任务 6：端到端验证与回归

**文件：**
- 修改：`tools/diag-frames-direct.js`（增加可选目标切换和截图标记）
- 修改：`DOCS.md`（更新多屏架构说明）

- [ ] 单屏环境运行 `node tools/diag-frames-direct.js`，确认真实屏幕持续解码、无 `no frames`/`AbortError`。
- [ ] 双屏环境启动 Host，选择第二屏，保存 Host 与 Viewer 截图，确认 Viewer 内容对应第二屏。
- [ ] 共享中切回第一屏，记录首个新关键帧到达时间，目标为小于 500ms；确认 WebRTC ICE/DTLS 未重连。
- [ ] 模拟拔出当前目标屏幕，确认旧流保持、UI 显示错误且不会黑屏。
- [ ] 运行完整门禁：`cargo check --release`、相关 Rust 测试、`cd viewer && npx vue-tsc --noEmit -p tsconfig.json`、`cd viewer && npm run build`、`npx vue-tsc --noEmit`。
- [ ] 提交：`test: verify multi-display switching`。

### 任务 7：文档与最终检查

**文件：**
- 修改：`DOCS.md`
- 修改：`AGENTS.md`（仅在用户明确要求时；默认不修改用户现有内容）

- [ ] 记录来源分组、稳定显示器 ID、共享中切换语义、失败回滚和真实截图验证方法。
- [ ] 检查 `git diff`，确认 `tools/output/` 未被提交、用户已有 `AGENTS.md` 修改未被覆盖。
- [ ] 使用 `git status --short` 和最终测试输出完成交付说明。
