# Warm Utility Design System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the cool web-dashboard styling in the Host and Viewer with the approved Warm Utility macOS design system while preserving all capture, source-selection, signaling, and playback behavior.

**Architecture:** Keep Vue component behavior and Tauri/WebRTC boundaries unchanged. Define semantic theme tokens once per npm project, then update existing component-scoped CSS to consume those tokens and use compact macOS group-row anatomy. Use `prefers-color-scheme` for light/dark appearance; do not add a runtime theme store or UI library.

**Tech Stack:** Vue 3 `<script setup>`, TypeScript, CSS custom properties, Tauri 2, native HTML video, Vitest, vue-tsc, Vite.

---

## File Map

- Modify `src/styles/tokens.css`: Host Warm Utility light/dark semantic tokens, type, spacing, shape, motion, focus.
- Modify `src/styles/base.css`: Host native typography/reset, controls, focus, scrollbars, reduced motion.
- Modify `src/components/TrayPanel.vue`: compact macOS panel structure and actions; preserve polling and commands.
- Modify `src/components/QRCard.vue`: QR connection group and orange status/action styling.
- Modify `src/components/SourcePicker.vue`: source tiles, source rows, adaptive surfaces; preserve the drag command and selection flow.
- Modify `src/components/SettingsOverlay.vue`: grouped settings rows and system red reset action.
- Modify `src/components/ConnectedDevicesListDrawer.vue`: grouped device rows and shared overlay anatomy.
- Modify `src/components/LanguageSelector.vue`: native-looking compact select using semantic tokens.
- Modify `src/components/ScreenRecordingPermissionModal.vue`: adaptive modal and permission states.
- Create `viewer/src/styles/tokens.css`: matching Viewer semantic tokens with `prefers-color-scheme`.
- Create `viewer/src/styles/base.css`: Viewer global reset, typography, focus, reduced-motion rules.
- Modify `viewer/src/App.vue`: import the Viewer style files and remove its duplicated cool palette.
- Modify `viewer/src/views/ConnectionPrompts.vue`: centered connection state using system typography and orange status.
- Modify `viewer/src/views/PlayerView.vue`: black content-first player, native fullscreen controls, compact overlay status, and adaptive connecting/disconnected states; preserve the load-bearing stream state machine exactly.
- Modify `viewer/src/components/PlayerControlPanel.vue`, `viewer/src/components/MyDeviceInfoCard.vue`, `viewer/src/components/PrivacyDialog.vue`: migrate remaining Viewer surfaces to semantic tokens; remove obsolete visual treatment without changing behavior.
- Modify `tests/vue/components.test.ts`: add stable semantic assertions for key Host states if existing test setup supports them.
- Create `viewer/tests/design-system.test.ts` only if a Viewer test runner already exists; otherwise use build/typecheck and Playwright screenshot checks because Viewer has no current test suite.
- Create `tools/design-system-check.mjs`: launch the dev server and use Playwright to capture Host/Viewer routes at desktop and narrow viewport sizes, asserting no horizontal overflow and visible primary surfaces.
- Modify `.gitignore`: keep `.superpowers/` brainstorming artifacts ignored.

## Task 1: Replace the global theme foundation

**Files:** `src/styles/tokens.css`, `src/styles/base.css`, `viewer/src/styles/tokens.css`, `viewer/src/styles/base.css`, `viewer/src/App.vue`

- [ ] **Step 1: Add the failing token contract check.** Add a small Node assertion in `tools/design-system-check.mjs` that reads both token files and requires the semantic names `--canvas`, `--surface`, `--group`, `--control`, `--text`, `--muted`, `--accent`, `--success`, `--danger`, `--focus-ring`, and `--motion-fast`.

```js
import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';

for (const file of ['src/styles/tokens.css', 'viewer/src/styles/tokens.css']) {
  const css = await readFile(file, 'utf8');
  for (const token of ['--canvas', '--surface', '--group', '--control', '--text', '--muted', '--accent', '--success', '--danger', '--focus-ring', '--motion-fast']) {
    assert(css.includes(token), `${file} is missing ${token}`);
  }
}
```

- [ ] **Step 2: Run the contract check and verify it fails** because the current files use `--bg`, cyan `--accent`, and do not define the new semantic roles.

Run: `node tools/design-system-check.mjs`

Expected: FAIL naming at least `src/styles/tokens.css is missing --canvas`.

