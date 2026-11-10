//! Gamepad driver: gilrs on its own thread, events drained on the GTK loop.
//!
//! The thread owns Gilrs (hotplug and SDL mappings included) and only sends
//! plain data; no GTK object is touched there. The main loop applies the
//! actions, and D3 keeps the launcher quiet while a game has the focus.

use std::time::Duration;

use gtk::prelude::*;

use super::gamepad_nav::{
    self, Dir, NavAction, NavRect, NavState, PadButton, PadInfo, PadInput, PadMapping, PadStamp,
    DEDUPE_WINDOW_MS,
};

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
            let mut pads: std::collections::HashMap<gilrs::GamepadId, (PadInfo, bool)> =
                std::collections::HashMap::new();
            let mut recent: Vec<PadStamp> = Vec::new();
            loop {
                while let Some(ev) = gilrs.next_event() {
                    let name = gilrs.gamepad(ev.id).name().to_string();
                    match ev.event {
                        gilrs::EventType::Connected => {
                            let gp = gilrs.gamepad(ev.id);
                            let info = PadInfo {
                                vendor: gp.vendor_id(),
                                product: gp.product_id(),
                                mapping: match gp.mapping_source() {
                                    gilrs::MappingSource::SdlMappings => PadMapping::Sdl,
                                    gilrs::MappingSource::Driver => PadMapping::Driver,
                                    _ => PadMapping::None,
                                },
                            };
                            let usable = gamepad_nav::is_gamepad(info);
                            pads.insert(ev.id, (info, usable));
                            if usable {
                                let _ = tx.send_blocking(GamepadMessage::Connected(name));
                            } else {
                                eprintln!("[gamepad] ignored non-gamepad device: {}", name);
                            }
                            continue;
                        }
                        gilrs::EventType::Disconnected => {
                            sticks.remove(&ev.id);
                            pads.remove(&ev.id);
                            let _ = tx.send_blocking(GamepadMessage::Disconnected(name));
                            continue;
                        }
                        _ => {}
                    }
                    let Some((info, usable)) = pads.get(&ev.id).copied() else {
                        continue;
                    };
                    if !usable {
                        continue;
                    }
                    let steam_virtual_connected = pads
                        .values()
                        .any(|(p, ok)| *ok && gamepad_nav::is_steam_virtual(*p));
                    if !gamepad_nav::accept_pad(info, steam_virtual_connected) {
                        continue;
                    }
                    let at_ms = ev
                        .time
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let stamp = PadStamp {
                        vendor: info.vendor,
                        product: info.product,
                        at_ms,
                    };
                    if !gamepad_nav::dedupe(stamp, &recent, DEDUPE_WINDOW_MS) {
                        continue;
                    }
                    recent.push(stamp);
                    recent.retain(|s| at_ms.saturating_sub(s.at_ms) <= 200);
                    let input = match ev.event {
                        gilrs::EventType::ButtonPressed(b, _) => button_input(b, true),
                        gilrs::EventType::ButtonReleased(b, _) => button_input(b, false),
                        gilrs::EventType::AxisChanged(axis, value, _) => {
                            let entry = sticks.entry(ev.id).or_insert((0.0, 0.0));
                            match axis {
                                gilrs::Axis::LeftStickX => {
                                    entry.0 = value;
                                    let (x, y) = *entry;
                                    Some(PadInput::Stick { x, y })
                                }
                                gilrs::Axis::LeftStickY => {
                                    entry.1 = value;
                                    let (x, y) = *entry;
                                    Some(PadInput::Stick { x, y })
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    if let Some(input) = input {
                        let _ = tx.send_blocking(GamepadMessage::Input(input));
                    }
                }
                std::thread::sleep(Duration::from_millis(8));
            }
        });
}

/// Wires the pad to the window. Best effort: without a controller or without
/// permissions the feature quietly does nothing.
pub fn start(
    window: &adw::ApplicationWindow,
    help: &gtk::Label,
    toasts: &adw::ToastOverlay,
    quick: std::rc::Rc<dyn Fn()>,
) {
    let (tx, rx) = async_channel::unbounded::<GamepadMessage>();
    spawn_thread(tx);
    let state = std::rc::Rc::new(std::cell::RefCell::new(NavState::default()));
    let config = crate::backend::config::ConfigManager::new();
    let repeat_delays = gamepad_nav::RepeatDelays::from_values(
        config
            .launcher_value("gamepadInitialRepeatDelay")
            .as_deref(),
        config.launcher_value("gamepadRepeatDelay").as_deref(),
    );

    let motion = gtk::EventControllerMotion::new();
    {
        // GTK synthesizes a motion event whenever the layout changes under a
        // still pointer, and showing the help bar does exactly that. Only a
        // real pointer movement may drop the pad ring.
        let last = std::rc::Rc::new(std::cell::Cell::new(None::<(f64, f64)>));
        let w = window.clone();
        let help_c = help.clone();
        motion.connect_motion(move |_, x, y| {
            let moved = last
                .get()
                .map(|(lx, ly)| (x - lx).abs() > 3.0 || (y - ly).abs() > 3.0)
                .unwrap_or(true);
            last.set(Some((x, y)));
            if moved {
                w.remove_css_class("gamepad-mode");
                help_c.set_visible(false);
            }
        });
    }
    window.add_controller(motion);

    // Keys do not clear the pad ring: controllers that emulate a keyboard
    // would otherwise remove it on the same press that set it. A real mouse
    // movement is the signal that the user switched input.
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, _, _, _| glib::Propagation::Proceed);
    window.add_controller(keys);

    let win = window.clone();
    let help_c = help.clone();
    let toasts_c = toasts.clone();
    let quick_c = quick.clone();
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let setting = crate::backend::config::ConfigManager::new()
            .launcher_value("controllerSupport")
            .map(|v| v != "0")
            .unwrap_or(true);
        // The pad must work in the window that actually has the focus: a
        // dialog presented as its own toplevel makes the main window
        // inactive, and gating on it dropped every event.
        let target: gtk::Window = win
            .application()
            .and_then(|app| app.active_window())
            .map(|w| w.upcast())
            .unwrap_or_else(|| win.clone().upcast());
        if !gamepad_nav::should_handle(target.is_active(), target.is_visible(), setting) {
            // D3: discard while a game has the focus, so nothing queues up.
            while rx.try_recv().is_ok() {}
            return glib::ControlFlow::Continue;
        }
        while let Ok(msg) = rx.try_recv() {
            match msg {
                GamepadMessage::Input(input) => {
                    target.add_css_class("gamepad-mode");
                    help_c.set_visible(true);
                    let mut st = state.borrow_mut();
                    if let Some(action) =
                        gamepad_nav::map_event_with_delays(input, &mut st, now_ms(), repeat_delays)
                    {
                        drop(st);
                        if action == NavAction::QuickAction {
                            quick_c();
                        } else {
                            apply_action(&target, action);
                        }
                    }
                }
                GamepadMessage::Connected(name) => {
                    let family = gamepad_nav::detect_family(&name);
                    help_c.set_text(gamepad_nav::help_text(family));
                    toasts_c.add_toast(adw::Toast::new(&format!("Controller connected: {}", name)));
                    eprintln!("[gamepad] connected: {}", name);
                }
                GamepadMessage::Disconnected(name) => {
                    toasts_c.add_toast(adw::Toast::new(&format!("Controller disconnected: {}", name)));
                    eprintln!("[gamepad] disconnected: {}", name);
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

fn apply_action(window: &gtk::Window, action: NavAction) {
    match action {
        NavAction::Move(dir) => move_focus(window, dir),
        NavAction::Activate => {
            if let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) {
                activate(&focus);
            }
        }
        NavAction::Back => close_top_modal(window),
        NavAction::TabPrev => {
            window.child_focus(gtk::DirectionType::TabBackward);
        }
        NavAction::TabNext => {
            window.child_focus(gtk::DirectionType::TabForward);
        }
        NavAction::Search => {
            if let Some(entry) = find_search(window.upcast_ref()) {
                entry.grab_focus();
            }
        }
        // North on an installed card would launch it and on a missing one
        // would install it, but the card only exposes its own click: tracked
        // in its issue for a dedicated play/install action.
        NavAction::QuickAction => {}
        NavAction::Context | NavAction::Menu => {}
    }
}

/// Moves the focus to the closest visible focusable widget in `dir`.
///
/// `gtk_widget_child_focus()` only walks the branch that already holds the
/// focus. Recently Played is a FlowBox, so the pad could move between cards
/// but never reach the filters, the library list or the header buttons.
fn move_focus(window: &gtk::Window, dir: Dir) {
    let Some(current) = gtk::prelude::GtkWindowExt::focus(window) else {
        // Nothing focused yet: keep GTK's first-widget choice.
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
        return;
    };
    let Some(current_rect) = widget_rect(&current, window) else {
        return;
    };

    let root = navigation_root(window);
    let mut candidates: Vec<(gtk::Widget, NavRect)> = Vec::new();
    collect_nav_targets(&root, window, &mut candidates);
    let rects: Vec<NavRect> = candidates.iter().map(|(_, rect)| *rect).collect();
    if let Some(index) = gamepad_nav::best_in_direction(current_rect, &rects, dir) {
        let target = candidates[index].0.clone();
        target.grab_focus();
        ensure_visible(&target);
    }
}

/// The widget tree that owns the current focus: the visible embedded dialog,
/// when there is one, so a pad cannot jump to the dimmed window behind it.
fn navigation_root(window: &gtk::Window) -> gtk::Widget {
    if let Some(app_win) = window.downcast_ref::<adw::ApplicationWindow>() {
        use adw::prelude::AdwApplicationWindowExt;
        if let Some(dialog) = app_win.visible_dialog() {
            return dialog.upcast();
        }
    }
    window.clone().upcast()
}

fn widget_rect(widget: &gtk::Widget, reference: &gtk::Window) -> Option<NavRect> {
    if !widget.is_visible() || !widget.is_mapped() || !widget.is_sensitive() {
        return None;
    }
    let bounds = widget.compute_bounds(reference)?;
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return None;
    }
    Some(NavRect::new(
        bounds.x() as f64,
        bounds.y() as f64,
        bounds.width() as f64,
        bounds.height() as f64,
    ))
}

fn collect_nav_targets(
    root: &gtk::Widget,
    reference: &gtk::Window,
    out: &mut Vec<(gtk::Widget, NavRect)>,
) {
    if is_nav_target(root) {
        if let Some(rect) = widget_rect(root, reference) {
            out.push((root.clone(), rect));
        }
    }
    let mut child = root.first_child();
    while let Some(next) = child {
        collect_nav_targets(&next, reference, out);
        child = next.next_sibling();
    }
}

/// Focusable widgets that make sense as pad targets. The list and flow boxes
/// are focusable containers: focusing them would only land on a row by
/// accident and skip the geometry that picks the right one.
fn is_nav_target(widget: &gtk::Widget) -> bool {
    if !widget.is_focusable() {
        return false;
    }
    if widget.is::<gtk::ListBox>() || widget.is::<gtk::FlowBox>() || widget.is::<gtk::Window>() {
        return false;
    }
    // The scrolled library and similar containers are focusable, but taking
    // the focus themselves moves nothing: their focusable children are the
    // real targets.
    !has_focusable_child(widget)
}

fn has_focusable_child(widget: &gtk::Widget) -> bool {
    let mut child = widget.first_child();
    while let Some(next) = child {
        if next.is_focusable() || has_focusable_child(&next) {
            return true;
        }
        child = next.next_sibling();
    }
    false
}

/// Buttons and rows activate themselves; a card's inner button is the usual
/// focus target. FlowBoxChild and ListBoxRow have their own activate signal.
fn activate(widget: &gtk::Widget) {
    if let Some(child) = widget.downcast_ref::<gtk::FlowBoxChild>() {
        // Library cards put the click handler on the inner game-card button;
        // the flow box only focuses the wrapper, so activating the wrapper
        // did nothing there.
        if let Some(card) = find_game_card(child.upcast_ref()) {
            card.activate();
        } else {
            child.emit_activate();
        }
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

fn close_top_modal(window: &gtk::Window) {
    // Embedded AdwDialogs are not toplevels and never appear in
    // `application().windows()`; this is the only handle on them.
    if let Some(app_win) = window.downcast_ref::<adw::ApplicationWindow>() {
        use adw::prelude::{AdwApplicationWindowExt, AdwDialogExt};
        if let Some(dialog) = app_win.visible_dialog() {
            dialog.close();
            return;
        }
    }
    for widget in gtk::Window::list_toplevels() {
        let Some(w) = widget.downcast_ref::<gtk::Window>() else {
            continue;
        };
        if w.is_visible() && w.is_modal() && !w.eq(window) {
            w.close();
            return;
        }
    }
}

/// The inner button that owns the click of a library card.
fn find_game_card(root: &gtk::Widget) -> Option<gtk::Button> {
    if let Some(button) = root.downcast_ref::<gtk::Button>() {
        if button.has_css_class("game-card") {
            return Some(button.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(found) = find_game_card(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
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
