# CorkyTux: Settings > Emulators visual system

## Document status

- Status: **row visual update implemented**: the name no longer carries status suffixes; `Linked` keeps a compact pill and a red `Unlink` button; `System` keeps the badge next to the name, shows a static red `Found in system` notice and keeps the `Set as linked` button as the only clickable action. The `Working…` flow, the stable slot and the action mapping stay intact except for the addition of `Unlink`, which I explicitly asked for so the link is not irreversible.
- Scope: hierarchy, spacing and visual treatment of the emulator rows, focused on identity, state and the available actions.
- Verified sources: `src/ui/settings.rs`, `rebuild_emu_rows` and the `EmuOp` handler; `src/backend/theme.rs`, theme tokens and accent palette; `src/ui/helpers.rs`, app CSS; `assets/icons/ATTRIBUTION.md` for the asset licenses.
- Changes in this version: canonical name without parentheses; `.emu-status-pill` for `Linked`/`System`/native; static `.emu-system-notice` with no hover, cursor or callback; `Set as linked` button with `.emu-link-action`; the `link_emulator_in` callback is unchanged. The A/B/C option sections are kept as historical traceability, but the current implementation is badge + static notice + button.
- Section "List of installable emulators (catalog)": **option 1+3 fixed as the only live one** (state-aware button + compaction of resolved rows), with a state → badge/label/class/width matrix. The row follows the content-list card style (Prism/Modrinth): class `.emu-row-card` and a 48 px square icon slot `.emu-row-icon` (11 Papirus SVGs; the official Azahar logo stays attributed but the resolver keeps the monogram).
- State logic, the state→action matrix and the handlers are not touched in this update.

## Fixed decisions and defaults

**Fixed decisions**

- The hierarchy separates availability, identity, state, operational feedback and action.
- The badge communicates state; the red notice communicates a finding; the button communicates the action. `Linked` keeps the small pill; `System` keeps its badge and uses the static `Found in system` notice, separate from `Set as linked`.
- I create no colors outside the existing tokens; derived ones (like `{accent_ui}`) are computed during CSS generation, not hardcoded.
- `Working…` does not replace the signal that describes the `source`: neither the state pill nor the `System` notice disappear during the operation.
- The action keeps `gtk::Button` semantics, keyboard and focus; the notice is a `gtk::Label` with no pointer signal.
- The status label is a stable slot and is always present in the row (see the "Stable slot" section).
- During the operation the button becomes insensitive; a double click does not trigger a second operation (see the "`Working…` flow and double click" section).
- The action border/focus uses `{accent_ui}`: in light `color-mix(in srgb, {text_main} 30%, {accent})`; in dark `{accent}` (see the "3:1 contrast" section).

**Proposed defaults, pending approval**

- Adwaita Sans as the nominal face inherited from libadwaita.
- Vertical centering over a 36 px visual rhythm.
- No ornamental animation or badge cross-fade.
- The current option is the final composition: `System` badge + static red notice + `.emu-link-action` button; options A/B/C remain as traceability.

## Objective

Make every row read left to right as an unambiguous operational sentence:

1. availability;
2. emulator identity;
3. current state of its source;
4. available action.

The central rule is that the state signal answers `where does it come from?` and the button answers `what can I do now?`. `Linked` uses a small pill; `System` keeps its badge, shows the static `Found in system` notice and ends with the `Set as linked` button.

## Design thesis

A row must feel like a compact, reliable control: clean surface, native typography, 4/8 px rhythm and a single semantic accent per function. State keeps a small transparent capsule; the action keeps a 36 px touch target, but changes surface, radius, weight or border to weigh more than state without competing with a primary action like `Install`.

## Typographic base

- Family: keep the native GTK/libadwaita typography, with **Adwaita Sans** as the nominal face and the system fallback. This local decision must not introduce a new family through CSS.
- Row identity: `.details-title`, 16 px, bold, `text_main`.
- Action: 14 px, bold.
- Process state `Working…`: `.time-label`, 12 px, `text_sec`.
- `Linked`/native badge: `.proton-path-badge` + `.emu-status-pill`, 11 px, bold, `min-height: 20px` and reduced horizontal padding.
- ~~Availability dot: 14 px, `emu-dot-on` or `emu-dot-off`.~~ **Obsolete**: I removed the `[•]` dot from the row (see the matrix and the open items in the catalog section); the classes are now unused, candidates for removal.
- Do not use a second family, decorative capitals or italic text.

## Existing tokens

### Surfaces and text

| Token | Light theme | Dark theme | Use in the row |
| --- | --- | --- | --- |
| `bg` | `#F4F1F8` | `#000000` | General app background. |
| `panel` | `#FFFFFF` | `#121212` | Settings panel and surface of the card that holds the list. |
| `card` | `#FFFFFF` | `#181818` | Card surface. |
| `well` | `#ECE8F2` | `#181818` | Secondary control background. |
| `hover` | `#E3DCEE` | `#282828` | Row or button hover. |
| `border` | `#D8D0E3` | `#282828` | Neutral borders. |
| `text_main` | `#241F2E` | `#E0E0E0` | Name and action labels. |
| `text_sec` | `#5B5468` | `#AAAAAA` | `Working…` and secondary metadata. |
| `text_muted` | `#8A8296` | `#777777` | Unavailable point. |

The destructive pattern keeps `#E5484D`; `Remove` uses it as a destructive action and the static `System` notice uses it as a warning, without being a badge or a clickable control.

### Configurable accent

| ID | Accent | Base value |
| --- | --- | --- |
| 0 | green | `#1db954` |
| 1 | blue | `#1E88E5` |
| 2 | cyan | `#00BCD4` |
| 3 | purple | `#AB47BC` |
| 4 | pink | `#EC407A` |
| 5 | red | `#EF5350` |
| 6 | orange | `#FFA726` |
| 7 | yellow | `#FFEE58` |
| 8 | teal | `#26A69A` |
| 9 | indigo | `#5C6BC0` |

