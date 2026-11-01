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

/// Analog sticks below this magnitude do not move the focus.
pub const DEADZONE: f32 = 0.5;
/// First repeat delay while a direction is held.
pub const FIRST_REPEAT_MS: u64 = 400;
/// Following repeat delay.
pub const REPEAT_MS: u64 = 120;

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

pub fn quick_action_target(selected: bool, already_running: bool) -> QuickTarget {
    if selected && !already_running {
        QuickTarget::Play
    } else {
        QuickTarget::None
    }
}

/// Bottom help bar text for the pad family. Only glyphs the family prints
/// are used; the physical South button always confirms.
pub fn help_text(family: PadFamily) -> &'static str {
    match family {
        PadFamily::PlayStation => "✕ Select   ○ Back   □ Context   △ Quick   L1/R1 Tabs   Options Menu",
        PadFamily::Xbox => "A Select   B Back   X Context   Y Quick   LB/RB Tabs   Menu Menu",
        PadFamily::Nintendo => "B Select   A Back   Y Context   X Quick   L/R Tabs   + Menu",
        PadFamily::Generic => "South Select   East Back   West Context   North Quick   LB/RB Tabs   Start Menu",
    }
}

/// D3: the pad is ignored unless the window is active, visible and the
/// feature is on.
pub fn should_handle(window_active: bool, window_visible: bool, setting: bool) -> bool {
    window_active && window_visible && setting
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
) -> Option<NavAction> {
    let dir = after?;
    if Some(dir) != before {
        state.next_repeat_ms = now_ms + FIRST_REPEAT_MS;
        return Some(NavAction::Move(dir));
    }
    if now_ms >= state.next_repeat_ms {
        state.next_repeat_ms = now_ms + REPEAT_MS;
        return Some(NavAction::Move(dir));
    }
    None
}

pub fn map_event(input: PadInput, state: &mut NavState, now_ms: u64) -> Option<NavAction> {
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
            step(before, after, state, now_ms)
        }
        PadInput::Stick { x, y } => {
            let before = effective(state);
            state.stick = stick_dir(x, y);
            let after = effective(state);
            step(before, after, state, now_ms)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_ignores_small_sticks() {
        let mut st = NavState::default();
        assert_eq!(map_event(PadInput::Stick { x: 0.4, y: 0.4 }, &mut st, 0), None);
        assert_eq!(map_event(PadInput::Stick { x: -0.49, y: 0.1 }, &mut st, 0), None);
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
        assert_eq!(map_event(PadInput::Dpad(Dir::Down, true), &mut st, 399), None);
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 400),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(map_event(PadInput::Dpad(Dir::Down, true), &mut st, 519), None);
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Down, true), &mut st, 520),
            Some(NavAction::Move(Dir::Down))
        );
        assert_eq!(map_event(PadInput::Dpad(Dir::Down, false), &mut st, 530), None);
        assert_eq!(map_event(PadInput::Dpad(Dir::Down, true), &mut st, 600), Some(NavAction::Move(Dir::Down)));
    }

    #[test]
    fn dpad_wins_over_the_stick() {
        let mut st = NavState::default();
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Up, true), &mut st, 0),
            Some(NavAction::Move(Dir::Up))
        );
        // The stick asks for the right while the D-pad holds up: no move.
        assert_eq!(map_event(PadInput::Stick { x: 0.9, y: 0.0 }, &mut st, 10), None);
        // Releasing the D-pad hands over to the stick.
        assert_eq!(
            map_event(PadInput::Dpad(Dir::Up, false), &mut st, 20),
            Some(NavAction::Move(Dir::Right))
        );
        assert_eq!(map_event(PadInput::Stick { x: 0.0, y: 0.0 }, &mut st, 30), None);
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
        assert_eq!(
            detect_family("Xbox Wireless Controller"),
            PadFamily::Xbox
        );
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
        let ds = PadInfo { vendor: Some(0x054c), product: Some(0x0ce6), mapping: PadMapping::Sdl };
        let stick = PadInfo { vendor: Some(0x044f), product: Some(0xb10a), mapping: PadMapping::None };
        let g29 = PadInfo { vendor: Some(G29_VENDOR), product: Some(G29_PRODUCT), mapping: PadMapping::Sdl };
        let driver = PadInfo { vendor: Some(0x1234), product: Some(0x0001), mapping: PadMapping::Driver };
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
        // A Steam Deck builtin (28de:1205) is physical, not virtual.
        let deck = PadStamp {
            vendor: Some(STEAM_VENDOR),
            product: Some(0x1205),
            at_ms: 3_000,
        };
        assert!(!dedupe(deck, &[virtual_pad], DEDUPE_WINDOW_MS));
        // A lone virtual pad is valid.
        assert!(dedupe(
            PadStamp { vendor: Some(STEAM_VENDOR), product: Some(STEAM_VIRTUAL_PRODUCT), at_ms: 3_000 },
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
        assert_eq!(quick_action_target(true, false), QuickTarget::Play);
        assert_eq!(quick_action_target(true, true), QuickTarget::None);
        assert_eq!(quick_action_target(false, false), QuickTarget::None);
        assert_eq!(quick_action_target(false, true), QuickTarget::None);
    }

}
