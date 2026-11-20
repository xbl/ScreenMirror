# AGENTS.md

> Operational guide for AI agents and humans working inside this `screenmirror/`
> subtree. This **is** a git repository (rooted at `screenmirror/`, branch:
> All paths below are relative to `screenmirror/`.

## What this is

A Tauri 2 + Vue 3 + TypeScript app that shares a single macOS screen/window to
a browser viewer over WebRTC, with QR-based connection on the local network.
No Electron. No analytics. English + Simplified Chinese only.

- Host: Tauri 2 desktop binary (`src-tauri/`).
- Viewer: SPA loaded by the browser (`viewer/`).
- Media: `xcap` capture → VideoToolbox H.264 → `str0m` WebRTC → native `<video>`.

## Product Positioning & Priorities

ScreenMirror is a **local-network extended-display tool**: any device that can
open a browser should be usable as a responsive second screen. Treat the LAN as
the normal deployment environment, not as a constrained WAN link.

- Optimize for interactive latency first; target end-to-end motion-to-photon
  latency below 500 ms.
- Prefer dropping stale frames over displaying an old frame. Do not add large
  jitter, playback, or encoder queues to make motion look smoother.
- Use the available LAN bandwidth for readable text and high-resolution screen
  content. Preserve at least 30 fps when the host can sustain it.
- High uses 1920px at 15fps to balance readable text with browser decode load;
  Ultra is opt-in at 3840px/15fps for devices that can sustain it.
- Quality controls belong on the host, where capture and H.264 settings are
  decided. Viewer controls must not pretend to change encoding unless they
  renegotiate the stream.
- `RTCRtpReceiver.jitterBufferTarget` is expressed in milliseconds; `0`
  requests no target buffering. `playoutDelayHint` is expressed in seconds;
  use `0` when supported. Keep both at zero for this LAN display.
- Validate latency with a moving clock or timer on the host and viewer, not
  only with a successful WebRTC connection or a rendered test pattern.
- On macOS, seed each stream with one direct `capture_image()` frame, then use
  `video_recorder()` for stable ongoing frame pacing. Set
  `SCREENMIRROR_USE_VIDEO_RECORDER=0` to force direct polling for diagnostics;
  it can reduce latency on some systems but may cause dropped frames at high
  resolution.

## Sub-projects (work in both, treat as one product)

| Sub-project | Path | Manager |
|---|---|---|
| Host SPA (Vue) | `src/` | `npm` |
| Tauri backend | `src-tauri/` | `cargo` |
| Viewer SPA | `viewer/` | its own `npm` |

`viewer/` is a **separate npm project** with its own `package.json`,
`node_modules/`, and `tsconfig.json`. Run `cd viewer && npm install` once
when the workspace is set up. **Do not** treat `viewer/dist/` as build output
of `npm run build` from the root — it is owned by the viewer's own build.

## Layout cheat-sheet

```
screenmirror/
├── src/                       # Host SPA (Vue + Tauri IPC)
│   ├── components/            # HostShell.vue, SourcePicker.vue, StartButton.vue, ...
│   ├── i18n/                  # en.ts, zh-CN.ts  (object literal exports)
│   └── utils/api.ts           # Typed wrappers around invoke('..._command')
├── src-tauri/
│   ├── src/
│   │   ├── permissions.rs     # macOS CGPreflight/CGRequest FFI + open settings
│   │   ├── commands.rs        # #[tauri::command] wrappers
│   │   └── lib.rs             # generate_handler! macro
│   ├── tests/                 # cargo integration tests (e.g. permissions.rs)
│   ├── tauri.conf.json        # Bundle id: dev.screenmirror.app
│   └── Cargo.toml
├── viewer/
│   ├── src/
│   │   ├── views/PlayerView.vue        # The fragile one
│   │   ├── views/MainView.vue          # Hosts ConnectionPrompts + PlayerView (v-if)
│   │   ├── components/EarlyOffer.vue   # RTCPeerConnection + viewer-stream event
│   │   ├── lib/viewerStatus.ts         # status state machine
│   │   └── i18n/                       # en.ts, zh-CN.ts
│   └── vite.config.ts
├── tests/e2e/                 # Full end-to-end tests (Puppeteer/WebRTC)
├── tools/                     # Headless diagnostics, screenshots, and assets
│   ├── verify-fix.js          # Main "frames render?" gate
│   └── output/                # Screenshots land here (gitignored)
├── tests/                     # vue/TS unit tests (vitest)
└── DOCS.md                    # Architecture overview (read this too)
```

## Conventions

### Vue 3 + `<script setup lang="ts">`

- No Options API. Every component is `<script setup lang="ts">`.
- i18n keys live in `src/i18n/{en,zh-CN}.ts` and `viewer/src/i18n/{en,zh-CN}.ts`.
  Locale files are plain `export default {}` object literals — there is no
  schema type, so adding/removing keys does **not** break typecheck. The
  typecheck only flags consumers that still reference a removed key when
  the i18n helper is statically typed (it isn't here). Always grep consumers
  before deleting a key.
- `provide`/`inject` uses a `unique symbol` (e.g. `export const MyKey: InjectionKey<...> = Symbol('MyKey')`)
  exported from a sibling `*.ts` or types file to guarantee type safety across
  components.

### SourcePicker state and previews

- `CommandState.capture_target` is the single source of truth for the active
  share target. Tauri events update the tray immediately; while the tray is
  mounted, a lightweight target-only reconciliation is required as a fallback
  for a missed cross-WebView event. It must not enumerate sources unless the
  target has actually changed.
- `set_capture_target` must finish the target switch before the standalone
  picker closes, then emit `capture-target-changed` with the complete
  `CaptureTargetState`. The embedded tray `SourcePicker` refreshes its card
  from that event.
- `SourcePicker` has two instances: the embedded tray card and the standalone
  picker window. They must not synchronize by copying refs or guessing from
  stale local state. Use Tauri events for cross-window changes and the command
  state for restoration.
- Preview requests must include the source kind. Display and window/App
  previews use separate cache keys; never show a display preview for a window
  source. During a pending request, show `source.previewLoading`; use
  `source.noPreview` only after the bounded capture attempt has failed.
- Window thumbnail captures (especially from Electron, Chromium, Terminal, Feishu)
  frequently contain an alpha channel. Thumbnail generation must composite
  RGBA pixels against an opaque background canvas (e.g. `#1e222b`) before
  encoding to JPEG; stripping alpha directly creates pitch-black images.
  Guard thumbnails with `is_visible_preview` to detect all-black frames and fall
  back to icons.
- System resource lifecycle must be strictly decoupled from the active share target:
  macOS virtual displays are system-level hardware displays. UI controls for
  creating or destroying virtual displays must be driven by global resource
  existence (`hasVirtualDisplay`), NEVER by `selectedSource?.isVirtual`. Users
  must be able to remove created virtual screens regardless of whether they are
  currently sharing the virtual screen, the primary display, or an app window.
- When destroying system resources (such as virtual displays), the frontend must
  optimistically update its local reactive state immediately (0ms visual feedback)
  rather than waiting for an IPC round-trip to re-enumerate sources.
- A source-selection change is verified with a real Tauri flow: open the tray,
  select an App, confirm the target name and preview change in the tray card,
  then switch back to the primary display. Vue tests alone do not cover the
  cross-WebView, xcap, or window-close ordering involved here.

### Rust (Tauri)

- Bundle id is **`dev.screenmirror.app`** (from `src-tauri/tauri.conf.json`).
  Do not rename without updating macOS deep links and code-signing.
- macOS Screen Recording permission uses `CGPreflightScreenCaptureAccess` to
  read state and `CGRequestScreenCaptureAccess` as a best-effort nudge. The
  reliable recovery path is `open_screen_recording_settings()`, which shells
  `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`
  via `std::process::Command::new("open")`. Do **not** route through
  `tauri-plugin-opener` — it can reject the private scheme.
- **macOS 15 (Sequoia) Preflight False-Negative Trap & TCC Fallback**:
  On macOS 15+ Sequoia with ad-hoc signed binaries (`codesign --sign -`),
  `CGPreflightScreenCaptureAccess()` has a known OS-level bug returning `false`
  even when the user has explicitly enabled screen recording in System Settings.
  Never rely on `CGPreflightScreenCaptureAccess()` alone. Always pair it with an
  authoritative runtime window-inspection fallback (`xcap::Window::all()` verifying
  non-empty cross-process window titles or app names, which macOS TCC strips when
  denied).
- **TCC List Registration**: Before opening System Settings, always invoke
  `CGRequestScreenCaptureAccess()` (`request_and_open_screen_recording_settings`).
  Opening settings without this nudge leaves the app absent from the macOS
  "Screen & System Audio Recording" list, leaving the user with nothing to toggle.
- **DMG Application Drag Guide**: The DMG bundle must configure an `/Applications`
  symlink (`app_drop_link_path: "/Applications"`) so users dragging the `.app`
  install into `/Applications` rather than running from temporary / Translocation
  directories, avoiding quarantine and TCC bundle ID mismatches.
- New Tauri commands go in `commands.rs`, registered in `lib.rs`'s
  `tauri::generate_handler!` macro (alphabetical order).
- Cross-platform code uses `#[cfg(target_os = "macos")]` and a paired
  `#[cfg(not(target_os = "macos"))]` stub returning `true` / `Ok(())`.
- Module visibility: In library crates (`src/lib.rs`), declare internal
  modules with `pub(crate) mod` rather than `pub mod`. `pub mod` marks all
  inner items as public API exported to outside consumers, which silences
  `rustc`'s `dead_code` warnings and lets unused models rot undetected.
- Concurrency & synchronization: Never use `tokio::time::sleep` or frontend
  `setTimeout` to mask async races or "wait" for signaling/WebRTC state.
  Always synchronize via deterministic channels, `tokio::sync::Notify`, or
  explicit protocol ACK messages.
- WebRTC Socket Error Tolerance: In `host.rs`'s WebRTC event loop, socket
  errors from `socket.recv_from` (such as `os error 49 Can't assign requested address`,
  `ECONNREFUSED`, `EHOSTUNREACH`) caused by auxiliary or virtual interfaces (e.g.
  `utun*`, `bridge100`) must be logged as warnings and continued, NEVER breaking
  the loop.
- LAN Interface Priority: Virtual interfaces (`utun*`, `bridge*`, `tun*`, `tap*`,
  `vbox*`) must be filtered out when binding/publishing signaling endpoints and
  SDP candidates. Always bind to physical private LAN IPv4 interfaces (`en0`, `en1`).

### macOS Virtual Displays (CGVirtualDisplay) & Lifecycle

- **WindowServer Asynchronous Teardown & Tombstoning**:
  macOS WindowServer deregisters displays (`CGVirtualDisplay`) asynchronously.
  Releasing the Objective-C object or calling `sm_virtual_display_destroy`
  returns immediately, but `CGGetActiveDisplayList` / `xcap::Monitor::all()` may
  still report the display for tens to hundreds of milliseconds.
  - The backend must track destroyed display IDs (`s_destroyed_ids`) and filter
    them out inside `enumerate_sources()`.
  - Feature-code filter: ScreenMirror virtual displays use product ID `0x534D`
    (decimal 21325, displayed as `Display #21325`). Any monitor with model number
    `0x534D` that is NOT actively registered in `s_active_displays` is an
    asynchronously terminating or orphaned phantom display. It must NEVER be
    returned as a physical "扩展显示器" (Extended Display); doing so causes
    destroyed cards to resurrect without their delete buttons.
- **Optimistic UI Updates for Hardware Destruction**:
  Frontend destruction actions (such as clicking `✕` on a virtual screen card)
  must optimistically remove the target from local reactive state (`sources.value`)
  immediately to provide 0ms visual feedback, rather than waiting for an IPC
  round-trip to re-enumerate sources and racing OS deregistration.
- **The Mode Change Trap**:
  Never change the display mode of a `CGVirtualDisplay` after creation via
  `CGDisplaySetDisplayMode`. CoreGraphics has an internal bug where mode-altered
  virtual displays are never reclaimed by WindowServer upon object deallocation,
  permanently leaking the virtual monitor until the host process completely exits.
  Always instantiate the display with its desired final resolution directly.

### PlayerView.vue — fragile, read before editing

The viewer playback state machine is in
`viewer/src/views/PlayerView.vue` and has been re-broken at least twice.
Rules learned the hard way:

- `<video>` is `v-if="status === 'streaming'"`. Status is `'connecting'` on
  mount, flips to `'streaming'` from `onStream` (the `viewer-stream` window
  event handler).
- The `<video>` template ref (`videoEl`) is the **only** attachment point.
  `onStream` must cache the stream into `pendingStream.value` **before**
  calling `markStreaming()`. Never `attachStream` synchronously inside
  `onStream` — that races the v-if transition and you get
  `play() rejected: AbortError` followed 5 s later by the `noFrames`
  watchdog.
- The attachment watcher is `flush: 'post'` so it runs after Vue patches
  the DOM. Do not change to `'pre'` or `'sync'`.
- The 5 s watchdog calls `markDisconnected()` if `videoWidth === 0`. It
  clears itself on `loadedmetadata`. Don't disable it.
- `MainView.vue` gates `PlayerView` itself behind `v-if="streaming"`. There
  is therefore a **two-level** v-if (outer in MainView, inner for `<video>`).
  Don't collapse them without rethinking the attach race.

## Verification gates (run before claiming a task done)

Consult `checks.md` for the official multi-layer checklist. Git hooks enforce
the standard gates automatically:
- `.husky/pre-commit`: ESLint, `vue-tsc` (Host & Viewer), Vitest, and `cargo check`.
- `.husky/pre-push`: Full Rust test suite (`cargo test`).

Manual gates before committing:

```bash
# Rust
cd src-tauri && cargo check --release
cd src-tauri && cargo test

# Viewer (its own tsconfig + package.json)
cd viewer && npx vue-tsc --noEmit -p tsconfig.json   # exit 0
cd viewer && npm run build                           # produces viewer/dist/

# Host
cd ..    && npx vue-tsc --noEmit                     # exit 0
npm run build                                        # may fail with rollup native-binary
                                                     # MODULE_NOT_FOUND — pre-existing env
                                                     # issue, not a code regression

# Headless E2E (real Tauri binary + headless Chrome)
node tools/verify-fix.js
# Expect: "VERDICT: ✅ Frames rendered AND canvas has visible non-black pixels"
# Exit 0 = pass, 1 = fail. Screenshot lands at tools/output/verify-fix.png.

# Deep headless diagnostic: captures WebRTC stats and first-change timing
node tools/diag-frames-direct.js
# Prefer this for latency/no-frame investigations; it does not require opening
# the Tauri UI. It writes tools/output/diag-direct-trace.json and screenshots.
```

Headless verification is the default for media changes. `diag-frames-direct.js`
starts the real Tauri binary and a fresh headless Chrome profile, then records
`framesDecoded`, jitter-buffer delay, packet loss, and the first decoded change.
For the extended-display target, validate latency with a moving host clock or
timer rather than treating a successful WebRTC handshake as proof of quality.
Always rebuild `viewer/dist/` before trusting headless output. If Vite/Rollup
fails because an optional native dependency is missing, treat any existing
`viewer/dist/` as stale and do not claim the new viewer code was verified.

Synthetic test caution: `SCREENMIRROR_CAPTURE=test` and `127.0.0.1` verify
signaling and basic pipeline plumbing, but bypass real macOS display capture
(ScreenCaptureKit), GPU encoding, multi-interface routing, and real-world
browser network policies. NEVER claim a user-reported streaming/connectivity
bug is fixed based solely on a synthetic headless test. When a user reports
that a fix does not work on their machine, do NOT assume code correctness from
headless tests: inspect the live environment first (e.g. `pc.connectionState`,
candidate protocol types, browser extensions like `WebRTC Leak Prevent`
disabling UDP via `disable_non_proxied_udp`).

For source-picker changes, also run a real macOS/Tauri UI check. The minimum
scenario is: open the tray, verify the primary display preview, choose an App
in the standalone picker, verify that the tray card changes to that App and
its preview, then switch back to the primary display. Do not claim this flow
is verified from Vue unit tests alone.

The pre-existing warnings, in case a reviewer flags them as new:

- `commands.rs:9` — unused import `Manager`
- `commands.rs:4` — unused import `ViewerSinkMap`
- `commands.rs:105` — unreachable expression after `app.restart()`
- `src-tauri/tests/permissions.rs:1-4` on macOS hosts only — `unused_imports`
  warning for `open_screen_recording_settings` (the import is unconditional
  but only used inside `#[cfg(not(target_os = "macos"))]`). The library
  build (`cargo check --release`) is unaffected.

## Git / commits

- `screenmirror/` is a git repo. `git add` / `git commit` work normally here.
- The `tools/output/` directory is the E2E screenshot dump and is gitignored;
  do not hand-edit or commit it.
- **Explicit staging only**: Never run `git add .` or `git add -A`. The working
  directory often contains pre-existing edits or diagnostics. Always stage
  specific modified paths.
- Pre-commit hooks (`.husky/pre-commit`) verify lint, types, vitest, and cargo
  check on every commit. Never bypass with `--no-verify`.

## Subagent and CLI Best Practices

- **Never grep `src-tauri` without excluding `target/`**: Running `grep -r` on
  `src-tauri/` traverses hundreds of megabytes of build artifacts in
  `src-tauri/target/`, causing commands to hang for minutes or time out. Always
  scope searches to `src-tauri/src/` or pass `--exclude-dir=target` (or use `rg`,
  which respects `.gitignore`).
- **Inspect `git status` at subagent startup**: When spawning a subagent in an
  existing workspace, inspect `git status` first to recognize pre-existing
  uncommitted files versus files modified by the task. Touch only files
  explicitly assigned in the task brief.
- **Port 3131 zombie hygiene**: E2E runs (`verify-fix.js` / `diag-frames-direct.js`)
  spawn Tauri binaries and headless Chrome. If an E2E run is interrupted or
  fails, port 3131 may stay bound. Always check `lsof -nP -iTCP:3131 -sTCP:LISTEN`
  and terminate lingering processes (`pkill -9 -f screenmirror`) before testing.
- **Viewer bundle synchronization**: `viewer/dist/` is the single source of
  truth served by the host Tauri binary. Any changes inside `viewer/` MUST be
  followed by `(cd viewer && npm run build)` before running headless E2E.
- **Dead code detection in Rust and Vue**:
  - In Rust: Use `pub(crate)` for internal modules in `src/lib.rs` so `rustc`
    flags dead code. Use `cargo-machete` to find unused dependencies.
  - In Frontend: Run `npx knip` inside `viewer/` or root to identify unused
    components, dead exports, and orphaned dependencies.

## Diagnosing "viewer no frames after 5s" — start here

The symptom `[player-view] no frames after 5s; videoWidth=0 readyState=0` in
the viewer is **the same symptom** for many distinct root causes. Before
editing PlayerView.vue or the WebRTC handshake, run:

```bash
# 1. Make sure no zombie screenmirror is squatting on the port.
lsof -nP -iTCP:3131 -sTCP:LISTEN
# If anything is listed, kill it (it may be from a previous crashed e2e run):
pkill -9 -f "aarch64-apple-darwin/debug/screenmirror"
pkill -9 -f "Google Chrome.*headless"

# 2. Run the direct diagnostic that drives both ends:
node tools/diag-frames-direct.js
# Look for in /tmp/diag-run*.log:
#   tauri log: "smoke room registered" THEN "signaling server on 0.0.0.0:3131"
#   tauri log: "WS handler entered" + "WS throttle passed for room=..., taken_snapshot=[...], requested_match=true"
#   viewer: "[early-offer] video track received"
#   tauri log: "host: MediaAdded mid=Mid(0) direction=SendOnly" + "host: starting capture loop"
#   diag trace: w=320x180 rs=4 pixR=255G=255B=255
# If any of those is missing, that's the actual broken link.
```

**Common non-code causes** that produce the "no frames" symptom but are NOT
PlayerView bugs:
- Browser privacy extensions disabling local UDP: If `pc.connectionState`
  remains stuck in `'new'` or `'checking'`, check candidate types. If candidates
  contain ONLY TCP and ZERO UDP candidates, check if a browser extension (such
  as *WebRTC Leak Prevent*) has set `rtcIPHandling: 'disable_non_proxied_udp'`
  or similar. ScreenMirror requires direct local UDP sockets; blocking non-proxied
  UDP breaks LAN WebRTC entirely.
- ScreenCaptureKit static screen cold-start: If ScreenCaptureKit reports
  `SCFrameStatus::Idle` continuously on static screens, the initial idle frame
  must be allowed through or synchronous fallback capture triggered, otherwise
  the viewer watchdog will fire before any frame is transmitted.
- Zombie `aarch64-apple-darwin/debug/screenmirror` holding port 3131 from a
  prior crashed e2e run → ws gets `NOT_ALLOWED` and closes immediately, no
  stream ever
  arrives.
- Chrome not actually launching under puppeteer (no `--user-data-dir`) → no
  viewer-side console at all; this looks like "nothing happens."
- Stale `viewer/dist/` (vue-tsc or vite cache) → fixes were committed but
  never rebuilt into the bundle the host serves. Run
  `cd viewer && npm run build` after every frontend change.

**When the diagnostic above shows `WS rejected: room not taken`** even though
`SCREENMIRROR_TEST_ROOM` is set, suspect a zombie binary from a previous
session listening on the port and answering `NOT_ALLOWED` before your fresh
binary can. Kill it and re-run.

## Where work artifacts live

| Concern | Path |
|---|---|
| Design specs | `../docs/superpowers/specs/*.md` (one level up) |
| Implementation plans | `../docs/superpowers/plans/*.md` |
| Subagent-driven-development brief/report/review | `.superpowers/sdd/task-N-*.md` |
| Plans produced by ZCode plan mode | `.zcode/plans/*.md` |
| Headless E2E (real host + real Chrome) | `tools/diag-frames-direct.js` (use this first when diagnosing "no frames") |

When picking up a plan, read the spec first, then the plan, then the brief —
in that order. Spec → plan → brief form a contract chain.

## Things to never do without checking first

- Renaming or splitting `PlayerView.vue` / `MainView.vue` — the v-if + watcher
  ordering is load-bearing.
- Switching the `<video>` back to `v-show` — this was the original bug.
- Routing the screen-recording settings URL through `tauri-plugin-opener`.
- Adding more than one IPC handler per Rust function — each `#[tauri::command]`
  wraps exactly one `crate::permissions` / `crate::network` / etc. function.
- Editing `viewer/src/views/PlayerView.vue`'s `onStream` order without
  re-running `tools/verify-fix.js`.
- Adding `tokio::time::sleep` or `setTimeout` to mask async races or "wait" for
  signaling/media state — use channels or `Notify` instead.
- Breaking the WebRTC event loop on transient UDP `socket.recv_from` errors (e.g. OS error 49).
- Claiming a user-reported streaming issue is resolved based purely on synthetic `SCREENMIRROR_CAPTURE=test` test runs.
- Running un-scoped `grep -r` or `find` on `src-tauri/` without `--exclude-dir=target`.
- Declaring internal crate modules as `pub mod` in `src/lib.rs` (use `pub(crate) mod`
  to let `rustc` catch dead code).
- Staging changes with `git add .` or `git add -A` (stage explicitly by path).
- Committing `viewer/` changes without rebuilding `viewer/dist/`.
- Assuming OS-level resource destruction (such as `CGVirtualDisplay`) is
  synchronously reflected in display enumeration (`Monitor::all()`).
- Returning a ScreenMirror virtual display (`productID == 0x534D` / `Display #21325`)
  as a physical extended monitor when it is not in the active registry.
- Relying solely on `CGPreflightScreenCaptureAccess` without runtime window inspection fallback on macOS Sequoia (causes false "permission not granted" persistent lockouts).
- Directing users to macOS System Settings Screen Recording without first calling `CGRequestScreenCaptureAccess` to register the app in the TCC list.
- Modifying the display mode of a `CGVirtualDisplay` after creation via
  `CGDisplaySetDisplayMode` (causes permanent leak in WindowServer).
- Tying system-level hardware resource creation or destruction UI strictly to the
  currently selected share target (`selectedSource?.isVirtual`).
