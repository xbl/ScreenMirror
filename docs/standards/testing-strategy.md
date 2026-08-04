# ScreenMirror 测试策略

## 目标与范围

ScreenMirror 是由 Tauri 2 Host、Vue 3 Host SPA、独立 Viewer SPA，以及 Rust WebRTC/信令服务组成的局域网扩展屏应用。测试策略覆盖功能正确性、播放链路、交互回归、性能边界和提交门禁。

测试应优先验证稳定的公开契约：用户可见状态、组件输入输出、命令/服务协议、媒体是否真实解码。不要把 CSS class、DOM 排列、像素坐标或内部实现细节当作主要契约。

## 分层模型

按“最窄有效层级”选择测试。技术行为测试和业务意图测试分开：单元/集成测试定位实现问题，E2E 验证用户实际完成了连接和观看流程。

| 改动类型 | 首选覆盖 | 需要扩大范围的情况 |
| --- | --- | --- |
| 纯函数、解析器、归一化、队列、状态转换 | TS/Vitest 或 Rust unit | 行为跨越模块边界 |
| Vue props、事件、可见状态、交互 | Vue component test | 影响完整用户流程 |
| Tauri command、服务、信令、采集源、权限、设备 | Rust integration test | 依赖真实浏览器或媒体链路 |
| WebRTC 采集、编码、信令、播放挂载 | 定向测试 + 真实 E2E | 用户看到的直播流程有变化 |
| 纯样式整理 | lint/typecheck/build | 可能改变布局或播放器行为 |
| Bug 修复 | 先补最窄层级的回归测试 | 根因在真实媒体或浏览器时序 |

不要用 E2E 替代可以快速、稳定表达的单元、组件或集成测试。外部依赖只在边界 mock，不能 mock 被测逻辑，也不能把 fake WebRTC 握手当成真实媒体集成。

## 当前测试落点

### Host Vue/TypeScript

- `tests/vue/api.test.ts`：API wrapper 契约。
- `tests/vue/components.test.ts`：Host 组件行为。
- `tests/SourcePicker.spec.ts`：来源选择交互。
- `src/` 与 `tests/` 的 lint、typecheck、Vitest 是常规门禁。

### Viewer Vue/TypeScript

Viewer 是独立 npm 项目，必须在 `viewer/` 内运行自己的命令。当前主要由 typecheck、build 和真实 WebRTC E2E 覆盖；涉及 `PlayerView` 状态机的改动应补 mounted SFC 或专门回归测试，不能只依赖成功握手。

`PlayerView` 的以下规则是负载时序契约：`<video>` 只在 `streaming` 状态渲染；`viewer-stream` 先缓存 `pendingStream` 再切换状态；post-flush watcher 在 DOM 创建后挂载；五秒 no-frame watchdog 保留；`MainView` 和 `PlayerView` 的两级 `v-if` 不应随意合并。

### Rust

Rust unit tests 位于模块内部，覆盖 WebRTC profile/normalization/queue、信令解析、采集源映射、preview cache、VideoToolbox 转换等纯行为。Rust integration tests 位于 `src-tauri/tests/`：

- `permissions.rs`
- `capture_sources.rs`
- `devices.rs`
- `network.rs`
- `room_id.rs`
- `signaling.rs`

它们用于验证 command、state、service、平台行为、设备状态、房间状态和 HTTP/WS 协议之间的协作。

## 命令与门禁

日常快速检查：

```bash
npm run lint
npm run typecheck
npm test
cd viewer && npm run lint && npm run typecheck
```

完整静态、构建和 Rust 检查：

```bash
npm run format:check
npm run build
cd viewer && npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --release
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --no-deps
cargo test --manifest-path src-tauri/Cargo.toml
```

`npm run check` 聚合 Host lint、typecheck 和 Vitest；`npm run check:all` 进一步执行 Host build 与 Rust 检查。Husky pre-commit 会运行配置的 Host/Viewer lint，但不代替完整测试。发布或合并前应记录所有未运行的门禁及原因。

Host 与 Viewer 的 `build` 是两个独立项目；不要把 `viewer/dist` 当作根项目构建输出。若 Vite/Rollup 因缺少可选 native dependency 失败，已有 `viewer/dist` 视为 stale，不能据此声称新 Viewer 代码已验证。

## WebRTC 与播放验收

媒体改动默认使用真实 Tauri binary 和 headless Chrome：

```bash
cd viewer && npm run build
cd ..
node tools/verify-fix.js
node tools/diag-frames-direct.js
```

`verify-fix.js` 必须输出：

```text
VERDICT: ✅ Frames rendered AND canvas has visible non-black pixels
```

`diag-frames-direct.js` 用于诊断 `framesDecoded`、jitter-buffer delay、packet loss 和首次解码变化。成功建立 WebRTC 连接或收到 video track 都不足以证明画面可用；验收必须确认 `videoWidth`、解码帧和非黑色画面。局域网低延迟目标应将 `RTCRtpReceiver.jitterBufferTarget` 与支持时的 `playoutDelayHint` 设为零，并用 Host/Viewer 的移动时钟或计时器验证延迟，而不是只测连接成功。

出现五秒无帧时，先检查 `3131` 端口是否被旧进程占用、Chrome 是否真实启动、room 是否被正确接管、track 是否收到、Host 是否产生 `MediaAdded` 和 capture loop 日志，以及 `viewer/dist` 是否刚刚重建。不要用放宽 watchdog、伪造 no-frame 成功或删除播放状态检查来掩盖链路问题。

## 失败分类与隔离

每次失败先按以下类别归因：

- 实现缺陷：公开行为不符合预期，补回归测试后修复。
- selector/fixture 缺陷：测试数据或定位器不再代表公开契约。
- 时序缺陷：等待了固定时间而不是可观察状态，改为等待事件、状态或输出。
- 环境缺陷：端口、权限、浏览器、可选 native dependency 或旧进程导致失败。

普通测试流程不得使用无意义的 `waitForTimeout`；E2E 应等待可观察的 DOM 状态、媒体事件、日志或统计值。测试应隔离临时目录、端口和浏览器 profile，并清理由诊断脚本启动的进程。

## 已知缺口

- Viewer 目前缺少系统化的 Vue component test，`PlayerView` 需要优先补齐挂载/卸载、pending stream 和 no-frame 回归覆盖。
- 真实 WebRTC E2E 依赖 macOS 屏幕录制权限、可用的 VideoToolbox/H.264 解码环境和干净端口，CI 需要显式准备这些条件。
- 延迟目标需要移动时钟或计时器场景的持续测量，不能以静态测试图替代。

## 完成标准

完成一个改动前应：补充适当层级的回归测试；运行受影响子项目的 lint、typecheck、unit/integration test；媒体改动重建 Viewer 并运行真实 E2E；执行 `git diff --check`；在交付说明中列出通过、失败、跳过的检查及环境原因。
