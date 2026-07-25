# AGENTS.md

> Operational guide for AI agents and humans working inside this `screenmirror/`
> subtree. The repo root `/Users/blxie/workspace/every-screen/` is **not** a
> git repository and contains unrelated directories (e.g. `deskreen/`) that
> must be ignored. All paths below are relative to `screenmirror/`.

## What this is

A Tauri 2 + Vue 3 + TypeScript app that shares a single macOS screen/window to
a browser viewer over WebRTC, with QR-based connection on the local network.
No Electron. No analytics. English + Simplified Chinese only.

- Host: Tauri 2 desktop binary (`src-tauri/`).
- Viewer: SPA loaded by the browser (`viewer/`).
- Media: `xcap` capture → VideoToolbox H.264 → `str0m` WebRTC → native `<video>`.

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
```

The pre-existing warnings, in case a reviewer flags them as new:

- `commands.rs:9` — unused import `Manager`
- `commands.rs:4` — unused import `ViewerSinkMap`
- `commands.rs:105` — unreachable expression after `app.restart()`
- `src-tauri/tests/permissions.rs:1-4` on macOS hosts only — `unused_imports`
  warning for `open_screen_recording_settings` (the import is unconditional
  but only used inside `#[cfg(not(target_os = "macos"))]`). The library
  build (`cargo check --release`) is unaffected.

## Git / commits

- **`/Users/blxie/workspace/every-screen/` is not a git repo.** Plan steps
  that say `git commit` do not apply. Produce a per-task implementation
  report (`.superpowers/sdd/task-N-report.md`) instead.
- The `tools/output/` directory is the E2E screenshot dump and should not be
  hand-edited.

## Where work artifacts live

| Concern | Path |
|---|---|
| Design specs | `../docs/superpowers/specs/*.md` (one level up) |
| Implementation plans | `../docs/superpowers/plans/*.md` |
| Subagent-driven-development brief/report/review | `.superpowers/sdd/task-N-*.md` |
| Plans produced by ZCode plan mode | `.zcode/plans/*.md` |

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