- I currently have `Accent=9`, indigo `#5C6BC0`.
- In the dark theme, `{accent}` uses the base value.
- In the light theme, `{accent}` uses `darken(hex, 0.22)`.
- `on_accent` is `#FFFFFF` in dark and `text_main` (`#241F2E`) in light.
- The following specs must keep using `{accent}`; neither the indigo nor the darkening result should be hardcoded. The derived `{accent_ui}` border follows the same rule: it is computed, not hardcoded.

### Component vocabulary already in use

| Component or pattern | Established use | Decision for this row |
| --- | --- | --- |
| `.add-btn` | 14 px bold, 36 px tall, 20 px radius, accent background and `on_accent`. | Reserved for `Install`, the only primary action. |
| `.settings-btn` | 14 px bold, `well`, 1.5 px accent border and 20 px radius. | Direct candidate for option A. |
| `.danger-btn` | Translucent red fill, `#E5484D` border, text in the same color and 36 px. | Reserved for `Remove`. |
| `.proton-path-badge` + `.emu-status-pill` | 11 px accent text, 1 px border, 10 px radius, `min-height: 20px` and reduced padding. | Compact pill for `Linked`/native. |
| `.emu-link-action` | 14 px touch action, 36 px, 20 px radius and translucent accent fill. | Used on `Set as linked`; does not alter `Remove` or `Install`. |
| `.emu-system-notice` | Static `gtk::Label` of 14 px, 36 px, 8 px radius, soft red background/border/text (`#E5484D`). | Communicates `Found in system`; it has no `:hover`, `:disabled`, cursor or `connect_clicked`. |
| `.action-btn` | 11 px bold, 18 px radius and `action_bg` (`#242424` in dark, `well` in light). | Do not use for this action: its scale fits a compact bar better. |
| `.filter-btn` | `well`, with checked in `hover`, accent text and accent border. | Do not use: checked communicates filter selection, not a link action. |
| `.icon-ghost` and `.source-link` | Transparent or textual indicators. | They are visual references for option C, not classes to reuse as-is. |
| `.destructive-action` | 36 px geometry, 14 px bold, 16 px horizontal padding and 20 px radius. | It contributes rhythm, not destructive color. |
| `.dark-btn` | Black button with white border. | Excluded: it would carry too much weight for an optional action. |
| `.section-head` | Shared section header (Epic/GOG Stores): 18 px bold, `text_main`. | Used in the 9 section headers; `.frame-title` (12 px) stays for accounts and other frames. |
| `.import-opt-title` | Import Manager option title: 14 px bold, `text_main` at rest, on `:checked` and `:hover`; `text_muted` on `:disabled`. It pins the color that Adwaita lightens on selection in the light theme. |
| `.import-dot-green` / `.import-dot-red` | Import Manager option indicator: 10 px circle, green `#00E639` border (trial) or red `#E5484D` (permanent); fill only on `:checked`. | No new colors (tokens already used in FREE and danger badges); native `ToggleButton` keyboard focus. |
| `popover` / `popover.menu` + `> contents` + `> arrow` + `modelbutton` | Popovers and menus without a double box: transparent exterior with no border/shadow/padding; background, `{border}` border, 12 px radius and subtle shadow only in `contents`; `arrow` same as contents; `modelbutton` with hover/focus-visible/disabled and token-based separators (same color-mix as the global `separator`). | Single global rule in `helpers.rs`; covers Popover, PopoverMenu/MenuButton, DropDown, GTK context menus and tooltips without duplicating per screen. |
| `.lib-card` / `.lib-cover` | Library card (Epic+GOG Stores): `{card}` background, `{border}` border, 14 px radius, rest shadow `0 2px 10px`; cover with a fixed-px `gtk::Image` over a `{well}` background with a minimum height (natural size clamped: it does not break the grid). Hover and border highlight come from the existing `.mc-tile:hover` / `.gog-tile:hover`. Normal/compact sizes by 900 px breakpoint (Epic 190/150, GOG 170/140). | No hover duplication; GOG uses the same badge+state hierarchy as Epic; typography via `details-title`/`time-label`. |
| `.mc-account > button`, `.import-mode:checked`, selected/hover rows | They already combine `well`, `hover` and `color-mix` with accent to tell surfaces and states apart. | They confirm that a faint fill is a product-native pattern, not a local invention. |

## Verified real structure

`rebuild_emu_rows` creates one horizontal `gtk::Box` per emulator, with these properties:

- horizontal spacing: 8 px;
- top and bottom margin of each row: 2 px;
- vertical list: 4 px spacing, so the effective visual spacing between neighbouring rows is 8 px;
- top margin of the list: 16 px;
- current child order: icon, text block (name + optional badge, description), operational slot, optional notice and optional action;
- the flexible label uses `.details-title`, start alignment and end ellipsizing; the name no longer concatenates `(system)`, `(linked)`, `(native)`, `(appimage)` or `(installed)` suffixes;
- the operational state uses `.time-label` and reserves 12 characters; with the stable slot this label always exists, empty at rest (see the "Stable slot" section);
- `Linked`/`System`/native uses `.proton-path-badge` plus `.emu-status-pill`; the `System` notice is a separate static label and the button is still a `gtk::Button`;
- `Found in system` reserves 140 px; `Set as linked` reserves 120 px; the other actions reserve 80 px.

The name is the canonical identity and is shown alone, with `.details-title`; the description takes the second line with `.time-label`. The state signal is rendered separately: `Linked` and `System` as a compact pill, `System` additionally as a non-interactive notice and `Set as linked` as the action.

## Row semantic matrix

