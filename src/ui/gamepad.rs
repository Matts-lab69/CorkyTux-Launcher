//! Gamepad driver: gilrs on its own thread, events drained on the GTK loop.
//!
//! The thread owns Gilrs (hotplug and SDL mappings included) and only sends
//! plain data; no GTK object is touched there. The main loop applies the
//! actions, and D3 keeps the launcher quiet while a game has the focus.

use std::time::Duration;

use gtk::prelude::*;

use super::gamepad_nav::{self, Dir, NavAction, NavState, PadButton, PadInput};

pub enum GamepadMessage {
    Input(PadInput),
    Connected(String),
    Disconnected(String),
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn button_input(button: gilrs::Button, pressed: bool) -> Option<PadInput> {
    use gilrs::Button;
    Some(match button {
        Button::South => PadInput::Button(PadButton::South, pressed),
        Button::East => PadInput::Button(PadButton::East, pressed),
        Button::West => PadInput::Button(PadButton::West, pressed),
        Button::North => PadInput::Button(PadButton::North, pressed),
        Button::LeftTrigger => PadInput::Button(PadButton::Lb, pressed),
        Button::RightTrigger => PadInput::Button(PadButton::Rb, pressed),
        Button::Select => PadInput::Button(PadButton::Select, pressed),
        Button::Start => PadInput::Button(PadButton::Start, pressed),
        Button::DPadUp => PadInput::Dpad(Dir::Up, pressed),
        Button::DPadDown => PadInput::Dpad(Dir::Down, pressed),
        Button::DPadLeft => PadInput::Dpad(Dir::Left, pressed),
        Button::DPadRight => PadInput::Dpad(Dir::Right, pressed),
        _ => return None,
    })
}

fn spawn_thread(tx: async_channel::Sender<GamepadMessage>) {
    let _ = std::thread::Builder::new()
        .name("gamepad".to_string())
        .spawn(move || {
            let mut gilrs = match gilrs::Gilrs::new() {
                Ok(g) => g,
                Err(e) => {
                    // No /dev/input access, no udev: disable the feature, do
                    // not panic and do not retry in a loop.
                    eprintln!("[gamepad] disabled: {}", e);
                    return;
                }
            };
            let mut sticks: std::collections::HashMap<gilrs::GamepadId, (f32, f32)> =
                std::collections::HashMap::new();
            loop {
                while let Some(ev) = gilrs.next_event() {
                    let name = gilrs.gamepad(ev.id).name().to_string();
                    match ev.event {
                        gilrs::EventType::Connected => {
                            let _ = tx.send_blocking(GamepadMessage::Connected(name));
                        }
                        gilrs::EventType::Disconnected => {
                            sticks.remove(&ev.id);
                            let _ = tx.send_blocking(GamepadMessage::Disconnected(name));
                        }
                        gilrs::EventType::ButtonPressed(b, _) => {
                            if let Some(input) = button_input(b, true) {
                                let _ = tx.send_blocking(GamepadMessage::Input(input));
                            }
                        }
                        gilrs::EventType::ButtonReleased(b, _) => {
                            if let Some(input) = button_input(b, false) {
                                let _ = tx.send_blocking(GamepadMessage::Input(input));
                            }
                        }
                        gilrs::EventType::AxisChanged(axis, value, _) => {
                            let entry = sticks.entry(ev.id).or_insert((0.0, 0.0));
                            match axis {
                                gilrs::Axis::LeftStickX => entry.0 = value,
                                gilrs::Axis::LeftStickY => entry.1 = value,
                                _ => continue,
                            }
                            let (x, y) = *entry;
                            let _ = tx.send_blocking(GamepadMessage::Input(PadInput::Stick { x, y }));
                        }
                        _ => {}
                    }
                }
                std::thread::sleep(Duration::from_millis(8));
            }
        });
}

/// Wires the pad to the window. Best effort: without a controller or without
/// permissions the feature quietly does nothing.
pub fn start(window: &adw::ApplicationWindow) {
    let (tx, rx) = async_channel::unbounded::<GamepadMessage>();
    spawn_thread(tx);
    let state = std::rc::Rc::new(std::cell::RefCell::new(NavState::default()));

    let motion = gtk::EventControllerMotion::new();
    {
        let w = window.clone();
        motion.connect_motion(move |_, _, _| {
            w.remove_css_class("gamepad-mode");
        });
    }
    window.add_controller(motion);

    let keys = gtk::EventControllerKey::new();
    {
        let w = window.clone();
        keys.connect_key_pressed(move |_, _, _, _| {
            w.remove_css_class("gamepad-mode");
            glib::Propagation::Proceed
        });
    }
    window.add_controller(keys);

    let win = window.clone();
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let setting = crate::backend::config::ConfigManager::new()
            .launcher_value("controllerSupport")
            .map(|v| v != "0")
            .unwrap_or(true);
        if !gamepad_nav::should_handle(win.is_active(), win.is_visible(), setting) {
            // D3: discard while a game has the focus, so nothing queues up.
            while rx.try_recv().is_ok() {}
            return glib::ControlFlow::Continue;
        }
        while let Ok(msg) = rx.try_recv() {
            match msg {
                GamepadMessage::Input(input) => {
                    win.add_css_class("gamepad-mode");
                    let mut st = state.borrow_mut();
                    if let Some(action) = gamepad_nav::map_event(input, &mut st, now_ms()) {
                        drop(st);
                        apply_action(&win, action);
                    }
                }
                GamepadMessage::Connected(name) => {
                    eprintln!("[gamepad] connected: {}", name);
                }
                GamepadMessage::Disconnected(name) => {
                    eprintln!("[gamepad] disconnected: {}", name);
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

fn apply_action(window: &adw::ApplicationWindow, action: NavAction) {
    match action {
        NavAction::Move(dir) => {
            let direction = match dir {
                Dir::Up => gtk::DirectionType::Up,
                Dir::Down => gtk::DirectionType::Down,
                Dir::Left => gtk::DirectionType::Left,
                Dir::Right => gtk::DirectionType::Right,
            };
            window.child_focus(direction);
            if let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) {
                ensure_visible(&focus);
            }
        }
        NavAction::Activate => {
            if let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) {
                activate(&focus);
            }
        }
        NavAction::Back => close_top_modal(window),
        NavAction::TabPrev => {
            window.child_focus(gtk::DirectionType::Left);
        }
        NavAction::TabNext => {
            window.child_focus(gtk::DirectionType::Right);
        }
        NavAction::Search => {
            if let Some(entry) = find_search(window.upcast_ref()) {
                entry.grab_focus();
            }
        }
        NavAction::Context | NavAction::Menu => {}
    }
}

/// Buttons and rows activate themselves; a card's inner button is the usual
/// focus target. FlowBoxChild and ListBoxRow have their own activate signal.
fn activate(widget: &gtk::Widget) {
    if let Some(child) = widget.downcast_ref::<gtk::FlowBoxChild>() {
        child.emit_activate();
        return;
    }
    if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>() {
        row.emit_activate();
        return;
    }
    if widget.activate() {
        return;
    }
    let mut parent = widget.parent();
    while let Some(p) = parent {
        if p.activate() {
            return;
        }
        parent = p.parent();
    }
}

/// Scrolls the focused widget into its nearest ScrolledWindow.
fn ensure_visible(widget: &gtk::Widget) {
    let mut parent = widget.parent();
    while let Some(p) = parent {
        if let Some(scrolled) = p.downcast_ref::<gtk::ScrolledWindow>() {
            if let Some(bounds) = widget.compute_bounds(scrolled) {
                let height = scrolled.height() as f32;
                let adj = scrolled.vadjustment();
                if bounds.y() < 0.0 {
                    let next = (adj.value() + bounds.y() as f64).max(adj.lower());
                    adj.set_value(next);
                } else if bounds.y() + bounds.height() > height {
                    let delta = (bounds.y() + bounds.height() - height) as f64;
                    let max = (adj.upper() - adj.page_size()).max(adj.lower());
                    adj.set_value((adj.value() + delta).min(max));
                }
            }
            return;
        }
        parent = p.parent();
    }
}

fn close_top_modal(window: &adw::ApplicationWindow) {
    let Some(app) = window.application() else {
        return;
    };
    let window: &gtk::Window = window.upcast_ref();
    for w in app.windows() {
        if w.is_visible() && w.is_modal() && w != *window {
            w.close();
            return;
        }
    }
}

fn find_search(root: &gtk::Widget) -> Option<gtk::SearchEntry> {
    if let Some(entry) = root.downcast_ref::<gtk::SearchEntry>() {
        return Some(entry.clone());
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(found) = find_search(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}
