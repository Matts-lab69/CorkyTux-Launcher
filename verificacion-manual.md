# Manual verification record

## User confirmation: binary18900eb

**CONFIRMADO POR EL USUARIO** (2026-11-11), not an automated or agent-observed
runtime result. Source: `18900ebf0ba21db1bc37b7eba6022668a86d9621`.
Installed executable: `~/.local/share/corkytux/corkytux`.
SHA256: `e4a9b6713f5d29efc87db4c17396e3970ed81118cb17465ac2eb281f99f97550`.

- [x] Tray: **CONFIRMADO POR EL USUARIO**, with18900eb.
- [x] Window behavior: **CONFIRMADO POR EL USUARIO**, with18900eb.
- [x] Window corners: **CONFIRMADO POR EL USUARIO**, with18900eb.
- [x] Eight-pixel scrollbar: **CONFIRMADO POR EL USUARIO**, with18900eb.
- [x] Controller: **CONFIRMADO POR EL USUARIO**, with18900eb; no claim for
  other controller families or an exhaustive hotplug/repeat matrix.
- [x] Steam: **CONFIRMADO POR EL USUARIO**, with18900eb; closed-client overlay
  and Flatpak are explicitly excluded.
- [x] Minecraft: **CONFIRMADO POR EL USUARIO**, with18900eb.
- [x] Stores/GOG: **CONFIRMADO POR EL USUARIO**, with18900eb.

These are the user's category-level confirmations only. They do not establish
unreported exact dimensions, theme pairs, device models, account sequences,
transport-failure simulation, latency measurements or memory-growth results.
An issue is closed only if its complete acceptance exactly matches the reported
confirmation; otherwise it stays open with a scope explanation.

## Explicitly untested

- [ ] Actual updater: **NO PROBADO**.
- [ ] Steam overlay with Steam initially closed: **NO PROBADO**.
- [ ] Other controller families: **NO PROBADO**.
- [ ] Steam Flatpak: **NO PROBADO**.

## New900x650 policy: separate from18900eb evidence

The new minimum is **900x650 logical pixels**, not the former800x600 request.
`WINDOW_MIN_WIDTH` and `WINDOW_MIN_HEIGHT` live in `src/ui/policies/layout.rs`.
Settings remains720x520. Its margins within the main-window budget are180
horizontal and130 vertical pixels; this does not change its preferred size.
The retained700px sidebar breakpoint remains unchanged and cannot be reached
by resizing a window with a900px minimum. Content requisitions may be larger
than the requested minimum; record actual allocation rather than assuming it.

- [ ] New900x650 artifact: **NO PROBADO** manually. Recheck window resize,
  Settings, details, corners, scrollbar and controller focus at reachable sizes.
  The user's18900eb confirmation does not transfer to a different binary.
- [ ] Exact issue-specific matrices not reported by the user: **NO PROBADO**.

No blanket completion status is inferred from unit, release or CI results.
