//! Shared width policy: how many columns a FlowBox fits, and how the details
//! panel coexists with the library at each width.

/// FlowBox columns that fit `width_sp` with 150 px cards and 8 px gaps.
/// The FlowBox already wraps; setting the per-line cap from the real width
/// keeps the last row from stretching over the whole viewport.
pub fn columns_for_width(width_sp: i32) -> u32 {
    const CARD: i32 = 150;
    const GAP: i32 = 8;
    if width_sp <= 0 {
        return 1;
    }
    let columns = (width_sp + GAP) / (CARD + GAP);
    columns.clamp(1, 10) as u32
}

pub enum DetailsMode {
    Library,
    Lateral,
    Overlay,
    Substitutive,
}

/// Pure layout policy for the details panel. Under 900 px the panel replaces
/// the library (with the panel's close button as "back"); from 900 to 1399 it
/// floats over it; from 1400 it sits beside it.
pub fn layout_mode(width: i32, panel_open: bool) -> DetailsMode {
    if !panel_open {
        return DetailsMode::Library;
    }
    if width < 900 {
        DetailsMode::Substitutive
    } else if width < 1400 {
        DetailsMode::Overlay
    } else {
        DetailsMode::Lateral
    }
}


#[cfg(test)]
mod tests {
    use super::{columns_for_width, layout_mode, DetailsMode};

    #[test]
    fn columns_for_width_table() {
        assert_eq!(columns_for_width(0), 1);
        assert_eq!(columns_for_width(-10), 1);
        assert_eq!(columns_for_width(200), 1);
        assert_eq!(columns_for_width(316), 2);
        assert_eq!(columns_for_width(474), 3);
        assert_eq!(columns_for_width(640), 4);
        assert_eq!(columns_for_width(800), 5);
        assert_eq!(columns_for_width(1280), 8);
        assert_eq!(columns_for_width(1920), 10);
        assert_eq!(columns_for_width(3840), 10);
    }

    #[test]
    fn details_layout_modes_and_edges() {
        assert_eq!(layout_mode(640, false), DetailsMode::Library);
        assert_eq!(layout_mode(640, true), DetailsMode::Substitutive);
        assert_eq!(layout_mode(899, true), DetailsMode::Substitutive);
        assert_eq!(layout_mode(900, true), DetailsMode::Overlay);
        assert_eq!(layout_mode(1024, true), DetailsMode::Overlay);
        assert_eq!(layout_mode(1399, true), DetailsMode::Overlay);
        assert_eq!(layout_mode(1400, true), DetailsMode::Lateral);
        assert_eq!(layout_mode(1920, true), DetailsMode::Lateral);
    }
}
