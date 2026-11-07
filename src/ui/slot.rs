//! Rebuild slots shared by the Settings and Stores pages.
//!
//! A slot holds the closure that rebuilds a list. That closure usually has to
//! rebuild another list too, and sometimes its own, so storing a strong `Rc` to
//! the slot inside the closure closes a loop: slot -> closure -> slot. Neither
//! side is ever dropped, and the widgets the closure rebuilt stay alive with
//! them. Closures keep a [`WeakSlot`] instead and upgrade it when they fire.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Zero-argument rebuild slot.
pub type Slot = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// Weak handle to a [`Slot`], safe to capture inside that slot's own closure.
pub type WeakSlot = Weak<RefCell<Option<Rc<dyn Fn()>>>>;

/// Run the closure currently stored in `slot`, if the slot is still alive.
///
/// A dead slot is a no-op: a stale widget must not resurrect the page.
pub fn run(slot: &WeakSlot) {
    if let Some(rc) = slot.upgrade() {
        if let Some(f) = rc.borrow().clone() {
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct DropFlag(Rc<Cell<bool>>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[test]
    fn a_closure_holding_only_a_weak_handle_is_freed_with_its_slot() {
        let dropped = Rc::new(Cell::new(false));
        let flag = DropFlag(dropped.clone());
        let fired = Rc::new(Cell::new(0));
        let fired_c = fired.clone();
        let slot: Slot = Rc::new(RefCell::new(None));
        let weak = Rc::downgrade(&slot);
        *slot.borrow_mut() = Some(Rc::new(move || {
            let _keep = &flag;
            assert!(weak.upgrade().is_some(), "the slot is alive while it runs");
            fired_c.set(fired_c.get() + 1);
        }));

        // The slot owns the closure; the closure owns no strong handle back.
        assert_eq!(Rc::strong_count(&slot), 1);
        run(&Rc::downgrade(&slot));
        assert_eq!(fired.get(), 1, "the stored closure still runs");

        drop(slot);
        assert!(
            dropped.get(),
            "the closure must be dropped together with its slot"
        );
    }

    #[test]
    fn running_a_dead_slot_is_a_no_op() {
        let slot: Slot = Rc::new(RefCell::new(Some(Rc::new(|| panic!("must not run")))));
        let weak = Rc::downgrade(&slot);
        drop(slot);
        run(&weak);
    }

    #[test]
    fn a_forward_slot_is_reached_through_its_weak_handle() {
        let forward: Slot = Rc::new(RefCell::new(None));
        let hits = Rc::new(Cell::new(0));
        let hits_c = hits.clone();
        *forward.borrow_mut() = Some(Rc::new(move || hits_c.set(hits_c.get() + 1)));

        let forward_w = Rc::downgrade(&forward);
        let owner: Slot = Rc::new(RefCell::new(None));
        *owner.borrow_mut() = Some(Rc::new(move || run(&forward_w)));
        run(&Rc::downgrade(&owner));

        assert_eq!(hits.get(), 1);
    }
}
