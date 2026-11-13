//! Pure gamepad navigation: deadzone, repeat and button mapping.
//!
//! No GTK and no gilrs here, so the whole policy is unit-tested.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction {
    Move(Dir),
    Activate,
    Back,
    TabPrev,
    TabNext,
    Search,
    Context,
    Menu,
    QuickAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    Lb,
    Rb,
    Start,
    Select,
}

#[derive(Clone, Copy, Debug)]
pub enum PadInput {
    Button(PadButton, bool),
    Dpad(Dir, bool),
    Stick { x: f32, y: f32 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavState {
    pub dpad: Option<Dir>,
    pub stick: Option<Dir>,
    pub next_repeat_ms: u64,
}

impl NavState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Analog sticks below this magnitude do not move the focus.
pub const DEADZONE: f32 = 0.5;
/// First repeat delay while a direction is held.
pub const FIRST_REPEAT_MS: u64 = 400;
/// Following repeat delay.
pub const REPEAT_MS: u64 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepeatDelays {
    pub initial_ms: u64,
    pub repeat_ms: u64,
}

impl RepeatDelays {
    pub fn from_values(initial: Option<&str>, repeat: Option<&str>) -> Self {
        fn parse(value: Option<&str>, default: u64) -> u64 {
            value
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(default)
        }

        Self {
            initial_ms: parse(initial, FIRST_REPEAT_MS),
            repeat_ms: parse(repeat, REPEAT_MS),
        }
    }
}

impl Default for RepeatDelays {
    fn default() -> Self {
        Self {
            initial_ms: FIRST_REPEAT_MS,
            repeat_ms: REPEAT_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadMapping {
    Sdl,
    Driver,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadInfo {
    pub vendor: Option<u16>,
    pub product: Option<u16>,
    pub mapping: PadMapping,
}

/// Valve vendor id (Steam Deck, Steam Controller and the virtual pad).
pub const STEAM_VENDOR: u16 = 0x28de;
/// The virtual pad Steam Input exposes: SDL calls it "Steam Virtual Gamepad"
/// (`src/joystick/usb_ids.h`, `USB_PRODUCT_STEAM_VIRTUAL_GAMEPAD 0x11ff`, and
/// `controller_list.h` maps 28de:11ff to it). The Steam Deck builtin is
/// 28de:1205 and must NOT be treated as the virtual pad.
pub const STEAM_VIRTUAL_PRODUCT: u16 = 0x11ff;
/// Logitech G29 driving wheel: never a gamepad.
pub const G29_VENDOR: u16 = 0x046d;
pub const G29_PRODUCT: u16 = 0xc24f;
/// Two pads masked by Steam Input report the same press within this window.
/// Provisional: measured on no hardware yet; revisit after the calibration
/// dump (see the P4 evidence command).
pub const DEDUPE_WINDOW_MS: u64 = 10;

/// Only devices mapped as a gamepad drive navigation. Wheels, flight sticks
/// and pedals have no SDL mapping (or are the G29) and are refused.
pub fn is_gamepad(info: PadInfo) -> bool {
    if info.vendor == Some(G29_VENDOR) && info.product == Some(G29_PRODUCT) {
        return false;
    }
    !matches!(info.mapping, PadMapping::None)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadStamp {
    pub vendor: Option<u16>,
    pub product: Option<u16>,
    pub at_ms: u64,
}

/// 28de:11ff, the Steam Input virtual pad (not the Deck's builtin 28de:1205).
pub fn is_steam_virtual(info: PadInfo) -> bool {
    info.vendor == Some(STEAM_VENDOR) && info.product == Some(STEAM_VIRTUAL_PRODUCT)
}

/// Steam Input exposes a virtual pad with vendor 0x28de and the physical pad
/// produces the same press a few ms apart. Keep the virtual one and drop the
/// duplicate, so one press is one action.
pub fn dedupe(event: PadStamp, recent: &[PadStamp], window_ms: u64) -> bool {
    if event.vendor == Some(STEAM_VENDOR) && event.product == Some(STEAM_VIRTUAL_PRODUCT) {
        return true;
    }
    !recent.iter().any(|r| {
        r.vendor == Some(STEAM_VENDOR)
            && r.product == Some(STEAM_VIRTUAL_PRODUCT)
            && event.at_ms.abs_diff(r.at_ms) <= window_ms
    })
}

/// While a Steam virtual pad is connected it represents every physical pad:
/// only its events count.
pub fn accept_pad(info: PadInfo, steam_virtual_connected: bool) -> bool {
    if !is_gamepad(info) {
        return false;
    }
    if steam_virtual_connected && !is_steam_virtual(info) {
        return false;
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadFamily {
    PlayStation,
    Xbox,
    Nintendo,
    Generic,
}

/// Family from the controller name, used for the glyphs (F3b).
pub fn detect_family(name: &str) -> PadFamily {
    let n = name.to_lowercase();
    if n.contains("xbox") || n.contains("xinput") {
        return PadFamily::Xbox;
    }
    if n.contains("dualsense")
        || n.contains("dualshock")
        || n.contains("playstation")
        || n.contains("wireless controller")
    {
        return PadFamily::PlayStation;
    }
    if n.contains("nintendo") || n.contains("switch") || n.contains("joy-con") {
        return PadFamily::Nintendo;
    }
    PadFamily::Generic
}

/// What North does on the focused card: play when something is selected and
/// not already running, nothing otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickTarget {
    Play,
    None,
}

pub fn quick_action_target(
    view: &str,
    dialog_open: bool,
    focus_in_library: bool,
    selected: bool,
    already_running: bool,
) -> QuickTarget {
    if view == "games" && !dialog_open && focus_in_library && selected && !already_running {
        QuickTarget::Play
    } else {
        QuickTarget::None
    }
}

/// Bottom help bar text for the pad family. Only glyphs the family prints
/// are used; the physical South button always confirms.
pub fn help_text(family: PadFamily) -> &'static str {
    match family {
        PadFamily::PlayStation => {
            "✕ Select   ○ Back   □ Context   △ Quick   L1/R1 Tabs   Options Menu"
        }
        PadFamily::Xbox => "A Select   B Back   X Context   Y Quick   LB/RB Tabs   Menu Menu",
        PadFamily::Nintendo => "B Select   A Back   Y Context   X Quick   L/R Tabs   + Menu",
        PadFamily::Generic => {
            "South Select   East Back   West Context   North Quick   LB/RB Tabs   Start Menu"
        }
    }
}

/// D3: the pad is ignored unless the window is active, visible and the
/// feature is on.
pub fn should_handle(window_active: bool, window_visible: bool, setting: bool) -> bool {
    window_active && window_visible && setting
}

/// A widget rectangle in window coordinates, used for spatial focus moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl NavRect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    fn center(self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    fn overlaps_x(self, other: Self) -> bool {
        self.x < other.x + other.w && other.x < self.x + self.w
    }

    fn overlaps_y(self, other: Self) -> bool {
        self.y < other.y + other.h && other.y < self.y + self.h
    }
}

/// Picks the closest focus target in `dir` from the widget rectangles.
///
/// GTK's own arrow-key move only tries the branch that already holds the
/// focus, so a FlowBox (Recently Played) traps the pad forever. Choosing from
/// every visible focusable widget lets Left leave the cards for the sidebar
/// and Up reach the header buttons.
pub fn best_in_direction(current: NavRect, candidates: &[NavRect], dir: Dir) -> Option<usize> {
    let (cx, cy) = current.center();
    let mut best: Option<(f64, f64, f64, usize)> = None;
    // When nothing lies mostly along `dir`, fall back to the widget that is
    // closest to the movement axis. Up from a Recently Played card has only
    // the sidebar filters above it, so it still needs to go somewhere.
    let mut fallback: Option<(f64, f64, usize)> = None;

    for (i, candidate) in candidates.iter().enumerate() {
        let (x, y) = candidate.center();
        let dx = x - cx;
        let dy = y - cy;
        let (primary, perpendicular, aligned) = match dir {
            Dir::Up => (-dy, dx.abs(), current.overlaps_x(*candidate)),
            Dir::Down => (dy, dx.abs(), current.overlaps_x(*candidate)),
            Dir::Left => (-dx, dy.abs(), current.overlaps_y(*candidate)),
            Dir::Right => (dx, dy.abs(), current.overlaps_y(*candidate)),
        };
        if primary <= 0.5 {
            continue;
        }
        let off_axis = (perpendicular, primary, i);
        if fallback.map(|b| off_axis < b).unwrap_or(true) {
            fallback = Some(off_axis);
        }
        // A widget in the same row is not "above" just because its centre is
        // a few pixels higher. Require the move to point mostly along `dir`,
        // so Left reaches the sidebar instead of Up grabbing its row.
        if primary < perpendicular * 0.5 {
            continue;
        }
        // Alignment is worth at most a tenfold distance advantage. This keeps
        // header traversal without letting arbitrarily distant aligned items win.
        let score = if aligned {
            primary * 0.1 + perpendicular * 0.25
        } else {
            primary + perpendicular * 2.0
        };
        let entry = (score, primary, perpendicular, i);
        if best.map(|b| entry < b).unwrap_or(true) {
            best = Some(entry);
        }
    }

    best.map(|(_, _, _, i)| i)
        .or_else(|| fallback.map(|(_, _, i)| i))
}

/// Exhaust a scrolling scope before considering visible targets outside it.
pub fn best_in_scope(
    current: NavRect,
    candidates: &[(NavRect, bool, bool)],
    dir: Dir,
) -> Option<usize> {
    best_in_scope_using(current, candidates, dir, best_in_direction)
}

pub fn best_form_in_scope(
    current: NavRect,
    candidates: &[(NavRect, bool, bool)],
    dir: Dir,
) -> Option<usize> {
    best_in_scope_using(current, candidates, dir, best_in_form)
}

/// Form fields move horizontally within their row, vertically to the next row.
pub fn best_in_form(current: NavRect, candidates: &[NavRect], dir: Dir) -> Option<usize> {
    let (cx, cy) = current.center();
    let horizontal = matches!(dir, Dir::Left | Dir::Right);
    let eligible: Vec<(usize, NavRect)> = candidates
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, rect)| {
            let (x, y) = rect.center();
            match dir {
                Dir::Left => current.overlaps_y(*rect) && x < cx - 0.5,
                Dir::Right => current.overlaps_y(*rect) && x > cx + 0.5,
                Dir::Up => !current.overlaps_y(*rect) && y < cy,
                Dir::Down => !current.overlaps_y(*rect) && y > cy,
            }
        })
        .collect();
    if horizontal {
        return eligible
            .iter()
            .min_by(|a, b| {
                (a.1.center().0 - cx)
                    .abs()
                    .total_cmp(&(b.1.center().0 - cx).abs())
            })
            .map(|(index, _)| *index);
    }
    let frontier = eligible
        .iter()
        .min_by(|a, b| {
            (a.1.center().1 - cy)
                .abs()
                .total_cmp(&(b.1.center().1 - cy).abs())
        })?
        .1;
    eligible
        .iter()
        .filter(|(_, rect)| rect.overlaps_y(frontier))
        .min_by(|a, b| {
            (a.1.center().0 - cx)
                .abs()
                .total_cmp(&(b.1.center().0 - cx).abs())
        })
        .map(|(index, _)| *index)
}

fn best_in_scope_using(
    current: NavRect,
    candidates: &[(NavRect, bool, bool)],
    dir: Dir,
    choose: fn(NavRect, &[NavRect], Dir) -> Option<usize>,
) -> Option<usize> {
    for scoped in [true, false] {
        let eligible: Vec<(usize, NavRect)> = candidates
            .iter()
            .enumerate()
            .filter(|(_, (_, inside, visible))| if scoped { *inside } else { *visible })
            .map(|(i, (rect, _, _))| (i, *rect))
            .collect();
        let rects: Vec<_> = eligible.iter().map(|(_, rect)| *rect).collect();
        if let Some(i) = choose(current, &rects, dir) {
            return Some(eligible[i].0);
        }
    }
    None
}

pub fn reveal_value(value: f64, lower: f64, upper: f64, page: f64, start: f64, size: f64) -> f64 {
    let delta = if start < 0.0 || size > page {
        start
    } else {
        (start + size - page).max(0.0)
    };
    (value + delta).clamp(lower, (upper - page).max(lower))
}

fn stick_dir(x: f32, y: f32) -> Option<Dir> {
    if x.abs() < DEADZONE && y.abs() < DEADZONE {
        return None;
    }
    if x.abs() >= y.abs() {
        Some(if x > 0.0 { Dir::Right } else { Dir::Left })
    } else {
        Some(if y > 0.0 { Dir::Down } else { Dir::Up })
    }
}

/// D-pad wins over the stick.
fn effective(state: &NavState) -> Option<Dir> {
    state.dpad.or(state.stick)
}

fn step(
    before: Option<Dir>,
    after: Option<Dir>,
    state: &mut NavState,
    now_ms: u64,
    delays: RepeatDelays,
) -> Option<NavAction> {
    let dir = after?;
    if Some(dir) != before {
        state.next_repeat_ms = now_ms.saturating_add(delays.initial_ms);
        return Some(NavAction::Move(dir));
    }
    if now_ms >= state.next_repeat_ms {
        state.next_repeat_ms = now_ms.saturating_add(delays.repeat_ms);
        return Some(NavAction::Move(dir));
    }
    None
}

#[cfg(test)]
pub fn map_event(input: PadInput, state: &mut NavState, now_ms: u64) -> Option<NavAction> {
    map_event_with_delays(input, state, now_ms, RepeatDelays::default())
}

pub fn map_event_with_delays(
    input: PadInput,
    state: &mut NavState,
    now_ms: u64,
    delays: RepeatDelays,
) -> Option<NavAction> {
    match input {
        PadInput::Button(button, pressed) => {
            if !pressed {
                return None;
            }
            Some(match button {
                PadButton::South => NavAction::Activate,
                PadButton::East => NavAction::Back,
                PadButton::West => NavAction::Context,
                PadButton::North => NavAction::QuickAction,
                PadButton::Lb => NavAction::TabPrev,
                PadButton::Rb => NavAction::TabNext,
                PadButton::Select => NavAction::Search,
                PadButton::Start => NavAction::Menu,
            })
        }
        PadInput::Dpad(dir, pressed) => {
            let before = effective(state);
            state.dpad = if pressed {
                Some(dir)
            } else if state.dpad == Some(dir) {
                None
            } else {
                state.dpad
            };
            let after = effective(state);
            step(before, after, state, now_ms, delays)
        }
        PadInput::Stick { x, y } => {
            let before = effective(state);
            state.stick = stick_dir(x, y);
            let after = effective(state);
            step(before, after, state, now_ms, delays)
        }
    }
}

/// Drive held directions even when gilrs emits no more events for a steady axis.
#[cfg(test)]
pub fn repeat(state: &mut NavState, now_ms: u64) -> Option<NavAction> {
    repeat_with_delays(state, now_ms, RepeatDelays::default())
}

/// Keep held-axis polling consistent with the configured input-event delays.
#[cfg(test)]
pub fn repeat_with_delays(
    state: &mut NavState,
    now_ms: u64,
    delays: RepeatDelays,
) -> Option<NavAction> {
    let dir = effective(state);
    step(dir, dir, state, now_ms, delays)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_direction_preserves_configured_repeat_delays() {
        let mut state = NavState::default();
        let delays = RepeatDelays::from_values(Some("500"), Some("75"));
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Right, true), &mut state, 0, delays),
            Some(NavAction::Move(Dir::Right))
        );
        assert_eq!(repeat_with_delays(&mut state, 499, delays), None);
        assert_eq!(
            repeat_with_delays(&mut state, 500, delays),
            Some(NavAction::Move(Dir::Right))
        );
        assert_eq!(repeat_with_delays(&mut state, 574, delays), None);
        assert_eq!(
            repeat_with_delays(&mut state, 575, delays),
            Some(NavAction::Move(Dir::Right))
        );
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Right, false), &mut state, 576, delays),
            None
        );
        assert_eq!(repeat_with_delays(&mut state, 1000, delays), None);
    }

    #[test]
    fn gamepad_form_one_two_one_fields_never_skip_a_row() {
        let top = NavRect::new(0.0, 0.0, 300.0, 40.0);
        let left = NavRect::new(0.0, 60.0, 140.0, 40.0);
        let right = NavRect::new(160.0, 60.0, 140.0, 40.0);
        let bottom = NavRect::new(0.0, 120.0, 300.0, 40.0);
        let fields = [top, left, right, bottom];
        assert_eq!(best_in_form(top, &fields, Dir::Down), Some(1));
        assert_eq!(best_in_form(left, &fields, Dir::Right), Some(2));
        assert_eq!(best_in_form(right, &fields, Dir::Left), Some(1));
        assert_eq!(best_in_form(right, &fields, Dir::Up), Some(0));
        assert_eq!(best_in_form(right, &fields, Dir::Down), Some(3));
        assert_eq!(best_in_form(bottom, &fields, Dir::Up), Some(1));
        assert_eq!(best_in_form(right, &fields, Dir::Right), None);
        assert_eq!(best_in_form(top, &fields, Dir::Up), None);
    }

    #[test]
    fn gamepad_held_direction_repeats_without_new_device_events() {
        let mut state = NavState::default();
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Right, true), &mut state, 0),
            Some(NavAction::Move(Dir::Right))
        );
        assert_eq!(repeat(&mut state, 399), None);
        assert_eq!(repeat(&mut state, 400), Some(NavAction::Move(Dir::Right)));
        assert_eq!(repeat(&mut state, 400), None);
        assert_eq!(repeat(&mut state, 520), Some(NavAction::Move(Dir::Right)));
        map_event(PadInput::Dpad(Dir::Right, false), &mut state, 521);
        assert_eq!(repeat(&mut state, 800), None);
        assert_eq!(
            map_event(PadInput::Stick { x: -1.0, y: 0.0 }, &mut state, 900),
            Some(NavAction::Move(Dir::Left))
        );
        assert_eq!(repeat(&mut state, 1300), Some(NavAction::Move(Dir::Left)));
        state.reset();
        assert_eq!(repeat(&mut state, 1600), None);
    }

    #[test]
    fn disconnect_reset_releases_all_held_directions_and_repeat_deadline() {
        let mut state = NavState {
            dpad: Some(Dir::Left),
            stick: Some(Dir::Up),
            next_repeat_ms: 900,
        };
        state.reset();
        assert_eq!(state, NavState::default());
        assert_eq!(
            map_event(PadInput::Stick { x: 0.0, y: 0.0 }, &mut state, 1000),
            None
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Left, true), &mut state, 1001),
            Some(NavAction::Move(Dir::Left))
        );
    }

    #[test]
    fn deadzone_ignores_small_sticks() {
        let mut st = NavState::default();
        assert_eq!(
            map_event(PadInput::Stick { x: 0.4, y: 0.4 }, &mut st, 0),
            None
        );
        assert_eq!(
            map_event(PadInput::Stick { x: -0.49, y: 0.1 }, &mut st, 0),
            None
        );
        assert_eq!(
            map_event(PadInput::Stick { x: 0.5, y: 0.0 }, &mut st, 0),
            Some(NavAction::Move(Dir::Right))
        );
    }

    #[test]
    fn hold_repeats_at_400_then_120() {
        let mut st = NavState::default();
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 0),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 399),
            None
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 400),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 519),
            None
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 520),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, false), &mut st, 530),
            None
        );
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 600),
            Some(NavAction::Move(Dir::Down))
        );
    }

    #[test]
    fn repeat_delays_accept_positive_values_and_default_invalid_values() {
        assert_eq!(
            RepeatDelays::from_values(Some("600"), Some("75")),
            RepeatDelays {
                initial_ms: 600,
                repeat_ms: 75,
            }
        );
        assert_eq!(
            RepeatDelays::from_values(Some("0"), Some("invalid")),
            RepeatDelays::default()
        );
        assert_eq!(
            RepeatDelays::from_values(None, None),
            RepeatDelays::default()
        );
    }

    #[test]
    fn hold_uses_configured_repeat_delays() {
        let delays = RepeatDelays::from_values(Some("500"), Some("75"));
        let mut state = NavState::default();
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Down, true), &mut state, 0, delays),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Down, true), &mut state, 499, delays),
            None
        );
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Down, true), &mut state, 500, delays),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Down, true), &mut state, 574, delays),
            None
        );
        assert_eq!(
            map_event_with_delays(PadInput::Dpad(Dir::Down, true), &mut state, 575, delays),
            Some(NavAction::Move(Dir::Down))
        );
    }

    #[test]
    fn dpad_wins_over_the_stick() {
        let mut st = NavState::default();
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Up, true), &mut st, 0),
            Some(NavAction::Move(Dir::Up))
        );
        // The stick asks for the right while the D-pad holds up: no move.
        assert_eq!(
            map_event(PadInput::Stick { x: 0.9, y: 0.0 }, &mut st, 10),
            None
        );
        // Releasing the D-pad hands over to the stick.
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Up, false), &mut st, 20),
            Some(NavAction::Move(Dir::Right))
        );
        assert_eq!(
            map_event(PadInput::Stick { x: 0.0, y: 0.0 }, &mut st, 30),
            None
        );
    }

    #[test]
    fn buttons_map_to_actions() {
        let mut st = NavState::default();
        let cases = [
            (PadButton::South, NavAction::Activate),
            (PadButton::East, NavAction::Back),
            (PadButton::West, NavAction::Context),
            (PadButton::North, NavAction::QuickAction),
            (PadButton::Lb, NavAction::TabPrev),
            (PadButton::Rb, NavAction::TabNext),
            (PadButton::Select, NavAction::Search),
            (PadButton::Start, NavAction::Menu),
        ];
        for (button, action) in cases {
            assert_eq!(
                map_event(PadInput::Button(button, true), &mut st, 0),
                Some(action)
            );
            assert_eq!(map_event(PadInput::Button(button, false), &mut st, 0), None);
        }
    }

    #[test]
    fn families_from_real_names() {
        assert_eq!(detect_family("Wireless Controller"), PadFamily::PlayStation);
        assert_eq!(
            detect_family("DualSense Wireless Controller"),
            PadFamily::PlayStation
        );
        assert_eq!(detect_family("Xbox Wireless Controller"), PadFamily::Xbox);
        assert_eq!(
            detect_family("Nintendo Switch Pro Controller"),
            PadFamily::Nintendo
        );
        assert_eq!(detect_family("8BitDo Pro 2"), PadFamily::Generic);
    }

    #[test]
    fn window_state_gate() {
        assert!(should_handle(true, true, true));
        assert!(!should_handle(false, true, true));
        assert!(!should_handle(true, false, true));
        assert!(!should_handle(true, true, false));
    }
    #[test]
    fn only_mapped_gamepads_are_accepted() {
        let ds = PadInfo {
            vendor: Some(0x054c),
            product: Some(0x0ce6),
            mapping: PadMapping::Sdl,
        };
        let stick = PadInfo {
            vendor: Some(0x044f),
            product: Some(0xb10a),
            mapping: PadMapping::None,
        };
        let g29 = PadInfo {
            vendor: Some(G29_VENDOR),
            product: Some(G29_PRODUCT),
            mapping: PadMapping::Sdl,
        };
        let driver = PadInfo {
            vendor: Some(0x1234),
            product: Some(0x0001),
            mapping: PadMapping::Driver,
        };
        assert!(is_gamepad(ds));
        assert!(is_gamepad(driver));
        assert!(!is_gamepad(stick));
        assert!(!is_gamepad(g29));
        let pads = [ds, stick, g29, driver];
        let accepted: Vec<bool> = pads.iter().map(|p| is_gamepad(*p)).collect();
        assert_eq!(accepted, vec![true, false, false, true]);
    }

    #[test]
    fn steam_input_duplicates_are_dropped() {
        let virtual_pad = PadStamp {
            vendor: Some(STEAM_VENDOR),
            product: Some(STEAM_VIRTUAL_PRODUCT),
            at_ms: 1_000,
        };
        let physical = PadStamp {
            vendor: Some(0x054c),
            product: Some(0x0ce6),
            at_ms: 1_005,
        };
        assert!(dedupe(virtual_pad, &[], DEDUPE_WINDOW_MS));
        assert!(!dedupe(physical, &[virtual_pad], DEDUPE_WINDOW_MS));
        // Outside the window it is a real second press.
        let later = PadStamp {
            vendor: Some(0x054c),
            product: Some(0x0ce6),
            at_ms: 1_050,
        };
        assert!(dedupe(later, &[virtual_pad], DEDUPE_WINDOW_MS));
        // Two real pads, no virtual one: both valid.
        let pad_a = PadStamp {
            vendor: Some(0x054c),
            product: Some(0x0ce6),
            at_ms: 2_000,
        };
        let pad_b = PadStamp {
            vendor: Some(0x045e),
            product: Some(0x0b13),
            at_ms: 2_004,
        };
        assert!(dedupe(pad_b, &[pad_a], DEDUPE_WINDOW_MS));
        // A Steam Deck builtin (28de:1205) is physical, not virtual: it is
        // masked while the virtual pad fires and valid on its own.
        let deck = PadStamp {
            vendor: Some(STEAM_VENDOR),
            product: Some(0x1205),
            at_ms: 1_005,
        };
        assert!(!dedupe(deck, &[virtual_pad], DEDUPE_WINDOW_MS));
        let deck_later = PadStamp {
            vendor: Some(STEAM_VENDOR),
            product: Some(0x1205),
            at_ms: 3_000,
        };
        assert!(dedupe(deck_later, &[virtual_pad], DEDUPE_WINDOW_MS));
        // A lone virtual pad is valid.
        assert!(dedupe(
            PadStamp {
                vendor: Some(STEAM_VENDOR),
                product: Some(STEAM_VIRTUAL_PRODUCT),
                at_ms: 3_000
            },
            &[],
            DEDUPE_WINDOW_MS
        ));
    }

    #[test]
    fn steam_virtual_pad_masks_but_the_deck_does_not() {
        let virtual_pad = PadInfo {
            vendor: Some(STEAM_VENDOR),
            product: Some(STEAM_VIRTUAL_PRODUCT),
            mapping: PadMapping::Sdl,
        };
        let deck = PadInfo {
            vendor: Some(STEAM_VENDOR),
            product: Some(0x1205),
            mapping: PadMapping::Sdl,
        };
        let physical = PadInfo {
            vendor: Some(0x054c),
            product: Some(0x0ce6),
            mapping: PadMapping::Sdl,
        };
        let other = PadInfo {
            vendor: Some(0x045e),
            product: Some(0x0b13),
            mapping: PadMapping::Sdl,
        };
        assert!(accept_pad(virtual_pad, true));
        assert!(!accept_pad(physical, true));
        assert!(accept_pad(physical, false));
        // A Steam Deck's builtin pad is physical: it must not mask another pad.
        assert!(accept_pad(deck, false));
        assert!(accept_pad(physical, false));
        // Two non-Valve pads coexist.
        assert!(accept_pad(other, false));
    }

    #[test]
    fn help_bar_per_family() {
        assert!(help_text(PadFamily::PlayStation).contains("✕ Select"));
        assert!(help_text(PadFamily::Xbox).contains("A Select"));
        assert!(help_text(PadFamily::Nintendo).contains("B Select"));
        assert!(help_text(PadFamily::Generic).contains("South Select"));
        assert!(help_text(PadFamily::Xbox).contains("Y Quick"));
    }

    #[test]
    fn quick_action_policy() {
        // Focus on a game card (inside the library stack).
        assert_eq!(
            quick_action_target("games", false, true, true, false),
            QuickTarget::Play
        );
        // Focus on the details panel: not the library body.
        assert_eq!(
            quick_action_target("games", false, false, true, false),
            QuickTarget::None
        );
        // Focus on the sidebar: also outside the library body.
        assert_eq!(
            quick_action_target("games", false, false, true, false),
            QuickTarget::None
        );
        assert_eq!(
            quick_action_target("games", true, true, true, false),
            QuickTarget::None
        );
        assert_eq!(
            quick_action_target("games", false, true, false, false),
            QuickTarget::None
        );
        assert_eq!(
            quick_action_target("games", false, true, true, true),
            QuickTarget::None
        );
        assert_eq!(
            quick_action_target("settings", false, true, true, false),
            QuickTarget::None
        );
        assert_eq!(
            quick_action_target("stores", false, true, true, false),
            QuickTarget::None
        );
        assert_eq!(
            quick_action_target("minecraft", false, true, true, false),
            QuickTarget::None
        );
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NavRect {
        NavRect::new(x, y, w, h)
    }

    #[test]
    fn spatial_move_leaves_the_recently_played_flow() {
        // One card in Recently Played, the sidebar row at the same height and
        // the header buttons above it.
        let card = rect(400.0, 300.0, 200.0, 140.0);
        let sidebar_row = rect(10.0, 340.0, 210.0, 40.0);
        let filter = rect(20.0, 120.0, 90.0, 30.0);
        let settings = rect(900.0, 20.0, 100.0, 30.0);
        let candidates = [filter, sidebar_row, settings];
        assert_eq!(best_in_direction(card, &candidates, Dir::Left), Some(1));
        assert_eq!(best_in_direction(card, &candidates, Dir::Up), Some(0));
        assert_eq!(best_in_direction(card, &candidates, Dir::Down), None);
        assert_eq!(best_in_direction(card, &candidates, Dir::Right), Some(2));
    }

    #[test]
    fn spatial_move_reaches_the_filters_from_a_card() {
        // Real window coordinates: the filters sit far to the left of the
        // Recently Played row, so no widget is directly above a card.
        let card = rect(587.0, 173.0, 206.0, 146.0);
        let filter = rect(17.0, 144.0, 107.0, 44.0);
        let add_game = rect(17.0, 0.0, 100.0, 44.0);
        let settings = rect(1611.0, 0.0, 140.0, 44.0);
        assert_eq!(
            best_in_direction(card, &[filter, add_game, settings], Dir::Up),
            Some(0)
        );
        // Nothing sits below the cards.
        assert_eq!(
            best_in_direction(card, &[filter, add_game, settings], Dir::Down),
            None
        );
    }

    #[test]
    fn spatial_move_keeps_the_header_row() {
        // Right from Add game must reach Settings across the window header,
        // not the nearer sidebar icon below it.
        let add_game = rect(6.0, 6.0, 110.0, 38.0);
        let settings = rect(1611.0, 3.0, 140.0, 44.0);
        let sidebar_icon = rect(163.0, 85.0, 34.0, 34.0);
        assert_eq!(
            best_in_direction(add_game, &[sidebar_icon, settings], Dir::Right),
            Some(1)
        );
    }

    #[test]
    fn nearby_directional_target_beats_distant_aligned_target() {
        let current = rect(0.0, 0.0, 40.0, 40.0);
        let nearby = rect(60.0, 45.0, 40.0, 40.0);
        let distant = rect(5000.0, 0.0, 40.0, 40.0);
        assert_eq!(
            best_in_direction(current, &[distant, nearby], Dir::Right),
            Some(1)
        );
    }

    #[test]
    fn scrolling_scope_prefers_hidden_content_and_allows_exit_when_exhausted() {
        let current = rect(0.0, 0.0, 40.0, 40.0);
        let candidates = [
            (rect(0.0, 500.0, 40.0, 40.0), true, false),
            (rect(0.0, 60.0, 40.0, 40.0), false, true),
        ];
        assert_eq!(best_in_scope(current, &candidates, Dir::Down), Some(0));
        assert_eq!(
            best_in_scope(candidates[0].0, &candidates, Dir::Up),
            Some(1)
        );
        assert_eq!(
            best_in_scope(current, &[(candidates[0].0, false, false)], Dir::Down),
            None
        );
    }

    #[test]
    fn horizontal_scroll_scope_allows_lateral_exit_after_last_item() {
        let current = rect(200.0, 0.0, 40.0, 40.0);
        let inside = rect(0.0, 0.0, 40.0, 40.0);
        let outside = rect(260.0, 0.0, 40.0, 40.0);
        assert_eq!(
            best_in_scope(
                current,
                &[(inside, true, false), (outside, false, true)],
                Dir::Right
            ),
            Some(1)
        );
        assert_eq!(
            best_in_scope(
                current,
                &[(inside, true, false), (outside, false, true)],
                Dir::Left
            ),
            Some(0)
        );
    }

    #[test]
    fn reveal_clamps_each_axis_and_keeps_oversized_targets_leading_edge() {
        assert_eq!(reveal_value(100.0, 0.0, 1000.0, 200.0, -40.0, 30.0), 60.0);
        assert_eq!(reveal_value(100.0, 0.0, 1000.0, 200.0, 190.0, 40.0), 130.0);
        assert_eq!(reveal_value(100.0, 0.0, 1000.0, 200.0, 10.0, 400.0), 110.0);
        assert_eq!(reveal_value(790.0, 0.0, 1000.0, 200.0, 190.0, 100.0), 800.0);
        assert_eq!(reveal_value(0.0, 0.0, 100.0, 200.0, -50.0, 40.0), 0.0);
    }

    #[test]
    fn spatial_move_prefers_the_same_row() {
        let card = rect(400.0, 300.0, 200.0, 140.0);
        let next_card = rect(620.0, 300.0, 200.0, 140.0);
        let close_but_off_axis = rect(620.0, 80.0, 200.0, 140.0);
        assert_eq!(
            best_in_direction(card, &[close_but_off_axis, next_card], Dir::Right),
            Some(1)
        );
    }

    #[test]
    fn spatial_move_ignores_the_widget_under_the_focus() {
        // FlowBoxChild and its inner game-card button share the same bounds.
        let child = rect(400.0, 300.0, 200.0, 140.0);
        let inner = rect(400.0, 300.0, 200.0, 140.0);
        assert_eq!(best_in_direction(child, &[inner], Dir::Right), None);
    }

    #[test]
    fn sidebar_row_does_not_jump_to_a_filter_on_the_right() {
        let row = rect(10.0, 340.0, 210.0, 40.0);
        let filter = rect(137.0, 197.0, 90.0, 30.0);
        let card = rect(400.0, 300.0, 200.0, 140.0);
        assert_eq!(best_in_direction(row, &[filter, card], Dir::Right), Some(1));
    }

    #[test]
    fn spatial_move_reaches_the_header_from_a_sidebar_row() {
        let row = rect(10.0, 340.0, 210.0, 40.0);
        let add = rect(20.0, 20.0, 100.0, 30.0);
        let settings = rect(900.0, 20.0, 100.0, 30.0);
        assert_eq!(best_in_direction(row, &[add, settings], Dir::Up), Some(0));
    }
}