- [ ] **Step 3: Replace Host tokens and baseline rules.** Use the spec values: light canvas `#f3efe9`, dark canvas `#171513`, light accent `#e96316`, dark accent `#ff7a25`; define all colors semantically and add `@media (prefers-color-scheme: dark)` overrides. Keep compatibility aliases (`--bg: var(--canvas)`, `--surface-2: var(--group)`, `--surface-3: var(--control)`, `--accent-strong`, `--accent-dim`, `--accent-line`, `--warn`) during component migration so intermediate commits remain buildable.

Use system typography:

```css
--font-body: -apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC", system-ui, sans-serif;
--font-display: -apple-system, BlinkMacSystemFont, "SF Pro Display", "PingFang SC", system-ui, sans-serif;
--font-mono: "SF Mono", ui-monospace, Menlo, monospace;
```

Set `--radius-sm: 6px`, `--radius-md: 8px`, `--radius-lg: 10px`, `--motion-fast: 140ms`, `--motion: 180ms`, and a two-layer accent focus ring.

- [ ] **Step 4: Add the matching Viewer token and base files.** Remove the duplicated `:root` token block from `viewer/src/App.vue`; import `./styles/tokens.css` and `./styles/base.css` in the global style block. Viewer tokens must expose the same semantic names but use `--canvas: #0e0d0c` for the player backdrop and the same Warm Utility text/accent roles.

- [ ] **Step 5: Run the contract and build checks.**

Run: `node tools/design-system-check.mjs`

Expected: PASS for both token files. Then run `npm run lint && npm run typecheck` and `cd viewer && npm run lint && npm run typecheck`.

- [ ] **Step 6: Commit the foundation.**

```bash
git add src/styles/tokens.css src/styles/base.css viewer/src/styles/tokens.css viewer/src/styles/base.css viewer/src/App.vue tools/design-system-check.mjs
git commit -m "feat: add warm utility theme tokens"
```

## Task 2: Restyle Host surfaces without changing behavior

**Files:** `src/components/TrayPanel.vue`, `src/components/QRCard.vue`, `src/components/SourcePicker.vue`, `src/components/SettingsOverlay.vue`, `src/components/ConnectedDevicesListDrawer.vue`, `src/components/LanguageSelector.vue`, `src/components/ScreenRecordingPermissionModal.vue`

- [ ] **Step 1: Preserve behavior with the existing tests.** Run the current focused tests before CSS edits.

Run: `npx vitest run tests/SourcePicker.spec.ts tests/vue/components.test.ts`

Expected: existing tests pass.

- [ ] **Step 2: Migrate shared control selectors.** In each component, replace direct cool colors and serif display usage with semantic variables. Use `var(--accent)` only for primary actions, current source selection, and waiting status. Use `var(--success)` for granted/connected state and `var(--danger)` for exit/reset/error. Replace pill-shaped `.btn` rules with 6-8px radius except status chips.

- [ ] **Step 3: Refine `TrayPanel.vue`.** Keep the existing template and API calls. Change only layout/style: logo plus title at the top, icon-only device/settings buttons, QRCard as one connection group, SourcePicker as one source group, and Exit as a separated red text/action row. Remove the oversized card impression by using consistent 10px grouped surfaces and 14-18px panel padding.

- [ ] **Step 4: Refine `QRCard.vue` and `SourcePicker.vue`.** Keep all preview loading/reconciliation logic untouched. Make QR high-contrast in both modes, use a compact connection status row, make `.sp-current` a grouped source row, and make standalone source choices use orange selected borders. Remove hard-coded cyan/blue-gray preview art colors by using semantic fallback surfaces.

- [ ] **Step 5: Refine settings, devices, language, and permission surfaces.** Keep their emits, API calls, and i18n keys unchanged. Use grouped rows with a single label/value/action alignment; use a right-side sheet only for the existing overlay behavior; use `var(--danger)` for reset and `var(--success)` for granted permission.

- [ ] **Step 6: Add a stable UI regression assertion.** In `tests/vue/components.test.ts`, assert that mounted Host surfaces expose semantic classes or accessible labels for the connection group, source group, settings rows, and permission action. Do not assert pixel coordinates or exact CSS class lists.

- [ ] **Step 7: Run Host verification.**

Run: `npm run lint && npm run typecheck && npm test && npm run format:check`

Expected: all existing and new Host tests pass; formatting has no changes.

- [ ] **Step 8: Commit the Host surface migration.**

