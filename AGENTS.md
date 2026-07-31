# AGENTS.md

> Operational guide for AI agents and humans working inside this `screenmirror/`
> subtree. This **is** a git repository (rooted at `screenmirror/`, branch:
> `master`). The parent directory `/Users/blxie/workspace/every-screen/`
> contains unrelated directories (e.g. `deskreen/`) that must be ignored.
> All paths below are relative to `screenmirror/`.

## What this is

A Tauri 2 + Vue 3 + TypeScript app that shares a single macOS screen/window to
a browser viewer over WebRTC, with QR-based connection on the local network.
No Electron. No analytics. English + Simplified Chinese only.

- Host: Tauri 2 desktop binary (`src-tauri/`).
- Viewer: SPA loaded by the browser (`viewer/`).
- Media: `xcap` capture → VideoToolbox H.264 → `str0m` WebRTC → native `<video>`.

## Product Positioning & Priorities

Screenmirror is a **local-network extended-display tool**: any device that can
open a browser should be usable as a responsive second screen. Treat the LAN as
the normal deployment environment, not as a constrained WAN link.

- Optimize for interactive latency first; target end-to-end motion-to-photon
  latency below 500 ms.
- Prefer dropping stale frames over displaying an old frame. Do not add large
  jitter, playback, or encoder queues to make motion look smoother.
- Use the available LAN bandwidth for readable text and high-resolution screen
  content. Preserve at least 30 fps when the host can sustain it.
- The default High profile is capped at 1920px because larger VideoToolbox
  frames can block encoding for hundreds of milliseconds; Ultra is opt-in for
  higher resolution when the host can sustain it.
- Quality controls belong on the host, where capture and H.264 settings are
  decided. Viewer controls must not pretend to change encoding unless they
  renegotiate the stream.
- WebRTC receiver delay properties use seconds: `jitterBufferTarget = 0.05`
  means 50 ms, while `0` requests no target buffering. Pair it with
  `playoutDelayHint = 0` when supported; never use a millisecond integer such
  as `50`.
- Validate latency with a moving clock or timer on the host and viewer, not
  only with a successful WebRTC connection or a rendered test pattern.
- On macOS, seed each stream with one direct `capture_image()` frame, then use
  `video_recorder()` for ongoing changes. This avoids recorder startup starvation
  while preserving higher throughput; set `SCREENMIRROR_USE_VIDEO_RECORDER=0`
  to force direct polling for diagnostics.

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
├── tools/                     # Headless E2E + diagnostics (puppeteer-core)
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
- `provide`/`inject` uses a `unique symbol` exported from a sibling
  `*.ts` file (see `src/components/PermissionModalHost.ts` for the pattern).
  The injection key file exports the symbol **and** the `Ref<... | null>`
  type so consumers can pass either `ref(null)` or `{ value: null }` as the
  fallback.

### Rust (Tauri)

- Bundle id is **`dev.screenmirror.app`** (from `src-tauri/tauri.conf.json`).
  Do not rename without updating macOS deep links and code-signing.
- macOS Screen Recording permission uses `CGPreflightScreenCaptureAccess` to
  read state and `CGRequestScreenCaptureAccess` as a best-effort nudge. The
  reliable recovery path is `open_screen_recording_settings()`, which shells
  `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`
  via `std::process::Command::new("open")`. Do **not** route through
  `tauri-plugin-opener` — it can reject the private scheme.
- New Tauri commands go in `commands.rs`, registered in `lib.rs`'s
  `tauri::generate_handler!` macro (alphabetical order).
- Cross-platform code uses `#[cfg(target_os = "macos")]` and a paired
  `#[cfg(not(target_os = "macos"))]` stub returning `true` / `Ok(())`.

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

```bash
# Rust
cd src-tauri && cargo check --release                # expect 3 pre-existing unused-import warnings
cd src-tauri && cargo test --test permissions        # 3/3 must pass on non-macOS

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

## Diagnosing "viewer no frames after 5s" — start here

The symptom `[player-view] no frames after 5s; videoWidth=0 readyState=0` in
the viewer is **the same symptom** for many distinct root causes. Before
editing PlayerView.vue or the WebRTC handshake, run:

```bash
# 1. Make sure no zombie screenmirror is squatting on the port.
lsof -nP -iTCP:3131 -sTCP:LISTEN
# If anything is listed, kill it (it may be from a previous crashed e2e run):
pkill -9 -f "target/debug/screenmirror"
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
- Zombie `target/debug/screenmirror` holding port 3131 from a prior crashed
  e2e run → ws gets `NOT_ALLOWED` and closes immediately, no stream ever
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
