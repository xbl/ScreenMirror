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

1. ScreenCaptureKit (or the xcap fallback) captures the selected source once.
2. The macOS VideoToolbox H.264 encoder produces one shared Annex-B stream per
   viewer capability tier: mobile (1280px) and the host-selected desktop tier.
3. Each WebRTC peer receives the newest frame from its tier through its own bounded queue.
4. `str0m` packetizes samples into the peer's RTP session.
5. The browser receives a `MediaStream` through `RTCPeerConnection.ontrack`.

## Capture sources and live switching

The host lists available capture sources in three groups: the primary display,
windows and apps, and extended displays. A source has both a legacy
index-based `id` and a native `sourceId`; selection, refresh reconciliation,
and capture use the stable `sourceId`. The legacy value remains only as a
fallback for older callers.

The primary display is selected by default when available. Display thumbnails
are requested after source metadata is shown, so preview capture never blocks
the initial selection or a source change. Previews are compact JPEG data URLs,
bounded to 320 px on their longest edge, cached per source and dimensions, and
refreshed asynchronously. A preview error or unavailable image leaves the
source usable with the "No preview available" fallback. Refresh generations
prevent a late, stale preview from overwriting a newer result.

Changing source or quality while a viewer is connected does not require a new
viewer connection. The host prepares a capture for the requested target,
checks that it has produced a startup keyframe, validates every active peer,
then commits the new capture before stopping the previous one. If preparation
or validation fails, the current share and the UI selection remain unchanged
and the host reports the switch error. Sources that disappear during refresh
are reported without discarding the previously selected preview.

## Signaling

The Axum server exposes the room HTTP endpoint and WebSocket signaling channel. The viewer sends an SDP offer containing a receive-only video section. The host accepts the offer, records the negotiated video MID, and sends encoded samples through the corresponding RTP writer.

## Privacy

The application keeps screen frames on the local network and does not collect usage data or send media to third parties.

## Verification

Run checks from the repository root unless a command changes directory:

```bash
(cd src-tauri && cargo check --release)
(cd src-tauri && cargo test --test permissions)
(cd src-tauri && cargo test --test capture_sources)

(cd viewer && npx vue-tsc --noEmit -p tsconfig.json)
(cd viewer && npm run build)

npx vue-tsc --noEmit
npm run build
npx vitest run tests/SourcePicker.spec.ts

node tools/diag-frames-direct.js
```

`diag-frames-direct.js` starts the real host with its synthetic moving capture
pattern and a fresh headless Chrome profile. Its final trace reports the
viewer dimensions, `framesDecoded`, `framesReceived`, `packetsLost`, and
jitter-buffer counters; it also writes viewer and decoded-frame screenshots
when a changing frame is observed. This is a media-path diagnostic, not a
host-UI source-switch driver: exercise a physical display or window switch
from the host picker, while watching that the existing viewer continues to
decode frames. The generated trace and screenshots are in `tools/output/`
and are intentionally ignored by git.

## Scope

The project intentionally focuses on local screen sharing, QR-based multi-viewer connection, screen/window selection, and English/Simplified Chinese interfaces. Paid upgrades, remote analytics, external tracking, and unrelated account features are outside the project scope.
