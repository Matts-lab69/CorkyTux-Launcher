//! Semantic GTK control adapters and per-window adjustment transactions.

use gtk::prelude::*;

use super::gamepad_nav::{Dir, NavAction};

pub fn is_control(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::Entry>()
        || widget.is::<gtk::SearchEntry>()
        || widget.is::<gtk::TextView>()
        || widget.is::<gtk::Range>()
        || widget.is::<gtk::SpinButton>()
        || widget.is::<gtk::DropDown>()
        || widget.is::<gtk::Switch>()
        || widget.is::<gtk::CheckButton>()
        || widget.is::<gtk::Button>()
        || widget.is::<gtk::MenuButton>()
}

fn target_focus_policy(focusable: bool, delegates_focus: bool) -> bool {
    focusable || delegates_focus
}

pub fn is_control_target(widget: &gtk::Widget) -> bool {
    target_focus_policy(
        widget.is_focusable(),
        widget.is::<gtk::DropDown>()
            || widget.is::<gtk::Entry>()
            || widget.is::<gtk::SearchEntry>()
            || widget.is::<gtk::SpinButton>()
            || widget.is::<gtk::MenuButton>(),
    )
}

pub fn is_form_control(widget: &gtk::Widget) -> bool {
    is_control(widget) && !widget.is::<gtk::Button>() && !widget.is::<gtk::MenuButton>()
}

pub fn owner(widget: &gtk::Widget) -> gtk::Widget {
    let mut node = Some(widget.clone());
    let mut fallback = widget.clone();
    let mut found = false;
    while let Some(current) = node {
        if current.is::<gtk::Entry>()
            || current.is::<gtk::SearchEntry>()
            || current.is::<gtk::TextView>()
            || current.is::<gtk::Range>()
            || current.is::<gtk::SpinButton>()
            || current.is::<gtk::DropDown>()
            || current.is::<gtk::Switch>()
            || current.is::<gtk::CheckButton>()
        {
            return current;
        }
        if !found && is_control(&current) {
            fallback = current.clone();
            found = true;
        }
        node = current.parent();
    }
    fallback
}

#[derive(Clone)]
enum Original {
    Number(f64),
    Selection(Option<glib::Object>),
}

struct Edit {
    widget: glib::WeakRef<gtk::Widget>,
    original: Original,
}

#[derive(Default)]
pub struct ControlState {
    edit: Option<Edit>,
}

#[derive(Debug, PartialEq)]
enum EditIntent {
    Begin,
    Accept,
    Cancel,
    Adjust(Dir),
    Leave,
    Ignore,
}

fn edit_intent(editing: bool, action: NavAction) -> EditIntent {
    if !editing {
        return if action == NavAction::Activate {
            EditIntent::Begin
        } else {
            EditIntent::Ignore
        };
    }
    match action {
        NavAction::Activate => EditIntent::Accept,
        NavAction::Back => EditIntent::Cancel,
        NavAction::Move(dir) => EditIntent::Adjust(dir),
        _ => EditIntent::Leave,
    }
}

fn scale_move(horizontal: bool, dir: Dir) -> EditIntent {
    if horizontal == matches!(dir, Dir::Left | Dir::Right) {
        EditIntent::Adjust(dir)
    } else {
        EditIntent::Leave
    }
}

fn selection_step(selected: u32, count: u32, positive: bool) -> Option<u32> {
    if count == 0 {
        return None;
    }
    if selected >= count {
        return Some(if positive { 0 } else { count - 1 });
    }
    Some(if positive {
        selected.saturating_add(1).min(count - 1)
    } else {
        selected.saturating_sub(1)
    })
}

fn popup_position(selected: u32, count: u32, matches_item: impl Fn(u32) -> bool) -> Option<u32> {
    if selected < count && matches_item(selected) {
        return Some(selected);
    }
    (0..count).find(|position| matches_item(*position))
}

fn popup_list(root: &gtk::Widget) -> Option<gtk::ListView> {
    if let Some(list) = root.downcast_ref::<gtk::ListView>() {
        return Some(list.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        if let Some(list) = popup_list(&node) {
            return Some(list);
        }
        child = node.next_sibling();
    }
    None
}

fn reveal_dropdown(drop: &gtk::DropDown) {
    let Some(popover) = visible_popover(drop.upcast_ref()) else {
        return;
    };
    let Some(list) = popup_list(popover.upcast_ref()) else {
        return;
    };
    let (Some(item), Some(model)) = (drop.selected_item(), list.model()) else {
        return;
    };
    // GTK resets its filter on selected-notify. Keep that native position when
    // it still identifies the current object; callbacks may have replaced a model.
    let Some(position) = popup_position(drop.selected(), model.n_items(), |position| {
        model.item(position).as_ref() == Some(&item)
    }) else {
        return;
    };
    list.scroll_to(
        position,
        gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT,
        None,
    );
}

fn queue_dropdown_reveal(drop: &gtk::DropDown) {
    reveal_dropdown(drop);
    let weak = drop.downgrade();
    // Initial popup allocation may follow activation. Re-read live state rather
    // than retaining a position that can become stale before the idle callback.
    glib::idle_add_local_once(move || {
        if let Some(drop) = weak.upgrade() {
            reveal_dropdown(&drop);
        }
    });
}

fn spin_scroll(editable: bool, dir: Dir) -> Option<gtk::ScrollType> {
    editable.then_some(if matches!(dir, Dir::Right | Dir::Up) {
        gtk::ScrollType::StepUp
    } else {
        gtk::ScrollType::StepDown
    })
}

fn original_value(widget: &gtk::Widget) -> Option<Original> {
    if let Some(range) = widget.downcast_ref::<gtk::Range>() {
        return Some(Original::Number(range.value()));
    }
    if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
        return Some(Original::Number(spin.value()));
    }
    widget
        .downcast_ref::<gtk::DropDown>()
        .map(|drop| Original::Selection(drop.selected_item()))
}

