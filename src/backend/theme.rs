use glib::subclass::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Dark
    }
}

impl ThemeMode {
    pub fn as_str(&self) -> &str {
        match self {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" => ThemeMode::Light,
            _ => ThemeMode::Dark,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AccentColor {
    pub id: u32,
    pub name: String,
    pub hex: String,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub hover_hex: String,
    pub hr: f64,
    pub hg: f64,
    pub hb: f64,
    pub pressed_hex: String,
    pub pr: f64,
    pub pg: f64,
    pub pb: f64,
}

fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    (
        ((v >> 16) & 0xFF) as f64 / 255.0,
        ((v >> 8) & 0xFF) as f64 / 255.0,
        (v & 0xFF) as f64 / 255.0,
    )
}

fn make_accent(id: u32, name: &str, primary: &str, hover: &str, pressed: &str) -> AccentColor {
    let (r, g, b) = hex_to_rgb(primary);
    let (hr, hg, hb) = hex_to_rgb(hover);
    let (pr, pg, pb) = hex_to_rgb(pressed);
    AccentColor {
        id,
        name: name.into(),
        hex: primary.into(),
        r, g, b,
        hover_hex: hover.into(),
        hr, hg, hb,
        pressed_hex: pressed.into(),
        pr, pg, pb,
    }
}

// Exact C++ ACCENTS table (primary / hover / pressed per accent)
pub fn all_accents() -> Vec<AccentColor> {
    vec![
        make_accent(0, "green", "#1db954", "#1ed760", "#1aa34a"),
        make_accent(1, "blue", "#1E88E5", "#42A5F5", "#1565C0"),
        make_accent(2, "cyan", "#00BCD4", "#26C6DA", "#0097A7"),
        make_accent(3, "purple", "#AB47BC", "#CE93D8", "#8E24AA"),
        make_accent(4, "pink", "#EC407A", "#F48FB1", "#D81B60"),
        make_accent(5, "red", "#EF5350", "#EF9A9A", "#E53935"),
        make_accent(6, "orange", "#FFA726", "#FFCC80", "#FB8C00"),
        make_accent(7, "yellow", "#FFEE58", "#FFF176", "#FDD835"),
        make_accent(8, "teal", "#26A69A", "#80CBC4", "#00897B"),
        make_accent(9, "indigo", "#5C6BC0", "#9FA8DA", "#3949AB"),
    ]
}

/// C++ ThemeManager::darken parity: channel * (1 - amount), clamped
pub fn darken(hex: &str, amount: f64) -> String {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    let ch = |c: u32| ((c as f64 * (1.0 - amount)).clamp(0.0, 255.0)) as u32;
    format!(
        "#{:02x}{:02x}{:02x}",
        ch((v >> 16) & 0xFF),
        ch((v >> 8) & 0xFF),
        ch(v & 0xFF)
    )
}

mod imp {
    use super::*;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct ThemeManager {
        pub theme: RefCell<ThemeMode>,
        pub accent_id: RefCell<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ThemeManager {
        const NAME: &'static str = "CorkyTuxThemeManager";
        type Type = super::ThemeManager;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for ThemeManager {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().load_from_config();
        }
    }
}

glib::wrapper! {
    pub struct ThemeManager(ObjectSubclass<imp::ThemeManager>);
}

impl ThemeManager {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn load_from_config(&self) {
        let config = super::ConfigManager::new();
        if let Some(val) = config.launcher_value("Theme") {
            *self.imp().theme.borrow_mut() = ThemeMode::from_str(&val);
        }
        if let Some(val) = config.launcher_value("Accent") {
            if let Ok(id) = val.parse::<u32>() {
                *self.imp().accent_id.borrow_mut() = id;
            }
        }
    }

    fn save_to_config(&self) {
        let config = super::ConfigManager::new();
        config.set_launcher_value("Theme", self.theme().as_str());
        config.set_launcher_value("Accent", &self.accent_id().to_string());
    }

    pub fn theme(&self) -> ThemeMode {
        self.imp().theme.borrow().clone()
    }

    pub fn is_dark(&self) -> bool {
        self.theme() == ThemeMode::Dark
    }

    pub fn set_theme(&self, mode: ThemeMode) {
        *self.imp().theme.borrow_mut() = mode;
        self.save_to_config();
    }

    pub fn accent_id(&self) -> u32 {
        *self.imp().accent_id.borrow()
    }

    pub fn set_accent_id(&self, id: u32) {
        let clamped = id.min(9);
        *self.imp().accent_id.borrow_mut() = clamped;
        self.save_to_config();
    }

    pub fn accent(&self) -> AccentColor {
        let accents = all_accents();
        let idx = (self.accent_id() as usize).min(accents.len() - 1);
        accents[idx].clone()
    }

    pub fn bg(&self) -> &str {
        if self.is_dark() { "#000000" } else { "#F8F9FA" }
    }

    pub fn panel(&self) -> &str {
        if self.is_dark() { "#121212" } else { "#FFFFFF" }
    }

    pub fn card(&self) -> &str {
        if self.is_dark() { "#181818" } else { "#FFFFFF" }
    }

    pub fn well(&self) -> &str {
        if self.is_dark() { "#181818" } else { "#F1F3F5" }
    }

    pub fn hover(&self) -> &str {
        if self.is_dark() { "#282828" } else { "#E9ECEF" }
    }

    pub fn border(&self) -> &str {
        if self.is_dark() { "#282828" } else { "#DEE2E6" }
    }

    pub fn text_main(&self) -> &str {
        if self.is_dark() { "#E0E0E0" } else { "#212529" }
    }

    pub fn text_sec(&self) -> &str {
        if self.is_dark() { "#AAAAAA" } else { "#495057" }
    }

    pub fn text_muted(&self) -> &str {
        if self.is_dark() { "#777777" } else { "#6C757D" }
    }

    pub fn accent_color(&self) -> String {
        let hex = self.accent().hex.clone();
        if self.is_dark() { hex } else { darken(&hex, 0.22) }
    }

    pub fn accent_hover(&self) -> String {
        let hex = self.accent().hover_hex.clone();
        if self.is_dark() {
            hex
        } else {
            darken(&hex, 0.22)
        }
    }

    pub fn accent_pressed(&self) -> String {
        let hex = self.accent().pressed_hex.clone();
        if self.is_dark() {
            hex
        } else {
            darken(&hex, 0.22)
        }
    }

    pub fn accent_text(&self) -> String {
        self.accent().hex.clone()
    }

    pub fn accent_rgb(&self) -> String {
        let a = self.accent();
        format!("{},{},{}", (a.r * 255.0) as u32, (a.g * 255.0) as u32, (a.b * 255.0) as u32)
    }

    pub fn accent_strip(&self) -> String {
        let a = self.accent();
        let luminance = 0.299 * a.r + 0.587 * a.g + 0.114 * a.b;
        let alpha = if self.is_dark() {
            if luminance > 0.5 { 0.25 } else { 0.35 }
        } else {
            if luminance > 0.5 { 0.15 } else { 0.25 }
        };
        format!(
            "rgba({},{},{},{})",
            (a.r * 255.0) as u32,
            (a.g * 255.0) as u32,
            (a.b * 255.0) as u32,
            alpha
        )
    }

    pub fn success(&self) -> &str {
        if self.is_dark() { "#1db954" } else { "#198754" }
    }

    pub fn danger(&self) -> &str {
        if self.is_dark() { "#FF0040" } else { "#DC3545" }
    }
}