| Source | Availability signal | Badge/label | Action |
| --- | --- | --- | --- |
| `linked` | lit dot | `Linked` (`.proton-path-badge` + `.emu-status-pill`) | `Unlink` (`.emu-unlink-action`); unlinks without touching the binary |
| `system` | lit dot | `System` (`.proton-path-badge` + `.emu-status-pill`) + static `Found in system` notice (`.emu-system-notice`, 140 px) | `Set as linked` (`.emu-link-action`, 120 px) |
| `appimage` | depending on reported availability | — | `Remove` |
| `none` or empty, non-native | unlit dot if not installed | — | `Install` or `Remove` depending on `installed` |
| native, empty source | lit dot | `Linked` (`.proton-path-badge` + `.emu-status-pill`) | `Unlink` (`.emu-unlink-action`) |

`Working…` describes an operation in progress, not the `source`. That is why it stays in its stable slot; the `System` action stays visible and is disabled during the operation.

## Row system

### Visual hierarchy

Recommended reading order:

```text
[48px icon] [flexible identity + badge] [description] [Working…] [static notice] [action]
```

- **Dot:** removed from this row; the actionable signal or the absence of a button communicates the state.
- **Identity:** flexible main block. It is what gets read first and what should be clipped last.
- **Working…:** transient feedback in `text_sec`; it does not use a second badge.
- **Linked:** state vocabulary in a small transparent pill, with accent text, 1 px border, 10 px radius, 11 px bold and 20 px minimum height.
- **System:** keeps the state pill next to the name; the `Found in system` notice is static and has no affordance; `Set as linked` is the only clickable action.
- **Action:** last element of the row, with an explicit imperative label, 36 px touch target and 14 px bold.

### Spacing and alignment

- Keep 8 px between the row children; the `gtk::Box` handles the rhythm.
- Do not add individual margins around the pill, the notice or the button.
- Vertically center the children over a 36 px minimum visual height; the icon slot and the `GtkImage` use `Center` on both axes.
- Keep the identity label with horizontal expansion and end ellipsizing.
- Reserve 140 px for the `Found in system` notice and 120 px for `Set as linked`; the text of both pieces must not be clipped. The other actions keep 80 px.
- Keep `Working…` in a stable 12-character slot to avoid layout jumps during the operation.

### Rule against two identical accent capsules

In a single row I do not allow two chips with the same visual language:

1. `Linked`/`System`/native uses a single state pill next to the name: transparent background, 11 px, 10 px radius, 1 px accent border and 20 px minimum height.
2. A capsule-shaped action must use 14 px bold, 20 px radius and stand out through its surface or weight.
3. `Install` keeps `.add-btn` and is the only primary action with a solid accent background.
4. `Remove` keeps `.danger-btn`: red outline with a translucent fill.
5. `System` adds a static `.emu-system-notice` notice; it is not a badge, button, link or hoverable element.
6. `Set as linked` stays a native `gtk::Button` to preserve keyboard, focus and semantics; the existing handler is not duplicated.
7. The accent border of the actions is not mixed with the warning red: the notice uses `#E5484D` and the button keeps `.emu-link-action`.

### Transient state and no animation

- Do not animate the badge change or the row height. Hover/focus changes and the appearance of `Working…` are immediate and utilitarian.
- `Working…` does not turn into a spinner nor duplicate the badge message.

### `Working…` flow and double click (functional spec)

Current behavior verified in the code:

- The button click inserts `"Working…"` into the `emu_status` map, updates the row's stable label and spawns a thread that sends `EmuOp` when it finishes.
- The `Found in system` notice and the `System` badge stay visible; the notice has no callback.
- The `EmuOp` handler (success or error) clears the `emu_status` map, writes feedback in the global status label and rebuilds the list.

Current mechanism:

1. **Rest:** `System` shows the `System` badge, the static `Found in system` notice, an empty stable slot and an enabled `Set as linked` button.
2. **Click (synchronous, before the thread):**
   a. Reentry guard: `if !b.is_sensitive() { return; }`.
   b. `b.set_sensitive(false)`.
   c. `set_text("Working…")` on the row label (direct mutation of the widget captured in the closure).
   d. Keep the insert in `emu_status` for continuity if a rebuild happens mid-operation.
   e. Thread + `EmuOp` send unchanged from the current code.
3. **Double click before the response:** the second click triggers nothing. GTK4 does not dispatch pointer events to insensitive widgets, and `set_sensitive(false)` is applied inside the first handler, before the second press is evaluated. The guard from point 2 also covers a programmatic `activate()`.
4. **Rebuild mid-operation:** if a list refresh (`settings.rs:1649`) recreates the row while the operation runs, the new button is born disabled: when building a `system` row, if `emu_status` holds `"Working…"` for that emulator, `btn.set_sensitive(false)`. The lock is therefore rebuild-proof and does not depend on the lifetime of the clicked widget.
5. **Re-activation (how and when):** the disabled button is not re-enabled in the same widget; the widget is discarded and the recovery is decided by the result through the `EmuOp` rebuild:
   - On success, `emu_status` is cleared and the new list turns the row into `Linked` (badge + red `Unlink` button, which returns the row to `System` if the binary is still on the PATH).
   - On error, `emu_status` is cleared, the message goes to the global status and the row goes back to `System` with badge, notice and a new enabled `Set as linked` button.
   There is no path where an old button gets re-enabled.
6. **During the operation the `System` action stays visible and gets disabled;** the feedback does not replace it.
7. **Feedback:** `Working…` is visible from the click until the result rebuild. On success, the global status label already shows the result message.

### Stable slot (decision)

**Decision: always reserve the width** — the status label exists in every row, empty by default; layout shift is not accepted.

- The label is always created with `set_width_chars(12)` and empty text at rest; the click only does `set_text("Working…")` and the result rebuilds the row with the empty slot.
- **Mechanism:** the label keeps the same `set_width_chars(12)` and the rebuild handles the empty state after the result.
- **Why:**
  1. The stable slot is already promised in this document and is the basis of the direct mutation in the flow above.
  2. It keeps the badge and button column aligned across rows; the list is scanned by columns.
  3. It prevents the action target from moving while I wait for the result.
