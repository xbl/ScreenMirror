# Screenmirror Documentation

## Current architecture

| Area | Implementation |
| --- | --- |
| Host lifecycle | `src-tauri/src/lib.rs`, `commands.rs` |
| Host interface | `src/components/HostShell.vue`, `TopBar.vue`, `ConnectedDevicesListDrawer.vue` |
| Connection | `src/components/QRCard.vue`, `src-tauri/src/signaling/handlers.rs` |
| Source selection | `src/components/SourcePicker.vue`, `src-tauri/src/webrtc/mod.rs` |
| Signaling | `src-tauri/src/signaling/*` |
| Browser viewer | `viewer/src/views/MainView.vue`, `PlayerView.vue`, `components/*` |
| Playback | Native HTML `<video>` with `MediaStream` |
| Localization | `src/i18n/*` and `viewer/src/i18n/*` |
| Permissions | `src-tauri/src/permissions.rs` and permission UI |
| Network | `src-tauri/src/network.rs` |

## Media pipeline

1. `xcap` captures the selected monitor or window as RGBA pixels.
2. The macOS VideoToolbox H.264 encoder produces Annex-B access units.
3. `str0m` negotiates a video media section and packetizes samples into RTP.
4. The browser receives a `MediaStream` through `RTCPeerConnection.ontrack`.
5. The viewer binds the stream to a native `<video>` element.

## Signaling

The Axum server exposes the room HTTP endpoint and WebSocket signaling channel. The viewer sends an SDP offer containing a receive-only video section. The host accepts the offer, records the negotiated video MID, and sends encoded samples through the corresponding RTP writer.

## Privacy

The application keeps screen frames on the local network and does not collect usage data or send media to third parties.

## Scope

The project intentionally focuses on local screen sharing, QR-based connection, a single viewer slot, screen/window selection, and English/Simplified Chinese interfaces. Paid upgrades, remote analytics, external tracking, and unrelated account features are outside the project scope.
