//! Nonmodal, widget-relative hints using the launcher's existing GTK controls.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;

use crate::backend::config::ConfigManager;
use crate::backend::tutorial::{self as model, Step, Tour};

type Target = Option<glib::WeakRef<gtk::Widget>>;
type Deferred = Option<Box<dyn FnOnce()>>;

struct Watch {
    widget: glib::WeakRef<gtk::Widget>,
    handlers: Vec<glib::SignalHandlerId>,
}

struct Inner {
    window: glib::WeakRef<adw::ApplicationWindow>,
    config: ConfigManager,
    tour: RefCell<Tour>,
    targets: [Target; 6],
    pending: Cell<Option<bool>>,
    popover: RefCell<Option<gtk::Popover>>,
    watch: RefCell<Option<Watch>>,
    previous_focus: RefCell<Target>,
    revision: Cell<u64>,
    deferred: RefCell<Deferred>,
}

#[derive(Clone)]
pub struct Controller(Rc<Inner>);

impl Controller {
    pub fn is_active(&self) -> bool {
        self.0.tour.borrow().active()
    }

    pub fn attach(
        window: &adw::ApplicationWindow,
        config: &ConfigManager,
        anchors: Vec<(Step, gtk::Widget)>,
    ) -> Self {
        let tour = Tour::new(config.launcher_value(model::KEY).as_deref());
        let pending = tour.should_auto_start().then_some(false);
        let mut targets = std::array::from_fn(|_| None);
        for (step, target) in anchors {
            targets[step.index()] = Some(target.downgrade());
        }
        let controller = Self(Rc::new(Inner {
            window: window.downgrade(),
            config: config.clone(),
            tour: RefCell::new(tour),
            targets,
            pending: Cell::new(pending),
            popover: RefCell::new(None),
            watch: RefCell::new(None),
            previous_focus: RefCell::new(None),
            revision: Cell::new(0),
            deferred: RefCell::new(None),
        }));
        let weak = Rc::downgrade(&controller.0);
        window.connect_map(move |_| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.try_start();
                }
            });
        });
        let weak = Rc::downgrade(&controller.0);
        window.connect_visible_notify(move |window| {
            if let Some(inner) = weak.upgrade() {
                if !window.is_visible() {
                    inner.dismiss();
                } else {
                    inner.try_start();
                }
            }
        });
        let weak = Rc::downgrade(&controller.0);
        window.connect_visible_dialog_notify(move |window| {
            if let Some(inner) = weak.upgrade() {
                if window.visible_dialog().is_some() && inner.tour.borrow().active() {
                    inner.dismiss();
                } else {
                    inner.try_start();
                }
            }
        });
        window.add_controller(escape_controller(&controller.0));
        let action = gio::SimpleAction::new(model::ACTION, None);
        let inner = controller.0.clone();
        action.connect_activate(move |_, _| {
            inner.pending.set(Some(true));
            inner.try_start();
        });
        window.add_action(&action);
        controller
    }

    pub fn when_inactive(&self, callback: impl FnOnce() + 'static) {
        *self.0.deferred.borrow_mut() = Some(Box::new(callback));
        self.0.flush_deferred();
    }
}

fn escape_controller(inner: &Rc<Inner>) -> gtk::EventControllerKey {
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(inner);
    controller.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            if let Some(inner) = weak.upgrade() {
                if inner.tour.borrow().active() {
                    inner.dismiss();
                    return glib::Propagation::Stop;
                }
            }
        }
        glib::Propagation::Proceed
    });
    controller
}

impl Inner {
    fn save(&self) {
        if model::save_state(&self.config, self.tour.borrow().status()).is_err() {
            eprintln!("[CorkyTux] Could not save tutorial state");
        }
    }

    fn ready(&self) -> bool {
        self.window.upgrade().is_some_and(|window| {
            window.is_visible() && window.is_mapped() && window.visible_dialog().is_none()
        })
    }

    fn flush_deferred(&self) {
        if self.pending.get().is_some() || self.tour.borrow().active() || !self.ready() {
            return;
        }
        let callback = self.deferred.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
    }

    fn try_start(self: &Rc<Self>) {
        if !self.ready() {
            return;
        }
        let Some(manual) = self.pending.take() else {
            self.flush_deferred();
            return;
        };
        let was_active = self.tour.borrow().active();
        if !self.tour.borrow_mut().start(manual) {
            self.flush_deferred();
            return;
        }
        if !was_active {
            if let Some(window) = self.window.upgrade() {
                *self.previous_focus.borrow_mut() =
                    gtk::prelude::GtkWindowExt::focus(&window).map(|widget| widget.downgrade());
            }
        }
        // Persist before showing: termination cannot cause a repeated first-run tour.
        self.save();
        self.show_current();
    }