- **Accepted cost:** an empty gap of ~12 characters at rest per row; bounded because `Working…` occupies 9 characters at 12 px, so the gap does not exceed the already expected size.
- **Rejected alternative** (accepting the shift): it breaks the alignment between rows, contradicts the document's promise and moves the target during the operation.

### 3:1 border and focus contrast across the 10 accents

Problem (major review finding): the border/focus in `{accent}` does not guarantee 3:1 (WCAG 1.4.11, non-text/components) with every accent in the light theme. Specifically, yellow already darkened by 22% (`#c6b944`) gives **2.01:1** against the `#FFFFFF` panel; without the prior darkening it would be even lighter (≈1.2:1).

Resolution:

- I define a derived **`{accent_ui}`** token for the action border and focus, computed during CSS generation (styles are built at runtime in `helpers.rs` with the current accent; no hex is hardcoded):
  - Light theme: `color-mix(in srgb, {text_main} 30%, {accent})` — where `{accent}` is already `darken(hex, 0.22)`.
  - Dark theme: `{accent}` with no mix: the ten base accents already meet ≥3.85:1 against the `#121212` panel.
- The 30% (instead of the mathematical 25% minimum) gives margin for the `#F4F1F8` window background surface and for render anti-aliasing.
- `color-mix` is already used in the app (`.danger-btn`, `.import-mode:checked`), so the GTK version in use supports it; the rule is universal and covers the 10 accents without a runtime table.

Verification in the light theme (accent darkened 22%, mixed 30% with `text_main`):

| Accent | Base value | light `{accent}` | `{accent_ui}` border | vs `#FFFFFF` | vs `#F4F1F8` |
| --- | --- | --- | --- | --- | --- |
| green | `#1db954` | `#169041` | `#1a6e3b` | 6.29 | 5.63 |
| blue | `#1E88E5` | `#176ab2` | `#1b538a` | 7.92 | 7.09 |
| cyan | `#00BCD4` | `#0092a5` | `#0b6f81` | 5.83 | 5.21 |
| purple | `#AB47BC` | `#853792` | `#683074` | 9.28 | 8.30 |
| pink | `#EC407A` | `#b8315f` | `#8c2c50` | 8.10 | 7.25 |
| red | `#EF5350` | `#ba403e` | `#8d3639` | 7.72 | 6.91 |
| orange | `#FFA726` | `#c6821d` | `#956422` | 5.09 | 4.55 |
| yellow | `#FFEE58` | `#c6b944` | `#958b3d` | 3.48 | 3.11 |
| teal | `#26A69A` | `#1d8178` | `#1f6462` | 6.87 | 6.14 |
| indigo | `#5C6BC0` | `#475395` | `#3c4376` | 9.31 | 8.32 |

All ≥3.1:1. In dark the minimum is indigo/purple at 3.85:1 against `#121212`.

Application to the `Set as linked` button: the 1.5 px border uses `{accent_ui}`; the 14%/22% fill keeps `{accent}` (it is decorative behind `text_main` text and does not constitute the control outline); GTK's native focus indicator is kept. The red `System` notice does not use this token because it is not interactive.

## Options for `Set as linked`

> The following A/B/C options are historical exploration from before the visual update. The current decision lives in "Document status" and in the matrix of the flow section: `Linked` keeps the compact pill; `System` keeps badge + static `Found in system` notice and the `Set as linked` button.

### Option A: Anchored capsule

**Reuse class:** `.settings-btn`.

**Visual description:** a neutral pill with a `well` surface, 1.5 px accent border, `text_main` text, 20 px radius and 14 px bold. The `System` badge stays transparent; the action gains weight from its solid background, its more rounded radius, its thicker border and its larger text.

**Existing CSS rule:**

```css
.settings-btn {
  font-size: 14px;
  min-height: 36px;
  padding: 0 16px;
  border-radius: 20px;
  background-color: {well};
  color: {text_main};
  border: 1.5px solid {accent};
  font-weight: bold;
}

.settings-btn:hover {
  background-color: {hover};
}
```

**Light:** `#ECE8F2` background over the `#FFFFFF` panel, `#241F2E` text and a darkened accent border. The surface difference is clear even when the accent has low luminance.

**Dark:** `#181818` background over the `#121212` panel, `#E0E0E0` text and the current accent border. With the current accent, the border uses `#5C6BC0`.

**During `Working…`:** the capsule stays in place, becomes insensitive and the feedback appears separately in `text_sec`. The badge does not turn into a spinner nor is the message duplicated.

**Pros:**

- maximum consistency with components already in use;
- native touch target and focus without creating a new style family;
- `text_main` offers better contrast than relying on the accent color for the label;
- it is easy to recognize as a button.

**Cons:**

- there are still two adjacent rounded silhouettes; they are told apart by fill, not by removing the second capsule;
- the accent border can make the action look similar to a filter or a settings control;
- reusing a class named `settings-btn` documents style, not row-specific intent.

### Option B: Active capsule

**Proposed new class:** `.emu-link-action`.

**Visual description:** a touch action with a translucent accent fill, 1.5 px accent border (derived `{accent_ui}` token, see the "3:1 contrast" section) and `text_main` text. The badge stays empty/transparent; the action is clearly filled. The text does not use accent, to keep it legible with every accent, especially the yellow and light ones.

**Proposed CSS rule:**

```css
.emu-link-action {
  font-size: 14px;
  min-height: 36px;
  padding: 0 16px;
  border-radius: 20px;
  background-color: color-mix(in srgb, {accent} 14%, transparent);
  border: 1.5px solid {accent_ui};
  color: {text_main};
  font-weight: bold;
}

.emu-link-action:hover {
  background-color: color-mix(in srgb, {accent} 22%, transparent);
}

.emu-link-action:disabled {
  background-color: color-mix(in srgb, {accent} 8%, transparent);
  border-color: {text_muted};
  color: {text_muted};
}
```

