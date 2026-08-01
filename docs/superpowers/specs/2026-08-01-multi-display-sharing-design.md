# Multi-display sharing design

## Goal

Allow a host with one or more displays to choose exactly one display to share.
The interaction should resemble AirPlay's source chooser: three source groups
(`Entire screen`, `Window or App`, and `Extended display`), with the display group
showing selectable entries for each connected monitor. The local-network
WebRTC viewer remains a single continuous stream.

## User experience

- On startup, enumerate displays and windows after the screen-recording
  permission check.
- Render a compact source chooser with the three AirPlay-style groups. The
  selected display entry shows a small preview, display name, resolution, and a
  primary-display marker. No preview is used as the encoded stream.
- Selecting a display before sharing sets the next capture target. Selecting a
  different display while sharing applies immediately without a new QR scan or
  WebRTC renegotiation.
- If a selected display disappears or capture permission is lost, keep the
  current stream alive, report the selection error, and leave the previous
  target active.

## Architecture

`CaptureSourceInfo` gains a stable source identifier, `is_primary`, and an
optional preview payload. macOS enumeration derives display metadata from xcap
and maps it to ScreenCaptureKit's display identifier; index fallback is retained
for older APIs and non-macOS stubs.

`set_capture_target` updates both the pending target and the active host peer.
The peer owns a replaceable capture handle. A target change stops the old loop,
discards queued frames, creates a new encoder when dimensions change, and starts
the new loop while preserving the socket, RTP writer, and viewer connection.
The first keyframe from the new target is allowed through the existing stale-frame
gate.

## Failure handling

Enumeration failures return an empty state with an actionable permission/error
message. A failed target switch is transactional: the old capture handle and
target remain active. Display removal is detected both during enumeration and
when a capture loop reports an out-of-range target.

## Verification

- Rust unit tests cover stable display mapping, target replacement, rollback on
  failed capture, and non-macOS enumeration stubs.
- Vue tests cover group rendering, selected display metadata, and disabled/error
  states.
- Run `cargo check --release`, relevant Rust tests, `npx vue-tsc --noEmit`, and
  `cd viewer && npm run build`.
- Run the real-screen headless diagnostic with two displays when available;
  verify continuous frames, no Viewer watchdog errors, and target switch below
  500 ms. A single-display environment must still pass the existing diagnostic.
