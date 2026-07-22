# Screenmirror

A Tauri 2 + Vue 3 + TypeScript screen sharing application written in Rust without Electron.

## Features

- Screen / window capture on macOS
- Browser viewer via WebRTC and native HTML video
- Local-network signaling via WebSocket and JSON messages
- Single viewer slot
- QR-code-based connection URL
- CLI flags: `--ip <addr>` and `--port <n>`
- English and Simplified Chinese interfaces

## Privacy

Screenmirror keeps signaling and media on the local network. It does not collect usage data, run tracking scripts, or send frames to third parties.

## Quick start

```bash
npm install
cd viewer && npm install && cd ..
cd src-tauri && cargo fetch && cd ..
npm run tauri:dev
```

Open the app and scan the QR code with a device on the same Wi-Fi.

## Tests

```bash
cd src-tauri && cargo test
npm run typecheck
npm run build
cd viewer && npm run typecheck && npm run build
```

## Build

```bash
npm run tauri:build
```

## CLI flags

```bash
./screenmirror --ip 192.168.1.100 --port 3133
```

## Architecture

The host captures a screen or window with `xcap`, encodes frames as H.264 with the macOS VideoToolbox encoder, and sends them through a negotiated WebRTC video track. The browser binds the received `MediaStream` to a native `<video>` element.