`{accent_ui}` is derived during CSS generation: `color-mix(in srgb, {text_main} 30%, {accent})` in light, `{accent}` in dark (see the "3:1 contrast" section). GTK's native focus indicator must not be removed.

**Light:** the faint fill composes over `#FFFFFF`; the accent is automatically darkened 22%; the border uses `{accent_ui}` (30% mix with `text_main`) and the text stays `#241F2E`. The button looks like a surface washed by the accent, not like another empty outline.

**Dark:** the faint fill composes over `#121212`; the text is `#E0E0E0` and the border uses the accent without darkening (all accents meet ≥3.85:1). For the current indigo, the border is `#5C6BC0` and the fill stays within the same tonal family.

**During `Working…`:** the button becomes insensitive and its `:disabled` state communicates it; the transient label precedes the `System` badge, so it reads `identity -> Working… -> System -> action`. The full mechanism (double-click guard, rebuild-proof lock and result-driven re-activation) is specified in the "`Working…` flow and double click" section.

**Pros:**

- better balance between discoverability and hierarchy: it looks actionable without seeming as primary as `Install`;
- it does not duplicate the badge because one capsule is empty and the other is filled;
- it keeps the action text in `text_main`, so legibility does not depend on the accent luminance;
- it reuses the `color-mix` language already accepted in `.import-mode:checked` and `.danger-btn`;
- the `{accent_ui}` border guarantees 3:1 with the 10 accents in both themes.

**Cons:**

- the 14% fill can be subtle for light accents, especially in the light theme (mitigated by the well-contrasted `{accent_ui}` border);
- it requires a localized class to avoid changing the meaning of `.settings-btn` across the whole app;
- the double-click and re-activation behavior depends on the functional spec, not on CSS alone.

### Option C: Accessible ghost link

**Proposed new class:** `.emu-link-ghost`.

**Visual description:** the row ends with 14 px bold text, with no permanent capsule. The label uses `text_sec` to keep contrast; on hover it switches to `text_main` and gets underlined, while focus can be outlined with `{accent}`. Visually it is `badge + link`, with no two capsules. It is still a `gtk::Button`, not a clickable label.

**Proposed CSS rule:**

```css
.emu-link-ghost {
  background-color: transparent;
  border: none;
  color: {text_sec};
  font-size: 14px;
  font-weight: bold;
  min-height: 36px;
  padding: 0 8px;
}

.emu-link-ghost:hover {
  color: {text_main};
  text-decoration-line: underline;
}

.emu-link-ghost:focus {
  outline: 1.5px solid {accent};
  outline-offset: 2px;
}
```

**Light:** `#5B5468` text over `#FFFFFF`; hover `#241F2E` and an outline in the darkened accent. Permanent accent text is not forced because yellow and other light accents do not offer a consistent contrast guarantee.

**Dark:** `#AAAAAA` text over `#121212`; hover `#E0E0E0` and an outline in the current accent. For the current indigo, focus uses `#5C6BC0`.

**During `Working…`:** the link stays in place and becomes insensitive. The 36 px box keeps the touch target even without a permanent background.

**Pros:**

- it is the cleanest solution and the one that fully removes the competition between two capsules;
- it reduces visual noise in a list that can hold many rows;
- it keeps focus visible and a 36 px target;
- the action text keeps contrast with `text_sec`/`text_main` in both themes.

**Cons:**

- it has less visual weight and can read as a static link if the imperative text is not recognized;
- a hover needed to discover the link affordance can be insufficient on a touchpad or with the keyboard;
- the lack of background makes `Set as linked` blend more with `Remove` if the other actions do not follow a strict hierarchy;
- reusing `.source-link` directly is not advisable: it is designed as a 12 px label. `.icon-ghost` alone does not work either because it removes the touch height.

## Comparison

| Criterion | A: Anchored capsule | B: Active capsule | C: Ghost link |
| --- | --- | --- | --- |
| Clarity as a button | high | high | medium |
| Separation from the badge | medium-high | high | very high |
| Hierarchy vs `Install` | correct | correct | correct, more discreet |
| Label contrast | high with `text_main` | high with `text_main` | high with `text_sec`/`text_main` |
| Consistency with tokens | maximum | high | high |
| Visual noise | medium | low-medium | minimal |
| Main risk | two capsules still visible | subtle fill with light accents | low discoverability |

## Decision

**Current composition: `System` badge + static `Found in system` notice + button B (`.emu-link-action`) with `Set as linked` text.**

It is the best balance between state, warning and action:

1. **Semantics:** the `System` badge communicates state; the red label communicates the finding; `Set as linked` communicates the action and keeps its callback.
2. **No duplication:** the notice is not a second button and has no affordance; the button keeps native text, focus and semantics.
3. **Hierarchy:** `Install` keeps the only solid accent; `Set as linked` is secondary but discoverable; `Remove` keeps the destructive language.
4. **Accessibility:** the button content does not depend on the accent contrast; the target measures 36 px and the `{accent_ui}` border keeps the control contrast.
5. **Notice:** `.emu-system-notice` is a static 36 px `gtk::Label`, with no `:hover`, cursor or `connect_clicked`.
6. **Consistency:** the icon slot is still 48 px and uses `Center`; `Install`/`Remove`, the state→action matrix and `Working…` are not altered.

Options A and C remain documented as historical alternatives; I do not remove them from the document to keep the decision traceable.

## Deliberate risk and mitigation

The main risk is that the red notice reads as an action. The mitigation is deliberate and bounded: the notice is a `gtk::Label` with no hover, cursor or callback, and the button keeps the `.emu-link-action` language.