```bash
git add src/components/TrayPanel.vue src/components/QRCard.vue src/components/SourcePicker.vue src/components/SettingsOverlay.vue src/components/ConnectedDevicesListDrawer.vue src/components/LanguageSelector.vue src/components/ScreenRecordingPermissionModal.vue tests/vue/components.test.ts
git commit -m "feat: apply warm utility styling to host surfaces"
```

## Task 3: Restyle Viewer as a native player surface

**Files:** `viewer/src/views/ConnectionPrompts.vue`, `viewer/src/views/PlayerView.vue`, `viewer/src/components/PlayerControlPanel.vue`, `viewer/src/components/MyDeviceInfoCard.vue`, `viewer/src/components/PrivacyDialog.vue`

- [ ] **Step 1: Preserve the playback invariants before editing.** Run `cd viewer && npm run lint && npm run typecheck && npm run build`. Do not change `pendingStream` ordering, the `flush: 'post'` watcher, the two-level `v-if`, or the no-frame watchdog.

- [ ] **Step 2: Replace connection prompt styling.** Use a small orange Screenmirror mark, system typography, a single status line, and one reconnect action. Remove serif display styling and cyan tokens. Keep the existing i18n text and emitted `reinitiate` event.

- [ ] **Step 3: Replace player styling only.** Keep `<video controls playsinline muted>` and event handlers unchanged. Use black/near-black canvas, `object-fit: contain`, and a compact bottom overlay only for state text and the fullscreen/native controls context. Do not add a custom fake video control bar that can conflict with native fullscreen. Disconnected and connecting states remain centered and use semantic orange/red status with the existing reconnect button.

- [ ] **Step 4: Migrate remaining Viewer components.** Use semantic tokens for the device card and any remaining dialog; remove obsolete blue/green hard-coded colors and any persistent privacy UI if still rendered by the current flow. Do not remove required connection or playback behavior.

- [ ] **Step 5: Run Viewer verification.**

Run: `cd viewer && npm run lint && npm run typecheck && npm run build`

Expected: exit 0 and a fresh `viewer/dist` build.

- [ ] **Step 6: Commit the Viewer migration.**

```bash
git add viewer/src/styles viewer/src/App.vue viewer/src/views/ConnectionPrompts.vue viewer/src/views/PlayerView.vue viewer/src/components/PlayerControlPanel.vue viewer/src/components/MyDeviceInfoCard.vue viewer/src/components/PrivacyDialog.vue
git commit -m "feat: apply warm utility styling to viewer"
```

## Task 4: Visual and responsive verification

**Files:** `tools/design-system-check.mjs`, `tools/output/` (ignored)

- [ ] **Step 1: Add Playwright checks.** The script must start Vite for Host and Viewer when needed, use a fresh browser context, capture 1440x900 and 390x844 screenshots, and assert `document.documentElement.scrollWidth <= window.innerWidth` for every route. It must test `prefers-color-scheme: light` and `dark` for Viewer and capture the Host tray/source-picker DOM when the Tauri shell route is unavailable.

- [ ] **Step 2: Run the visual check.**

Run: `node tools/design-system-check.mjs`

Expected: screenshots in `tools/output/design-system/`, no horizontal overflow, no console errors from the design surfaces, and visible primary content in both modes.

- [ ] **Step 3: Run the real media gates.**

Run from the repository root after the fresh Viewer build:

```bash
node tools/verify-fix.js
node tools/diag-frames-direct.js
```

Expected: the frames verdict remains successful and WebRTC stats remain available. CSS changes must not alter playback attachment or capture behavior.

## Task 5: Final review and cleanup

- [ ] **Step 1: Search for old visual debt.** Run `rg -n "Fraunces|Newsreader|#7be0d2|#a3ecdf|#137cbd|#00f992|linear-gradient|border-radius: 999px" src viewer/src` and remove only obsolete UI styling left by the migration. Keep gradients or pills only where they are semantically required by QR/indicator controls.

- [ ] **Step 2: Run the complete checks.**

```bash
npm run check
npm run format:check
cd viewer && npm run check:all
cd .. && cargo check --manifest-path src-tauri/Cargo.toml
git diff --check
```

- [ ] **Step 3: Review the diff against the design spec.** Confirm no Tauri command, WebRTC state machine, source synchronization, i18n contract, or native video attachment logic changed.

- [ ] **Step 4: Commit final cleanup.**

```bash
git add src viewer tools/design-system-check.mjs
git commit -m "chore: finish warm utility design system cleanup"
```
