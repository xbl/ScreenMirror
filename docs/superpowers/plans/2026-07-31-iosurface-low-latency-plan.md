# IOSurface 低延迟采集实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` 或 `superpowers:subagent-driven-development` 按任务逐项执行。每一步使用 checkbox 跟踪。

**目标：** 使用 ScreenCaptureKit + IOSurface + 原生 VideoToolbox，消除 xcap 采集缓冲，将真实 Host/Viewer 延迟压到 500ms 内。

**架构：** macOS 13+ 下新增独立 ScreenCaptureKit 捕获器，回调只保留最新 `CVPixelBuffer`。原生 VideoToolbox 编码器直接消费 IOSurface；初始化或运行失败自动回退现有 xcap/FFmpeg 路径。

**技术栈：** Rust、`screencapturekit` 8.x、CoreMedia/CoreVideo、VideoToolbox、现有 `str0m` WebRTC。

---

### Task 1: 添加 macOS 依赖与捕获模块接口

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 修改：`src-tauri/src/webrtc/mod.rs`
- 创建：`src-tauri/src/webrtc/screencapturekit_capture.rs`

- [ ] 添加 macOS-only `screencapturekit = "8.0.1"` 依赖，启用 macOS 13 API feature；运行 `cargo check` 确认依赖可链接。
- [ ] 定义 `ScreenKitCapture`、`ScreenKitFrame` 和 `ScreenKitError`。`ScreenKitFrame` 必须包含宽、高、bytes-per-row、BGRA/IOSurface 句柄和 `Instant` 时间戳。
- [ ] 在非 macOS 构建中提供返回“不支持”的 stub，确保 Linux CI 和现有测试继续编译。
- [ ] 添加 `start_screen_capture(target, fps) -> Result<ScreenKitCapture, ScreenKitError>` 接口；启动失败不得 panic。
- [ ] 提交：`feat: add ScreenCaptureKit capture interface`。

### Task 2: 实现 SCStream 最新帧槽与权限回退

**文件：**
- 修改：`src-tauri/src/webrtc/screencapturekit_capture.rs`
- 修改：`src-tauri/src/webrtc/mod.rs`

- [ ] 用 `SCShareableContent` 按 `CaptureTarget.id` 选择显示器，配置 BGRA、目标尺寸、FPS 和 `queue_depth=1`。
- [ ] 注册 `SCStream` screen output 回调；回调只替换容量为 1 的最新样本，绝不等待编码器。
- [ ] 检查 `SCStreamFrameInfo.status`，跳过 idle/incomplete 样本；记录 `displayTime` 作为帧年龄来源。
- [ ] 在 `SCREENMIRROR_USE_IOSURFACE=1` 时优先启动 ScreenCaptureKit；权限、像素格式、显示器枚举或启动失败时记录原因并回退 xcap。
- [ ] 编写 stride、偶数尺寸、旧帧替换的单测；提交：`feat: add latest-frame ScreenCaptureKit source`。

### Task 3: 原生 VideoToolbox IOSurface 编码器

**文件：**
- 创建：`src-tauri/src/webrtc/video_toolbox_iosurface.rs`
- 修改：`src-tauri/src/webrtc/video_toolbox.rs`
- 修改：`src-tauri/src/webrtc/mod.rs`

- [ ] 创建 `VTCompressionSession`，设置 realtime、平均码率、期望 FPS、最大关键帧间隔、禁用 frame reordering 和 lookahead。
- [ ] 通过 `CVPixelBuffer`/IOSurface 输入编码，不复制为 `RgbaImage`；回调复制 H.264 access unit 到拥有的 `Vec<u8>`，附带关键帧和采集时间戳。
- [ ] 将 SPS/PPS 转成现有 `H264EncodedFrame` 要求的 Annex B；关键帧必须带参数集。
- [ ] 实现 Drop、编码器 flush 和错误回收；编码失败返回错误而不是终止 Host。
- [ ] 添加仅 macOS 运行的 320x240 synthetic pixel-buffer 编码测试；提交：`feat: encode IOSurface frames with VideoToolbox`。

### Task 4: 接入异步 WebRTC 捕获循环

**文件：**
- 修改：`src-tauri/src/webrtc/mod.rs`
- 修改：`src-tauri/src/webrtc/host.rs`

- [ ] 在 `spawn_video_capture_loop` 中增加 ScreenCaptureKit 分支；容量为 1 的捕获槽连接到编码线程。
- [ ] IOSurface 编码器成功时走原生路径；编码器或捕获器异常时只对当前流回退到现有 RGBA 编码器。
- [ ] 保留 `captured_at`、150ms 旧帧丢弃、关键帧优先和现有 sink，不修改 WebRTC SDP 或 Viewer 播放状态机。
- [ ] `CaptureHandle::stop()` 必须停止 SCStream、关闭通道并回收 VTCompressionSession。
- [ ] 提交：`perf: use IOSurface capture and encoding path`。

### Task 5: 验证与默认策略

**文件：**
- 修改：`AGENTS.md`
- 修改：必要时 `tools/diag-frames-direct.js`

- [ ] 运行 `cargo check --release`、`cargo test --test devices`、`cd viewer && npm run build`、`git diff --check`。
- [ ] 使用 `SCREENMIRROR_USE_IOSURFACE=1 node tools/diag-frames-direct.js`，确认 `framesDecoded` 增长、丢包为 0、无编码队列增长。
- [ ] 用真实 Host/Viewer 计时器截图确认延迟 ≤500ms、High 分辨率不变、无持续跳帧。
- [ ] 只有真实 macOS 验收通过后，才把 IOSurface 设为默认；否则保持 xcap 默认并记录开关使用方式。
- [ ] 将采集时间戳、回退条件和验证命令补充到 `AGENTS.md`；提交：`docs: document IOSurface capture validation`。