fn restore(widget: &gtk::Widget, original: Original) {
    match original {
        Original::Number(value) => {
            if let Some(range) = widget.downcast_ref::<gtk::Range>() {
                range.set_value(value);
            } else if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
                spin.set_value(value);
            }
        }
        Original::Selection(item) => {
            if let Some(drop) = widget.downcast_ref::<gtk::DropDown>() {
                let position = drop
                    .model()
                    .and_then(|model| (0..model.n_items()).find(|i| model.item(*i) == item))
                    .unwrap_or(gtk::INVALID_LIST_POSITION);
                drop.set_selected(position);
            }
        }
    }
}

fn adjust(widget: &gtk::Widget, dir: Dir) {
    let positive = matches!(dir, Dir::Right | Dir::Up);
    let sign = if positive { 1.0 } else { -1.0 };
    if let Some(range) = widget.downcast_ref::<gtk::Range>() {
        range.set_value(range.value() + sign * range.adjustment().step_increment());
    } else if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
        if let Some(scroll) = spin_scroll(spin.is_editable(), dir) {
            spin.update();
            spin.emit_change_value(scroll);
        }
    } else if let Some(drop) = widget.downcast_ref::<gtk::DropDown>() {
        let count = drop.model().map(|m| m.n_items()).unwrap_or(0);
        // Down advances through a list; Up increases a numeric control.
        if let Some(next) = selection_step(
            drop.selected(),
            count,
            matches!(dir, Dir::Down | Dir::Right),
        ) {
            drop.set_selected(next);
            queue_dropdown_reveal(drop);
        }
    }
}

pub fn visible_popover(root: &gtk::Widget) -> Option<gtk::Popover> {
    if let Some(popover) = root.downcast_ref::<gtk::Popover>() {
        if popover.is_visible() && popover.is_mapped() {
            return Some(popover.clone());
        }
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        if let Some(popover) = visible_popover(&node) {
            return Some(popover);
        }
        child = node.next_sibling();
    }
    None
}

impl ControlState {
    pub fn clear(&mut self) {
        self.finish(false);
    }

