use crate::backend::theme::ThemeManager;

pub fn install(theme: &ThemeManager) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let accent = theme.accent_color();
    let css = format!(
        ".mcx {{ background-color: #000000; }}\
        .mcx .mc-head {{ background-color: #181818; border: 1px solid #242424; border-radius: 16px; padding: 10px 12px; }}\
        .mcx .mc-tile {{ background-color: #181818; border: 1px solid #242424; border-radius: 16px; }}\
        .mcx .mc-tile:hover {{ border-color: {accent}; }}\
        .mcx .mc-tile-name {{ color: #ECEEF3; font-size: 14px; font-weight: 700; }}\
        .mcx .mc-tile-sub {{ color: #A3A9B7; font-size: 11px; }}\
        .mcx .time-label {{ color: #A3A9B7; }}\
        .mcx .frame-title {{ color: #A3A9B7; font-size: 11px; font-weight: 800; letter-spacing: 0.8px; text-transform: uppercase; }}\
        .mcx .add-btn {{ background-color: {accent}; color: #FFFFFF; border: none; border-radius: 999px; font-weight: 800; }}\
        .mcx .settings-btn {{ background-color: #272727; color: #ECEEF3; border: 1px solid #242424; border-radius: 999px; font-weight: 700; }}\
        .mcx .settings-btn:hover {{ border-color: {accent}; }}\
        .mcx .mc-row {{ background-color: #181818; border: 1px solid #242424; border-radius: 12px; }}\
        .mcx .mc-row:hover {{ border-color: {accent}; }}\
        .mcx .mc-empty {{ background-color: #181818; border: 1px dashed #242424; border-radius: 16px; }}\
        .mcx .skeleton-tile {{ background-color: #181818; border: 1px solid #242424; }}\
        .mcx .store-row {{ background-color: #181818; border: 1px solid #242424; }}\
        .mcx .search-entry {{ background-color: #272727; border: 1px solid #242424; border-radius: 999px; color: #ECEEF3; }}\
        .mcx .mcx-bar button, .mcx .mcx-bar entry, .mcx .mcx-bar dropdown {{ min-height: 40px; }}\
        .mcx .mcx-bar entry {{ min-width: 160px; }}\
        .mcx .settings-tab {{ min-height: 40px; }}"
    );
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
