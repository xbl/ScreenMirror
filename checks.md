# ScreenMirror 提交流程与质量检测清单 (Commit Checklist)

> **核心原则**：本项目作为低延迟局域网投屏工具，对稳定性、首帧延迟与视频流完整性有极高要求。在每一次 `git commit` 之前，**必须**严格按顺序执行并通过以下全部检测项。任何一项失败均**严禁提交**。

---

## 一、 一键快速执行脚本 (All-in-One Command)

在项目根目录下可依次执行核心门禁校验：

```bash
# 1. 后端与单元测试
(cd src-tauri && cargo check --release && cargo test)

# 2. 前端类型检查与 Viewer 构建打包
(cd viewer && npx vue-tsc --noEmit -p tsconfig.json && npm run build)
npx vue-tsc --noEmit
npm test

# 3. 端到端 (E2E) 投屏全链路验证
node tools/verify-fix.js
```

---

## 二、 分层检测门禁细则

### 1. 后端代码检查与测试 (Rust)
- [ ] **编译与静态检查**：
  ```bash
  cd src-tauri && cargo check --release
  ```
  - **判定标准**：退出码为 `0`，无语法错误，无新增编译器警告。
- [ ] **全量 Rust 单元与集成测试**：
  ```bash
  cd src-tauri && cargo test
  ```
  - **判定标准**：所有测试全部通过（通过数应 $\ge 50$）。
- [ ] **关键子系统专项回归**：
  ```bash
  cd src-tauri && cargo test --test signaling
  cd src-tauri && cargo test --test permissions
  cd src-tauri && cargo test --test capture_sources
  ```
  - **判定标准**：全部测试 PASS，无超时、无挂起。

### 2. Viewer 浏览器端检查与构建 (Viewer SPA)
> `viewer/` 是独立的 npm 工程，其打包产物 `viewer/dist/` 直接被 Tauri 宿主服务挂载。修改前端后**必须重新构建**，否则宿主将运行陈旧代码。

- [ ] **Viewer 类型检查**：
  ```bash
  cd viewer && npx vue-tsc --noEmit -p tsconfig.json
  ```
  - **判定标准**：退出码为 `0`，无 TypeScript 类型报错。
- [ ] **Viewer 生产构建**：
  ```bash
  cd viewer && npm run build
  ```
  - **判定标准**：成功生成 `viewer/dist/`，无 Rollup / Vite 打包失败或原生依赖缺失。

### 3. Host 宿主前端检查与测试 (Host SPA)
- [ ] **Host 类型检查**：
  ```bash
  npx vue-tsc --noEmit
  ```
  - **判定标准**：退出码为 `0`。
- [ ] **Vue 单元测试与组件交互测试**：
  ```bash
  npm test
  ```
  - **判定标准**：16 项测试全部通过（包含 `SourcePicker.spec.ts` 的源切换与品质控制用例）。

### 4. 端到端与媒体链路验证 (Headless E2E & Media Diagnostic)
> 涉及信令、WebRTC、编码（VideoToolbox/FFmpeg）、采集源切换或前端播放器状态机（PlayerView）的修改，**必须**运行真实无头端到端测试。

- [ ] **E2E 帧渲染基线门禁**：
  ```bash
  # 运行前请确保无历史残留僵尸进程占住 3131 端口：
  # lsof -nP -iTCP:3131 -sTCP:LISTEN
  node tools/verify-fix.js
  ```
  - **判定标准**：输出必须包含：
    `VERDICT: ✅ Frames rendered AND canvas has visible non-black pixels — viewer bug fully fixed`
    且退出码为 `0`。
- [ ] **深度媒体流诊断（当涉及视频编解码、帧率控制或底层 I/O 时必跑）**：
  ```bash
  node tools/diag-frames-direct.js
  ```
  - **判定标准**：
    - `packetsLost` 必须为 `0`；
    - `framesReceived` 与 `framesDecoded` 必须持续增长（$\ge 300$）；
    - 首帧变化时间（first-change latency）必须低于 500ms。

---

## 三、 架构与代码规范红线 (Red Lines)

1. **严禁硬编码 Sleep 规避时序**：
   - 严禁在信令或状态机中使用 `tokio::time::sleep` 或前端 `setTimeout` 规避异步竞态，必须使用确定性的消息确认或通道事件。
2. **遵守 PlayerView 脆弱状态机约定**：
   - `<video>` 挂载与 `MediaStream` 挂载依赖特定的 DOM patch 时序，严禁在未经过完整 E2E 验证前擅自修改流绑定逻辑或随意重命名组件。
3. **保证资源回收与连接清理**：
   - WebSocket 断开、浏览器退出或投屏停止时，必须通过 RAII（如 `ConnectionGuard`）自动注销 sink、释放设备并关闭 Peer，禁止产生孤立进程或泄露信令连接。
4. **清理僵尸进程与端口占用**：
   - 本地测试完成后，若测试异常中断，必须清理残留的 `screenmirror` 或无头 `Chrome` 进程，防止占用 3131 端口影响后续测试。
5. **Git 提交干净度**：
   - 严禁提交 `tools/output/` 目录下的调试日志与截图文件（已配置在 `.gitignore`）。
   - Commit 标题与描述严格遵循规范：`fix(模块): 简短说明` 或 `feat(模块): 简短说明`。
6. **禁止将纯内存 Synthetic / Mock 模式作为真实投屏故障修复的唯一验收依据**：
   - `SCREENMIRROR_CAPTURE=test` 与 `127.0.0.1` 仅能用于信令通道冒烟测试。涉及无画面、黑屏、卡顿或连接超时的故障，必须在真实屏幕抓取（ScreenCaptureKit）和真实局域网 IP 上验证。
7. **WebRTC 事件循环中严禁在单次 UDP 错误时退出**：
   - `socket.recv_from` 遇到 `os error 49 (Can't assign requested address)` 等由虚拟网卡/ICMP 产生的不可达暂态错误时，必须只记录警告并继续循环，严禁 break 终止连接。
8. **当用户反馈真实环境失败时，坚持现场复现原则**：
   - 严禁仅跑自动化脚本自证清白，必须优先提取真实客户端的 WebRTC 状态（`pc.connectionState`）与 Candidate 协议类型，排查是否受浏览器隐私扩展策略（如 `WebRTC Leak Prevent` 禁用非代理 UDP）限制。

---

## 四、 提交流程示范

```bash
# 1. 确认工作区变更
git status
git diff

# 2. 执行本清单中所有门禁测试并通过
(cd src-tauri && cargo check --release && cargo test)
(cd viewer && npx vue-tsc --noEmit -p tsconfig.json && npm run build)
npx vue-tsc --noEmit && npm test
node tools/verify-fix.js

# 3. 暂存指定修改文件
git add <修改文件> checks.md

# 4. 提交
git commit -m "fix(signaling): decouple websocket read/write channels and eliminate artificial delays"
```
