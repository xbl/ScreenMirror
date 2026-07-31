# IOSurface 低延迟采集设计

## 目标

在 macOS 13 及以上版本，将 Screenmirror 的 Host 到 Viewer 实际动作延迟
降低到不超过 500ms，同时保持当前 High 画质档和稳定帧率。

## 架构

使用 ScreenCaptureKit 的 `SCStream` 替换 macOS 下的
`xcap::video_recorder()`。Stream 回调只保留最新的 `CMSampleBuffer`，并从
其中取得 `CVPixelBuffer`/IOSurface 和采集时间戳；旧样本在编码前直接丢弃。

使用原生 VideoToolbox `VTCompressionSession` 编码实时 H.264，关闭帧重排
和 lookahead。编码器直接消费 IOSurface，输出带 SPS/PPS 的 Annex B access
unit，继续接入现有 WebRTC sink 和过期帧保护逻辑。

## 兼容与回退

- 通过 `SCREENMIRROR_USE_IOSURFACE=1` 启用新路径。
- ScreenCaptureKit 路径要求 macOS 13 或更高版本。
- 权限、显示器枚举、Stream 启动、像素格式或编码器初始化失败时，记录原因
  并自动回退到现有 xcap/FFmpeg 路径。
- 保留 `SCREENMIRROR_USE_VIDEO_RECORDER`，用于诊断回退路径。

## 数据流

`SCStream 回调 -> 容量为 1 的最新样本槽 -> VTCompressionSession ->
H264EncodedFrame -> 现有 RTP/WebRTC sink`

每个帧携带采集时间戳。非关键帧超过 150ms 时丢弃；需要恢复解码状态时保留
关键帧，但采集回调永远不会等待编码器。

## 验证标准

- Rust release 检查和设备集成测试通过。
- 无 UI 诊断持续解码、丢包为 0、没有编码队列增长。
- 真实 Host/Viewer 计时器截图显示端到端延迟不超过 500ms，High 分辨率保持
  不变，且没有持续跳帧。
