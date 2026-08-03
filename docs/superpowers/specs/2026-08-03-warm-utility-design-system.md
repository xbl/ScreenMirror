# Screenmirror Warm Utility Design System

## Goal

Make Screenmirror feel like a focused macOS menu-bar utility rather than a web dashboard. The host and browser Viewer share one visual language built from black, white, warm neutrals, and a restrained orange accent. The interface follows the operating system light or dark appearance automatically.

The redesign must preserve all existing capture, source-switching, connection, permission, and playback behavior.

## Direction

The selected direction is **Warm Utility**:

- Native macOS structure and density come first.
- Warm white and graphite surfaces replace the current cool blue-gray palette.
- Orange provides identity but remains semantically scarce.
- SF Pro and the system CJK font replace the serif display treatment.
- Controls use compact macOS proportions, visible hierarchy, and direct labels.
- Motion confirms state changes without delaying repeated actions.

## Product Surfaces

### Tray panel

The tray panel is the primary host surface. It is not a dashboard. It contains, in order:

1. Product identity and icon-only device/settings tools.
2. QR connection group with URL, connection status, and copy action.
3. Current capture source with preview and a clear Change action.
4. Host-side quality segmented control.
5. A quiet destructive Exit action separated from the workflow.

The panel keeps the current compact window dimensions and avoids nested cards. A bordered group may frame one coherent object such as the QR connection block or source row.

### Source picker

The standalone picker remains a two-step workflow:

- Step one: entire primary screen, window/App, and available extended displays.
- Step two: running windows and apps.

The window owns its chrome, supports dragging from the title area, resizes according to content, and closes immediately after selecting a screen. Cards are selection tiles rather than decorative containers. The selected source uses an orange border and subtle orange focus fill.

### Settings and connected devices

Settings use macOS-style grouped rows instead of a webpage drawer full of cards. Each row maps one label and optional explanation to one control or status. Connected devices use the same panel and row anatomy. Destructive reset actions use system red, not orange.

### Viewer

The Viewer gives the full viewport to the native video element. Native video controls remain enabled, including fullscreen. Connecting and disconnected states use the same warm tokens and compact controls as the host. No marketing header, privacy panel, feature description, or permanent toolbar is added.

## Design Tokens

Tokens live in each SPA's global style entry and have matching semantic names.

### Color

| Token role | Light | Dark | Use |
| --- | --- | --- | --- |
| Canvas | `#f3efe9` | `#171513` | Window background |
| Surface | `#faf8f4` | `#211e1b` | Primary panel |
| Group | `#fffdfa` | `#2a2622` | Grouped rows and coherent objects |
| Control | `#ebe5dc` | `#332e29` | Secondary controls |
| Selected | `#ffffff` | `#453d36` | Active segmented item |
| Text | `#211e1a` | `#f4eee7` | Primary text |
| Secondary text | `#786f65` | `#aaa097` | Supporting information |
| Accent | `#e96316` | `#ff7a25` | Main action, waiting, selection |
| Success | `#35a55b` | `#43c56c` | Connected/granted |
| Danger | `#c43f39` | `#ff716b` | Disconnect/reset/error |

Borders use low-alpha versions of the foreground color. Focus rings use the accent at accessible contrast. QR modules use near-black on near-white for reliable scanning regardless of theme.

### Typography

- Body: `-apple-system`, `BlinkMacSystemFont`, `SF Pro Text`, system sans-serif.
- Display roles use the same system stack with `SF Pro Display` where available.
- Chinese falls through to PingFang via the macOS system stack.
- Technical values use `SF Mono` or the system monospace stack.
- Letter spacing is zero. Hierarchy comes from size, weight, color, and spacing.

### Shape, spacing, and depth

- Base spacing unit: 4 px.
- Compact controls: 28-32 px high.
- Standard controls: 32-36 px high.
- Control radius: 6-8 px.
- Group radius: 10 px.
- Window radius is native or 12-14 px for custom chrome.
- Avoid pill buttons except true status chips.
- Use hairline borders and shallow shadows. Do not use gradients as page decoration.
- Translucency is reserved for overlays or Viewer controls over video.

### Motion

- Press feedback begins on pointer down.
- Repeated controls use no entrance animation.
- Hover/color transitions: 120-160 ms.
- Panels and dialogs: 180-220 ms ease-out.
- Animate only transform and opacity where possible.
- `prefers-reduced-motion: reduce` removes movement while retaining state color/opacity feedback.

## Component Rules

- Icon buttons use familiar symbols, 28-32 px square hit areas, and accessible labels.
- Primary buttons are orange with white text.
- Secondary buttons use the control surface and hairline border.
- Text actions use orange without a rounded container unless they need a larger hit target.
- Binary settings use a checkbox or switch. Multiple exclusive quality options use a segmented control.
- Status colors are semantic: orange waiting, green connected/granted, red failed/destructive.
- Source and device rows truncate long primary text and never create horizontal scrolling.
- Dialogs and drawers have one level of grouping; cards are not nested inside cards.
- Empty and loading states occupy stable dimensions so the layout does not shift.

## Architecture

Host tokens remain in `src/styles/tokens.css` and baseline rules in `src/styles/base.css`. Existing Vue components consume semantic variables and keep their current behavior and Tauri APIs. Shared visual primitives are expressed through small global utility classes only when at least two components need the exact same control anatomy; otherwise component-scoped styles remain local.

Viewer tokens move out of the monolithic `viewer/src/App.vue` style block into matching `viewer/src/styles/tokens.css` and `viewer/src/styles/base.css` files. The Viewer does not import Host source files because it is a separate npm project and build boundary.

No UI framework, animation library, or runtime theme dependency is added. CSS `prefers-color-scheme` provides automatic appearance switching.

## Behavior and Error Handling

- Existing Tauri commands and WebRTC state machines remain unchanged unless a UI control currently misrepresents their behavior.
- Source previews retain loading, unavailable, and selected-source synchronization semantics.
- Permission failures remain actionable and link to macOS settings.
- Viewer errors remain limited to connecting and disconnected/no-frame states with a reconnect action.
- Unsupported CSS appearance features degrade to opaque semantic surfaces.

## Accessibility

- Maintain visible keyboard focus using the accent ring.
- Preserve native `<button>`, `<select>`, and `<video controls>` semantics.
- Icon-only buttons require localized accessible labels.
- Text and essential controls meet WCAG AA contrast in both appearances.
- Do not communicate connection state by color alone; pair color with text.
- Respect reduced motion, reduced transparency, and increased contrast where supported.

## Verification

1. Run Host lint, typecheck, component tests, and build.
2. Run Viewer lint, typecheck, tests if present, and build.
3. Run Rust checks only if behavior or Tauri configuration changes.
4. Capture Host tray, source picker, settings, and Viewer screenshots in light and dark appearance.
5. Check desktop and narrow mobile Viewer viewports for clipping, overlap, and horizontal scrolling.
6. Exercise the real source-switch workflow and Viewer stream path after visual changes.

## Out of Scope

- Changing the application logo artwork.
- Changing WebRTC, capture, encoder, signaling, or source-selection behavior.
- Adding a third-party component or animation framework.
- Adding manual theme preferences; appearance follows macOS/browser settings.
- Reintroducing removed privacy notices, top bars, or generic error popups.
