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
                PadButton::North => NavAction::Menu,
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
            (PadButton::North, NavAction::Menu),
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
}