- the badge has no fill;
- the action uses 14 px against the badge's 11 px;
- the action uses a 20 px radius against 10 px;
- the action has a 1.5 px border against 1 px;
- the action label uses `text_main`, not accent;
- only `Install` gets a solid accent fill;
- the action border uses `{accent_ui}` (≥3:1 with the 10 accents) while the badge keeps `{accent}`; the action outline is deeper but stays within the accent family.

## Acceptance criteria for a future implementation

- The row keeps the existing order, spacing and target.
- State and action never appear as two identical badges.
- `System` stays visible during `Working…`, with badge, static notice and button in the same geometry.
- The action becomes insensitive during the operation and is restored when the response arrives.
- Success produces a `Linked` row with no action; error restores `Set as linked` and shows separate feedback.
- All proposed classes interpolate existing tokens; no extra hex appears.
- Keyboard focus stays visible and the action keeps a touch target of at least 36 px.
- A double click on `Set as linked` does not trigger a second operation: reentry guard + synchronous `set_sensitive(false)` + rebuild-proof lock when the map holds `Working…`.
- `Working…` is visible from the click until the result rebuild, not only at the end.
- The status label exists in every row (stable slot); the layout does not shift when the feedback appears.
- The action border meets ≥3:1 with the 10 accents in light and dark (`{accent_ui}` rule).
- `.emu-link-action` defines `:disabled` (8% fill, `text_muted` border and text).

## Final decision

- Current composition: `System` badge, static `.emu-system-notice` notice and `.emu-link-action` button with `Set as linked` text.
- The review resolutions are specified in this document: `Working…` flow and double click, stable slot, 3:1 contrast with the 10 accents and explicit separation between notice and action.
- The implementation covers the CSS of `.emu-link-action` and `.emu-system-notice`, the derived `{accent_ui}` token, the stable status label, the rebuild-proof lock and the `link_emulator_in` handler without duplication.

## List of installable emulators (catalog)

### Context and scope

This section keeps the catalog spec and its traceability; the current implementation of the row lives in `rebuild_emu_rows` and in the classes described above.

It is the same `rebuild_emu_rows` function, but in its catalog mode: when `emulator-manager` reports the catalog and most entries are in source `none`, the list keeps the `Install` button for pending entries and compacts the resolved rows. The name/description filter and the visual identity of each row are still part of the live option.

Goals of this proposal: hierarchy (name ≠ description), exploration (search), context (console/generation) and a stable slot for the emulator icon. The slot uses the 11 verified Papirus SVGs; the official Azahar logo is available and attributed, but the resolver keeps the monogram given the scope of this update.

### Verified current state

- Current row: horizontal `gtk::Box`, 8 px spacing, 2 px vertical margins:
  `[48px icon] [name + badge] [description] [stable slot] [optional static notice] [action]`.
- The name uses only `emu.name` with `.details-title` (16 px bold, `text_main`); it no longer concatenates `source` suffixes. The description stays on the second line with `.time-label`.
- `Linked`/native shows `Linked` in a 20 px pill; `System` shows `System` in a pill plus a static red notice; `none` shows no badge.
- `Install` button: `.add-btn` (solid accent fill, **30 px, 15 px radius**), fixed width 80 px. `Set as linked`: `.emu-link-action`, width 120 px. `Remove`: `.danger-btn`, width 80 px. The three classes share metrics: `font-size: 14px`, `min-height: 30px`, `padding: 0 14px`, `border-radius: 15px` — a capsule of 2.1× the font size, which is the comfortable ratio; they were 36 px (2.6×) before and read as disproportionate against the letters. The red `Found in system` notice also drops to `min-height: 30px` so that in the `System` row the notice and the button measure the same and the height does not get skewed.
- Alphabetical order by name enforced at `plugins.rs:787`. The name/description filter precedes the list; each row uses a Papirus icon when one exists and inherits the page scroll.
- Available data: `EmuInfo` (`plugins.rs:79-87`) brings `name`, `path`, `description`, `installed`, `native`, `source`, `settings`. **There is no console category, no version, no project URL and no icon field;** the UI resolves the asset by name. `RegistryEntry` (plugin registry) neither.

### Data constraints

- Console/generation **does not travel in the data**: I have to derive it on the UI side. Recommended: static `tag → family` map (same pattern `integration.rs:51-68` already uses for `runner → name`). Rejected alternative in this iteration: asking the backend for a new field (it touches the `emulator-manager` plugin, out of repo scope).
- The icon does not travel in the data: the UI keeps a static `normalized name → SVG file` map under `assets/icons/emulators/`. The fallback is a deterministic typographic marker (monogram of the initial) for Azahar or a missing asset, in the same geometry.

### Applied GNOME HIG recommendations

1. **List rows pattern (List Rows):** primary text (name) + secondary text (description/metadata) on two lines with different weights, left alignment, end ellipsis. The current row violates this by cramming everything into one bold line.
2. **Search as the entry point** for long lists: `GtkSearchEntry` is already part of the app (it searches in the sidebar and other views) and the theme already defines `.search-entry` and `.filter-bar`. For a catalog that is going to grow, the live name/description filter is the HIG entry control.
3. **Grouping with section headers** when there are recognizable categories: in this catalog the families are obvious (Nintendo vs Sony), which allows group headers instead of a flat list.
4. **Icon as the leading element**: the HIG starts at 32 px for content recognizable by visual identity; this section deliberately scales the slot to 48 px to align the row with Prism/Modrinth content lists (justification in "Row style: card"). The slot geometry is fixed so it does not push the text.
5. **Right-aligned action column** with a consistent width (140 px for the `Found in system` notice, 120 px for `Set as linked`, 80 px for `Install`/`Remove`): already satisfied; kept.
6. **Explicit empty state** after a filter with no results (message in `.time-label`), and an initial `hint` that today only says `"No emulators reported by emulator-manager."`.
7. Virtualization (`GtkListView`) is noted as a future improvement for catalogs of hundreds of items; for ~10-40 the current manual construction (`rebuild_emu_rows`) is acceptable and consistent with the rest of the app.

