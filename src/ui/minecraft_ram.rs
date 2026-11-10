//! RAM editing boundaries shared by the global and per-instance settings.

use std::{cell::RefCell, rc::Rc};

use gtk::prelude::*;

const MIN_MB: f64 = 512.0;
const MAX_MB: f64 = 16384.0;
const STEP_MB: f64 = 256.0;

#[derive(Debug)]
struct SavePolicy {
    context: String,
    saved: u32,
    original: Option<(String, u32)>,
}

impl SavePolicy {
    fn commit(&mut self, context: String, value: u32) -> bool {
        let save = !context.is_empty() && self.context == context && self.saved != value;
        self.context = context;
        self.saved = value;
        self.original = None;
        save
    }

    fn rebase(&mut self, context: String, value: u32) {
        self.context = context;
        self.saved = value;
        self.original = None;
    }

    fn cancel_value(&mut self, context: &str) -> Option<u32> {
        self.original
            .take()
            .filter(|(id, _)| id == context)
            .map(|(_, value)| value)
    }
}

fn typed_mb(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(MIN_MB, MAX_MB))
}

pub fn bind(
    scale: &gtk::Scale,
    entry: &gtk::Entry,
    label: &gtk::Label,
    context: impl Fn() -> String + 'static,
    persist: impl Fn(&str, u32) + 'static,
) {
    scale.set_range(MIN_MB, MAX_MB);
    scale.set_increments(STEP_MB, STEP_MB);
    let context: Rc<dyn Fn() -> String> = Rc::new(context);
    let policy = Rc::new(RefCell::new(SavePolicy {
        context: context(),
        saved: scale.value() as u32,
        original: None,
    }));
    let commit: Rc<dyn Fn()> = {
        let weak = scale.downgrade();
        let policy = policy.clone();
        let context = context.clone();
        Rc::new(move || {
            let Some(scale) = weak.upgrade() else { return };
            let id = context();
            let value = scale.value() as u32;
            if policy.borrow_mut().commit(id.clone(), value) {
                persist(&id, value);
            }
        })
    };
    let rebase: Rc<dyn Fn()> = {
        let weak = scale.downgrade();
        let policy = policy.clone();
        let context = context.clone();
        Rc::new(move || {
            if let Some(scale) = weak.upgrade() {
                policy.borrow_mut().rebase(context(), scale.value() as u32);
            }
        })
    };
    let begin: Rc<dyn Fn()> = {
        let weak = scale.downgrade();
        let policy = policy.clone();
        let context = context.clone();
        Rc::new(move || {
            if let Some(scale) = weak.upgrade() {
                policy.borrow_mut().original = Some((context(), scale.value() as u32));
            }
        })
    };
    let cancel: Rc<dyn Fn()> = {
        let weak = scale.downgrade();
        let policy = policy.clone();
        Rc::new(move || {
            if let Some(scale) = weak.upgrade() {
                let id = context();
                let original = policy.borrow_mut().cancel_value(&id);
                if let Some(value) = original {
                    scale.set_value(value as f64);
                }
                policy.borrow_mut().rebase(id, scale.value() as u32);
            }
        })
    };
    let actions = gio::SimpleActionGroup::new();
    for (name, callback) in [
        ("commit", commit.clone()),
        ("begin", begin),
        ("cancel", cancel),
        ("rebase", rebase),
    ] {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| callback());
        actions.add_action(&action);
    }
    scale.insert_action_group("ram", Some(&actions));

    let weak_entry = entry.downgrade();
    let weak_label = label.downgrade();
    scale.connect_value_changed(move |scale| {
        let value = scale.value() as u32;
        if let Some(entry) = weak_entry.upgrade() {
            entry.set_text(&value.to_string());
        }
        if let Some(label) = weak_label.upgrade() {
            label.set_text(&format!("{} MB", value));
        }
    });

    let release = gtk::EventControllerLegacy::new();
    release.set_propagation_phase(gtk::PropagationPhase::Capture);
    let save = commit.clone();
    release.connect_event(move |_, event| {
        if event.event_type() == gtk::gdk::EventType::ButtonRelease {
            save();
        }
        glib::Propagation::Proceed
    });
    scale.add_controller(release);

    let keys = gtk::EventControllerKey::new();
    let save = commit.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
            save();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    scale.add_controller(keys);

    let focus = gtk::EventControllerFocus::new();
    let save = commit.clone();
    focus.connect_leave(move |_| save());
    scale.add_controller(focus);

    let apply_entry: Rc<dyn Fn()> = {
        let weak_scale = scale.downgrade();
        let weak_entry = entry.downgrade();
        Rc::new(move || {
            let (Some(scale), Some(entry)) = (weak_scale.upgrade(), weak_entry.upgrade()) else {
                return;
            };
            if let Some(value) = typed_mb(&entry.text()) {
                scale.set_value(value);
                commit();
            } else {
                entry.set_text(&(scale.value() as u32).to_string());
            }
        })
    };
    let apply = apply_entry.clone();
    entry.connect_activate(move |_| apply());
    let focus = gtk::EventControllerFocus::new();
    focus.connect_leave(move |_| apply_entry());
    entry.add_controller(focus);
}

pub fn rebase(scale: &gtk::Scale) {
    let _ = scale.activate_action("ram.rebase", None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_only_actual_changes_once_at_boundaries() {
        let mut policy = SavePolicy {
            context: "instance-a".into(),
            saved: 2048,
            original: None,
        };
        assert!(!policy.commit("instance-a".into(), 2048));
        assert!(policy.commit("instance-a".into(), 2304));
        assert!(!policy.commit("instance-a".into(), 2304));
    }

    #[test]
    fn changing_instance_or_cancel_rebase_does_not_write() {
        let mut policy = SavePolicy {
            context: "instance-a".into(),
            saved: 2048,
            original: Some(("instance-a".into(), 2048)),
        };
        assert!(!policy.commit("instance-b".into(), 4096));
        assert_eq!(policy.cancel_value("instance-b"), None);
        policy.rebase("instance-b".into(), 4096);
        assert!(!policy.commit("instance-b".into(), 4096));
        assert!(!policy.commit(String::new(), 8192));
    }

    #[test]
    fn cancel_restores_edit_start_without_saving_and_loading_clears_snapshot() {
        let mut policy = SavePolicy {
            context: "instance-a".into(),
            saved: 2048,
            original: Some(("instance-a".into(), 2048)),
        };
        let original = policy.cancel_value("instance-a").unwrap();
        assert_eq!(original, 2048);
        policy.rebase("instance-a".into(), original);
        assert!(!policy.commit("instance-a".into(), original));
        policy.original = Some(("instance-a".into(), 2048));
        policy.rebase("instance-b".into(), 4096);
        assert_eq!(policy.cancel_value("instance-b"), None);
        assert!(!policy.commit("instance-b".into(), 4096));
    }

    #[test]
    fn typed_values_preserve_non_step_input_and_clamp_safely() {
        assert_eq!(typed_mb(" 1337 "), Some(1337.0));
        assert_eq!(typed_mb("1"), Some(MIN_MB));
        assert_eq!(typed_mb("99999"), Some(MAX_MB));
        assert_eq!(typed_mb("NaN"), None);
        assert_eq!(typed_mb("inf"), None);
        assert_eq!(typed_mb("wrong"), None);
    }
}
