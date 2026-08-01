---
name: automated-testing
description: Test Screenmirror Vue, Rust, WebRTC, and Tauri boundaries with the narrowest reliable layer.
---

# Screenmirror Automated Testing

Use this skill when changing or debugging `src/`, `viewer/`, `src-tauri/`, `tests/`, `src-tauri/tests/`, or `tools/`, and when validating a streaming, signaling, playback, or permission workflow.

## Workflow

1. Read `AGENTS.md` and `DOCS.md`, inspect the changed path and existing tests, and preserve unrelated worktree changes.
2. Classify the change and choose the narrowest effective test layer:

| Change | First test layer | Add broader coverage when |
| --- | --- | --- |
| Pure parser, normalization, queue, or state transition | TS/Vitest or Rust unit test | It crosses a module boundary |
| Vue props, emitted events, visible state, or user interaction | Vue component test | It changes a real workflow |
| Command, service, signaling, capture-source, permission, or device contract | Rust integration test | It depends on the actual browser/media path |
| WebRTC capture, encode, signaling, `PlayerView` attachment, or stream lifecycle | Targeted unit/integration test plus real E2E | The user-visible stream path changes |
| CSS-only cleanup | Static checks | Layout or playback behavior can regress |
| Bug fix | Regression test at the narrowest layer | The bug involves real media or browser behavior |

3. Test stable public contracts, not incidental DOM structure, CSS class names, pixel coordinates, or implementation-only helpers. Mock only external boundaries; never mock the logic under test or use a fake handshake as an integration test.
4. For a failure, classify it before editing: implementation, selector/fixture, timing, or environment/dependency. Record the command and first meaningful error.

## Repository Commands

Run focused checks first, then the relevant broader gates.

```bash
# Host Vue/TypeScript
npm run lint
npm run typecheck
npm test
npm run format:check

# Viewer (separate npm project)
cd viewer && npm run lint && npm run typecheck && npm run build

# Rust
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml --release
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --no-deps
cargo test --manifest-path src-tauri/Cargo.toml
```

`npm run check` covers root lint, typecheck, and Vitest. `npm run check:all` adds the root build and Rust checks. Root and Viewer builds are separate. The Husky pre-commit hook runs configured root and Viewer lint checks; it is not a replacement for full verification.

## Fragile Playback Rules

Before changing `viewer/src/views/PlayerView.vue`, preserve these invariants:

- `<video>` is rendered only while status is `streaming`.
- The `viewer-stream` handler stores the `MediaStream` in `pendingStream` before calling `markStreaming()`.
- The handler does not attach synchronously; the post-flush watcher attaches after Vue creates the `<video>` ref.
- The five-second no-frame watchdog remains enabled and is cleared by `loadedmetadata`.
- `MainView.vue` also gates `PlayerView` with `v-if="streaming"`; the two-level transition is intentional.

For playback changes, add a regression test for state/attachment behavior where practical, rebuild `viewer/dist`, then run the real media gate. A successful WebRTC connection or received track does not prove that frames render.

## Real Media Verification

Use these from the repository root for changes affecting capture, encoding, signaling, stream attachment, or user-visible playback:

```bash
cd viewer && npm run build
cd ..
node tools/verify-fix.js
node tools/diag-frames-direct.js
```

`verify-fix.js` must report `VERDICT: ✅ Frames rendered AND canvas has visible non-black pixels`. `diag-frames-direct.js` records `framesDecoded`, packet loss, jitter-buffer delay, and first decoded change. For the LAN display target, validate a moving host clock/timer and keep receiver buffering targets at zero (`jitterBufferTarget = 0`, `playoutDelayHint = 0` when supported).

If the Viewer build fails because an optional Vite/Rollup native dependency is missing, treat existing `viewer/dist` as stale and do not claim E2E verification. If no frames appear, check zombie binaries on port `3131`, stale `viewer/dist`, Chrome launch, signaling room ownership, received track, Host `MediaAdded`, and capture-loop logs before changing playback code.

## Completion Checklist

- Add or update the narrowest regression test for behavior changes.
- Run focused checks, then the relevant Host, Viewer, Rust, and real-media gates.
- Review failures for environment causes before changing production behavior.
- Use `git diff --check` and report any skipped gate or stale build explicitly.
