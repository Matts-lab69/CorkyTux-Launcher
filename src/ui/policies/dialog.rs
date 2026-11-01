//! Dialog size policy: a dialog never grows past 90% of its parent and
//! never shrinks below the usable minimum, unless the parent is tiny.

use adw::prelude::*;
use gtk::prelude::*;

const DIALOG_MIN_W: i32 = 320;
const DIALOG_MIN_H: i32 = 240;

/// Fits a dialog into 90% of its parent window: never larger than the size
/// it asked for, never smaller than a usable minimum while the parent has
/// room. A parent of 0 means unknown, so the request passes through.
pub fn dialog_size(parent_w: i32, parent_h: i32, want_w: i32, want_h: i32) -> (i32, i32) {
    if parent_w <= 0 || parent_h <= 0 {
        return (want_w, want_h);
    }
    let fit = |want: i32, parent: i32, min: i32| {
        let cap = (parent as f64 * 0.9).round() as i32;
        want.min(cap).max(min.min(cap))
    };
    (fit(want_w, parent_w, DIALOG_MIN_W), fit(want_h, parent_h, DIALOG_MIN_H))
}

/// Applies `dialog_size` to an AdwDialog against the active window.
pub fn fit_dialog(dialog: &adw::Dialog, want_w: i32, want_h: i32) {
    let (parent_w, parent_h) = gtk::Window::list_toplevels()
        .first()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((0, 0));
    let (w, h) = dialog_size(parent_w, parent_h, want_w, want_h);
    dialog.set_width_request(w);
    dialog.set_height_request(h);
    dialog.set_content_width(w);
    dialog.set_content_height(h);
}

#[cfg(test)]
mod tests {
    use super::dialog_size;

    #[test]
    fn dialog_size_fits_the_parent() {
        // Unknown parent: the request passes through.
        assert_eq!(dialog_size(0, 0, 520, 380), (520, 380));
        // Room enough: unchanged.
        assert_eq!(dialog_size(1920, 1080, 520, 380), (520, 380));
        // 90% of the parent wins over the request.
        assert_eq!(dialog_size(640, 480, 640, 480), (576, 432));
        assert_eq!(dialog_size(500, 400, 520, 380), (450, 360));
        // Tiny parent: the cap wins over the minimum.
        assert_eq!(dialog_size(200, 200, 520, 380), (180, 180));
        // The usable minimum wins over a small request.
        assert_eq!(dialog_size(1000, 1000, 200, 150), (320, 240));
    }
}
