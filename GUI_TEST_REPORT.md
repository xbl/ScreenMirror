# GUI End-to-End Test Report

**Date**: 2026-07-22
**Goal**: Validate the host application, signaling service, H.264 capture path, and native browser video playback.

## Environment

The shell session does not provide an interactive macOS window automation context. The backend and browser bundle can still be built and tested independently.

## Current verification

- Rust unit and integration tests pass.
- Host and viewer TypeScript checks pass.
- Host and viewer production builds pass.
- The local FFmpeg installation exposes the macOS VideoToolbox H.264 encoder.
- The signaling service accepts a browser-style SDP offer and produces an SDP answer.
- The viewer requests a receive-only video section and binds the resulting `MediaStream` to `<video>`.

## Media pipeline

1. `xcap` captures the selected screen or window as RGBA pixels.
2. The macOS VideoToolbox H.264 encoder emits Annex-B access units.
3. `str0m` packetizes the samples on the negotiated RTP video writer.
4. The browser receives a WebRTC video track through `ontrack`.
5. The native video element reports its dimensions and playback time.

## Browser acceptance criteria

A real browser session is accepted when all of the following are true within ten seconds:

- `video.videoWidth > 0`
- `video.videoHeight > 0`
- `video.readyState >= 2`
- `video.currentTime` increases continuously
- no `blob:` media URL is present
- no image element is used for the shared screen

## Static and automated evidence

The checked-in E2E script probes the native video element, waits one second, and verifies that playback time advances. It also verifies that no external tracking resource is loaded and that the page contains no legacy product name.

The remaining manual step is opening the generated room URL in Safari or Chrome on the same LAN as the host and confirming the acceptance criteria above.