### Cross-cutting row decisions (fixed)

1. **Split the combined label into two lines:** line 1 name in `.details-title` (16 px bold), line 2 description in `.time-label` (12 px, `text_sec`, one line with ellipsis). The refactor is already applied in `rebuild_emu_rows`.
2. **Stable 48 px square leading icon slot, centered vertically and horizontally:** always reserved. The 11 emulators with a verified SVG use the asset within the same geometry; the `GtkImage` measures 40×40 and uses `Center`; Azahar or a missing asset show a deterministic monogram. The text and the action column do not move. Chosen size: 48 px; justification in "Row style: card".
3. **Name/description search** as the top control (`.search-entry` + `.filter-bar`, already defined): the live option is a flat list with a filter, without grouping.
4. **Aligned action column** and unchanged button vocabulary except for the `System` presentation: `.add-btn`, `.emu-link-action`, `.emu-system-notice` and `.danger-btn`; fixed width per state to avoid jitter.

### Live option: 1+3 — State-aware button + compaction

**Structure:**

```text
[search filter (GtkSearchEntry, .search-entry)]
[ .emu-row-card ]
  [48px icon]   Name + badge             (.details-title, bold 16, 1 line, ellipsis)
  [SVG or monogram]   Description — one line     (.time-label, 12, ellipsis)
                                      [slot 12ch]  [Found in system]  [Set as linked]
```

**Visual description:** flat list of two-line cards (cross-cutting decisions), with a button whose **label and class change with the row state** (state-aware) and resolved rows **with no pending action collapse to a badge** (compaction). The pill only appears where there is a real action: pending entries, in-flight operations and removable entries. The action column stays aligned with a fixed width per state, no jitter.

### Row style: card in the content-list style (Prism/Modrinth)

The row adopts the visual pattern of the content lists of Java Minecraft launchers (Prism Launcher / Modrinth): flat card with a neutral border, rounded corners and a panel background that separates each emulator. **Only the row container changes**: buttons, badges, stable slot and the live 1+3 option matrix stay intact (next section). I removed the `[•]` dot from this row (the usable state is already reflected in the line 1 badge and the button type); the `emu-dot-on`/`emu-dot-off` classes are now unused, candidates for removal. I do not copy the reference buttons (`View` / `Versions` / `Install`): those actions do not exist in `EmuOp`; I also do not add a selection checkbox (`EmuInfo` has none) nor an author field (it does not exist in `EmuInfo`).

**New `.emu-row-card` rule** (row frame):

```css
.emu-row-card {
  background-color: {panel};         /* a unit distinct from the page background */
  border: 1px solid {border};        /* neutral gray, not accent */
  border-radius: 6px;                /* corners ~6-8 px */
  padding: 8px 10px;                 /* inner padding ~8-10 px */
  margin: 2px 0;                     /* spacing between cards ~4-6 px */
}
```

Justification for the new class: no existing class models a neutral row card. Reusing `.mc-row` (Minecraft content list) or `.game-row` would couple the emulator catalog to another section's style: if the mod listing changes tomorrow, the emulator one would change for no related reason. A localized class follows the precedent of `.emu-link-action` (created for the launcher instead of reusing foreign vocabulary).

**Text block (two lines, no author field):** line 1 the name in `.details-title` (bold 16, single-line, ellipsize); line 2 the description in `.time-label` (12, `text_sec`, single-line, ellipsize). That is the split missing today: the current label concatenates `"Name (badge) — description"` into a single `.details-title` (`settings.rs:2452-2461`). I **remove** the `[•]` availability dot from the row: the usable state is already reflected in the line 1 badge and the button type; it is not replaced by a checkbox. The `emu-dot-on`/`emu-dot-off` classes are now unused (minor open item: candidates for removal).

**Icon slot (48 px, chosen):**

```css
.emu-row-icon {                       /* NEW */
  min-width: 48px;
  min-height: 48px;
  border-radius: 6px;                 /* echo of the card radius */
  background-color: {well};           /* monogram as fallback */
  border: 1px solid {border};
}
```

Why 48 px (requested range 48-56): it is the thumbnail size of Prism/Modrinth content lists, the reference analogy; the catalog moves 10-12 flat items with search, so 48 px gives the slot presence without inflating the row height (56 px would cost reading density while scrolling); and it coexists with the 12-char stable slot: the two-line identity reads complete and the monogram (emulator initial, ~18-20 px bold) stays legible as a fallback for Azahar or when an asset is missing. **Alignment:** the slot uses `Center` on both axes and the icon measures 40×40 inside the 48 px slot; the text block and the action column stay vertically centered relative to the card height. **Stable against the real asset:** the verified SVGs are served in the same geometry and the layout does not move.

**The icon is a `GtkPicture`, not a `GtkImage` (2026-09-25 correction).** `GtkImage:pixel-size` only applies to `ICON_NAME`-type images, so on a *paintable* —as is the case here, since it is built with `set_paintable`— the call is a no-op. The SVGs are 48×48, so the texture kept its full natural size, and `set_size_request(40, 40)` could not shrink it because it sets a **minimum, not a maximum**: the icon filled the slot and `Align::Center` had no room left to act, which is exactly why the centering was not noticeable. `GtkPicture` with `can_shrink(true)` + `ContentFit::Contain` does respect the 40×40, and since the paintable is square it fits without distortion.

**State → (badge, button text, CSS class, fixed width) matrix:** I **removed** the `Dot` column from the matrix and from the row: the usable state is reflected in the compact pill or in the action. The `emu-dot-on`/`emu-dot-off` classes are now unused (minor open item: candidates for removal).