    fn available(&self) -> [bool; 6] {
        std::array::from_fn(|index| {
            self.targets[index]
                .as_ref()
                .and_then(|target| target.upgrade())
                .is_some_and(|target| {
                    target.is_visible() && target.is_mapped() && target.is_sensitive()
                })
        })
    }

    fn teardown(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
        if let Some(watch) = self.watch.borrow_mut().take() {
            if let Some(widget) = watch.widget.upgrade() {
                for handler in watch.handlers {
                    widget.disconnect(handler);
                }
            }
        }
        let popover = self.popover.borrow_mut().take();
        if let Some(popover) = popover {
            popover.popdown();
            popover.unparent();
        }
    }

    fn restore_focus(&self) {
        let target = self
            .previous_focus
            .borrow_mut()
            .take()
            .and_then(|target| target.upgrade());
        if self.ready() {
            if let Some(target) =
                target.filter(|target| target.is_mapped() && target.is_sensitive())
            {
                let _ = target.grab_focus();
            }
        }
    }

    fn dismiss(self: &Rc<Self>) {
        if !self.tour.borrow().active() && self.pending.get().is_none() {
            return;
        }
        self.pending.set(None);
        self.tour.borrow_mut().dismiss();
        self.save();
        self.teardown();
        self.restore_focus();
        self.flush_deferred();
    }

    fn advance(self: &Rc<Self>) {
        if !self.ready() {
            self.dismiss();
            return;
        }
        self.tour.borrow_mut().next(&self.available());
        self.show_current();
    }

    fn show_current(self: &Rc<Self>) {
        self.teardown();
        let mut available = self.available();
        let (step, anchor) = loop {
            let step = self.tour.borrow_mut().current(&available);
            let Some(step) = step else {
                self.save();
                self.restore_focus();
                self.flush_deferred();
                return;
            };
            if let Some(anchor) = self.targets[step.index()]
                .as_ref()
                .and_then(|target| target.upgrade())
            {
                break (step, anchor);
            }
            available[step.index()] = false;
        };
        let revision = self.revision.get();
        let popover = gtk::Popover::new();
        popover.set_autohide(false);
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_has_arrow(true);
        popover.set_parent(&anchor);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_top(12);
        body.set_margin_bottom(12);
        body.set_margin_start(12);
        body.set_margin_end(12);
        let progress = gtk::Label::new(Some(&format!("{} / 6", step.index() + 1)));
        progress.add_css_class("time-label");
        progress.set_halign(gtk::Align::Start);
        body.append(&progress);
        let title = gtk::Label::new(Some(step.title()));
        title.add_css_class("info-value");
        title.set_halign(gtk::Align::Start);
        body.append(&title);
        let text = gtk::Label::new(Some(step.body()));
        text.set_wrap(true);
        text.set_max_width_chars(36);
        text.set_width_chars(28);
        text.set_xalign(0.0);
        body.append(&text);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let skip = gtk::Button::with_label("Skip");
        skip.add_css_class("settings-btn");
        let more = available
            .iter()
            .skip(step.index() + 1)
            .any(|present| *present);
        let next = gtk::Button::with_label(if more { "Next" } else { "Done" });
        next.add_css_class("suggested-action");
        row.append(&skip);
        row.append(&next);
        body.append(&row);
        popover.set_child(Some(&body));
        let weak = Rc::downgrade(self);
        skip.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.dismiss();
            }
        });
        let weak = Rc::downgrade(self);
        next.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.advance();
            }
        });
        let weak = Rc::downgrade(self);
        popover.connect_closed(move |_| {
            if let Some(inner) = weak.upgrade() {
                if inner.revision.get() == revision {
                    inner.dismiss();
                }
            }
        });
        let weak = Rc::downgrade(self);
        let unmap = anchor.connect_unmap(move |_| {
            if let Some(inner) = weak.upgrade() {
                if inner.revision.get() == revision {
                    inner.advance();
                }
            }
        });
        let weak = Rc::downgrade(self);
        let sensitive = anchor.connect_sensitive_notify(move |target| {
            if !target.is_sensitive() {
                if let Some(inner) = weak.upgrade() {
                    if inner.revision.get() == revision {
                        inner.advance();
                    }
                }
            }
        });
        *self.watch.borrow_mut() = Some(Watch {
            widget: anchor.downgrade(),
            handlers: vec![unmap, sensitive],
        });
        popover.add_controller(escape_controller(self));
        *self.popover.borrow_mut() = Some(popover.clone());
        popover.popup();
        let _ = next.grab_focus();
    }
}