    fn finish(&mut self, cancel: bool) {
        let Some(edit) = self.edit.take() else { return };
        let Some(widget) = edit.widget.upgrade() else {
            return;
        };
        if cancel && widget.activate_action("ram.cancel", None).is_err() {
            restore(&widget, edit.original);
        }
        if let Some(popover) = visible_popover(&widget) {
            popover.popdown();
        }
        // RAM scales expose an optional commit boundary; other controls need none.
        if !cancel {
            if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
                if spin.is_editable() {
                    spin.update();
                }
            }
            let _ = widget.activate_action("ram.commit", None);
        }
    }

    pub fn validate(&mut self, window: &gtk::Window) {
        let valid = self
            .edit
            .as_ref()
            .and_then(|edit| edit.widget.upgrade())
            .map(|widget| {
                widget.is_visible()
                    && widget.is_mapped()
                    && widget.is_sensitive()
                    && gtk::prelude::GtkWindowExt::focus(window)
                        .map(|focus| focus == widget || focus.is_ancestor(&widget))
                        .unwrap_or(false)
            })
            .unwrap_or(self.edit.is_none());
        if !valid {
            self.finish(false);
        }
    }

    pub fn handle(&mut self, window: &gtk::Window, action: NavAction) -> bool {
        self.validate(window);
        let focus = gtk::prelude::GtkWindowExt::focus(window).map(|focus| owner(&focus));
        if let Some(scale) = focus
            .as_ref()
            .and_then(|widget| widget.downcast_ref::<gtk::Scale>())
        {
            match action {
                NavAction::Move(dir) => {
                    if scale_move(scale.orientation() == gtk::Orientation::Horizontal, dir)
                        == EditIntent::Leave
                    {
                        self.finish(false);
                        return false;
                    }
                    if self.edit.is_none() && !self.begin(window) {
                        return false;
                    }
                    adjust(scale.upcast_ref(), dir);
                    return true;
                }
                NavAction::Activate => {
                    self.finish(false);
                    let _ = scale.activate_action("ram.commit", None);
                    return true;
                }
                _ => {}
            }
        }
        match edit_intent(self.edit.is_some(), action) {
            EditIntent::Cancel => self.finish(true),
            EditIntent::Accept => self.finish(false),
            EditIntent::Adjust(dir) => {
                if let Some(widget) = self.edit.as_ref().and_then(|edit| edit.widget.upgrade()) {
                    adjust(&widget, dir);
                }
            }
            EditIntent::Leave => {
                self.finish(false);
                return false;
            }
            EditIntent::Ignore => return false,
            EditIntent::Begin => return self.begin(window),
        }
        true
    }

    fn begin(&mut self, window: &gtk::Window) -> bool {
        let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) else {
            return false;
        };
        if let Some(popover) = visible_popover(window.upcast_ref()) {
            if !focus.is_ancestor(&popover) {
                return false;
            }
        }
        let widget = owner(&focus);
        if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
            if !spin.is_editable() {
                return true;
            }
            spin.update();
        }
        if let Some(original) = original_value(&widget) {
            let _ = widget.activate_action("ram.begin", None);
            self.edit = Some(Edit {
                widget: widget.downgrade(),
                original,
            });
            if let Some(drop) = widget.downcast_ref::<gtk::DropDown>() {
                drop.emit_activate();
                queue_dropdown_reveal(drop);
            }
            return true;
        }
        if let Some(switch) = widget.downcast_ref::<gtk::Switch>() {
            switch.set_active(!switch.is_active());
            return true;
        }
        if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
            check.set_active(!check.is_active());
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepad_horizontal_slider_adjusts_inline_and_leaves_on_vertical_moves() {
        assert_eq!(scale_move(true, Dir::Left), EditIntent::Adjust(Dir::Left));
        assert_eq!(scale_move(true, Dir::Right), EditIntent::Adjust(Dir::Right));
        assert_eq!(scale_move(true, Dir::Up), EditIntent::Leave);
        assert_eq!(scale_move(true, Dir::Down), EditIntent::Leave);
        assert_eq!(scale_move(false, Dir::Up), EditIntent::Adjust(Dir::Up));
        assert_eq!(scale_move(false, Dir::Right), EditIntent::Leave);
    }

    #[test]
    fn leaf_controls_require_focusability_but_compounds_can_delegate() {
        assert!(!target_focus_policy(false, false));
        assert!(target_focus_policy(true, false));
        assert!(target_focus_policy(false, true));
    }

    #[test]
    fn popup_reveal_preserves_native_index_and_handles_model_replacement() {
        assert_eq!(popup_position(900, 1000, |i| i == 900), Some(900));
        assert_eq!(popup_position(2, 5, |i| i == 1 || i == 2), Some(2));
        assert_eq!(popup_position(900, 3, |i| i == 1), Some(1));
        assert_eq!(popup_position(900, 0, |_| true), None);
        assert_eq!(popup_position(2, 3, |_| false), None);
    }

    #[test]
    fn spin_adjustment_honors_editability_and_semantic_step_direction() {
        for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
            assert_eq!(spin_scroll(false, dir), None);
        }
        assert_eq!(spin_scroll(true, Dir::Up), Some(gtk::ScrollType::StepUp));
        assert_eq!(spin_scroll(true, Dir::Right), Some(gtk::ScrollType::StepUp));
        assert_eq!(
            spin_scroll(true, Dir::Down),
            Some(gtk::ScrollType::StepDown)
        );
        assert_eq!(
            spin_scroll(true, Dir::Left),
            Some(gtk::ScrollType::StepDown)
        );
    }

    #[test]
    fn confirm_enters_then_accepts_and_back_cancels_before_dialog() {
        assert_eq!(edit_intent(false, NavAction::Activate), EditIntent::Begin);
        assert_eq!(edit_intent(true, NavAction::Activate), EditIntent::Accept);
        assert_eq!(edit_intent(true, NavAction::Back), EditIntent::Cancel);
        assert_eq!(edit_intent(false, NavAction::Back), EditIntent::Ignore);
        assert_eq!(
            edit_intent(true, NavAction::Move(Dir::Left)),
            EditIntent::Adjust(Dir::Left)
        );
        assert_eq!(edit_intent(true, NavAction::TabNext), EditIntent::Leave);
    }

    #[test]
    fn empty_and_changed_dropdown_models_are_safe() {
        assert_eq!(selection_step(0, 0, true), None);
        assert_eq!(selection_step(u32::MAX, 3, true), Some(0));
        assert_eq!(selection_step(8, 3, false), Some(2));
        assert_eq!(selection_step(2, 3, true), Some(2));
        assert_eq!(selection_step(0, 3, false), Some(0));
    }
}