| State | Badge | Button text | CSS class | Fixed width |
| --- | --- | --- | --- | --- |
| `none`/empty not installed | — | `Install` | `.add-btn` | 80 px |
| `EmuOp` in flight (Installing) | — | `Installing…` | `.add-btn` + **new rule** `.add-btn:disabled` | 96 px |
| installed `appimage` | — | `Remove` | `.danger-btn` | 80 px |
| detected `system` | `System` + `Found in system` notice (static) | `Set as linked` | `.emu-link-action` + `.emu-system-notice` | 120 px + 140 px |
| `linked` / native | `Linked` (`.proton-path-badge` + `.emu-status-pill`) | `Unlink` (`.emu-unlink-action`, red pill) | — | 80 px |
| `EmuOp(Err)` with recovery | — | `Retry` (or re-exposed `Install`/`Remove` depending on the previous state) | `.emu-link-action` | 96 px |

**Stable slot (12 chars, always present):** during the operation the slot shows `Working…` (`st.set_width_chars(12)`, `settings.rs:2548`); at rest it stays empty. `Working…` coexists with the pill or the action and does not push the layout. The button label (`Installing…`) is complementary, not a duplicate.

**Classes:** I reuse `.add-btn`, `.danger-btn`, `.emu-link-action`, `.proton-path-badge`, `.details-title`, `.time-label`, `.search-entry`/`.filter-bar`, `.seg-*`. **New rules already implemented:** `.emu-status-pill` (compact badge), `.emu-system-notice` (static red label with no affordance), `.emu-row-card` (row container) and `.emu-row-icon` (48 px icon slot), the last two in "Row style: card". **New rule specified but NOT implemented:** `.add-btn:disabled` for `Installing…`; it is recorded in "Open items of this section" because it affects the plugin catalog, not the emulator row. The `.emu-link-action:disabled` rule does exist and preserves the button's disabled state during `Working…`.

### Appendix: Evaluated and discarded options (traceability)

- **Option A — Two-line row with search:** its structure was absorbed as the cross-cutting base of 1+3 (two lines + filter); discarded as a standalone proposal because it does not define the button state treatment.
- **Option B — Headers per family:** discarded; it requires a `tag → family` map and a new helper (`.catalog-group`), and with 10 items the vertical overhead of headers does not pay off.
- **Option C — Family tabs + search:** discarded; it is the one with the most states and is over-engineering for a 10-12 item catalog; only worth revisiting if the catalog grows.
- **Option 2 — Stable imperative action + state in slot/badge:** discarded; it strictly honors "badge=state, button=action" but keeps the `Install` wall in the pending rows, the symptom 1+3 fixes.
- **Option 3 — Compaction (alone):** discarded as a standalone option; its technique (collapsing resolved rows into a badge) is integrated into 1+3.

**Comparison (reference):**

| Criterion | A: Two lines + search | B: Family headers | C: Tabs + search | 1+3 (live) |
| --- | --- | --- | --- | --- |
| Hierarchy fix | high | high | high | high |
| Console context | none | high | high | none (mitigated by search) |
| `Install` wall | persists | persists | persists | removed (labels + compaction) |
| New data needed | none | `tag→family` map | `tag→family` map | none |
| New theme classes | none | 1 helper (`.catalog-group`) | none | 3 new rules (`.emu-row-card`, `.emu-row-icon`, `.add-btn:disabled`) |

### Open items of this section

- **Real icon asset:** completed for the 11 emulators with a SVG verified in Papirus, and the official Azahar logo (CC BY 4.0) is downloaded and attributed in `assets/icons/ATTRIBUTION.md`. The 48 px slot (`.emu-row-icon`) stays stable; the resolver still uses the monogram for Azahar until a separate activation decision.
- **Orphan `emu-dot-on` / `emu-dot-off` classes (candidates for removal):** after my decision to remove the `[•]` dot from the row, the classes stayed defined in `helpers.rs:553-554` but unused in code, and the mention of the availability point in the "Typographic base" (`Set as linked` section) became obsolete. I do not delete them yet: since their contrast folklore (green `#00E639` / `text_muted`) is shared, I mark them as candidates for removal in a future cleanup.
- **`tag → family` map**: **does not apply to the live option** (flat list with filter, no grouping). It stays open only if the catalog grows and I revive appendix B or C.
- **Empty state gap**: the current list hint (`"No emulators reported by emulator-manager."`) does not tell "no plugin reported" apart from "no filter results"; an explicit empty state needs to be defined for each case (the live option includes search, so "no filter results" is a reachable state).
- **`.add-btn:disabled` not implemented**: the matrix of this section specifies that `Installing…` use `.add-btn` + `.add-btn:disabled` at 96 px, but `helpers.rs` only defines `.add-btn` and `.add-btn:hover`. Today a disabled `Install` during installation falls back to the GTK default style and breaks the consistency of the accent capsule. The specified rule would be the same as `.emu-link-action:disabled` (8% accent fill, `{text_muted}` border and text). I do not implement it in this iteration: it touches the plugin catalog and its verification is visual.
- **96 px widths not implemented**: the matrix reserves 96 px for `Installing…` and for `Retry` in error recovery, but `rebuild_emu_rows` sets 80 px for `Install`/`Remove` and 120 px for the pin. I have to decide the real width of the transient states so the text is not clipped and the column does not jump; it stays pending in the visual matrix.

## Recorded open items (out of scope)

- **`System`/`Linked` badge contrast:** the same risk resolved by the "3:1 contrast" section pre-exists in the state vocabulary: the badge text and border use `{accent}` and, with light accents in the light theme, fall below 3:1 (yellow darkened 22%: 2.01:1 against `#FFFFFF`). I do not fix it in this iteration; it stays recorded for a future task on badge vocabulary contrast.
- **Wine button:** the screenshot and the theme confirmation (light/dark) are still pending to diagnose the reported visual defect; the code causes have already been ruled out.
