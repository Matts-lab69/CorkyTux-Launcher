use crate::backend::theme::ThemeManager;

pub fn install(theme: &ThemeManager) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let accent = theme.accent_color();
    let css = format!(
        ".mcx-page {{ background-color: #000000; }}\
        .mcx .mc-head {{ background-color: #181818; border: 1px solid #242424; border-radius: 16px; padding: 10px 12px; }}\
        .mcx .mcx-headcard {{ background-color: #181818; border: 1px solid #242424; border-radius: 12px; padding: 16px; }}\
        .mcx .mc-tile {{ background-color: #181818; border: 1px solid #242424; border-radius: 16px; }}\
        .mcx .mc-tile:hover {{ border-color: {accent}; }}\
        .mcx .mc-tile-name {{ color: #ECEEF3; font-size: 14px; font-weight: 700; }}\
        .mcx .mc-tile-sub {{ color: #A3A9B7; font-size: 11px; }}\
        .mcx .time-label {{ color: #A3A9B7; }}\
        .mcx .frame-title {{ color: #A3A9B7; font-size: 11px; font-weight: 800; letter-spacing: 0.8px; text-transform: uppercase; }}\
        .mcx .add-btn {{ background-color: #AA47BC; color: #FFFFFF; border: none; border-radius: 10px; font-size: 14px; font-weight: 800; }}\
        .mcx .mcx-bar {{ border: none; background: none; padding: 0; }}\
        .mcx .mcx-bar > button, .mcx .mcx-bar > entry, .mcx .mcx-bar > dropdown, .mcx .mcx-bar > menubutton {{ min-height: 40px; padding-top: 0; padding-bottom: 0; border-radius: 10px; font-size: 14px; }}\
        .mcx .mcx-bar > button {{ min-width: 40px; padding-left: 12px; padding-right: 12px; }}\
        .mcx .mcx-bar entry {{ background-color: #181818; border: 1.5px solid {accent}; color: #ECEEF3; min-width: 120px; }}\
        .mcx .mcx-bar entry placeholder {{ color: #A3A9B7; }}\
        .mcx .mcx-bar dropdown {{ min-width: 96px; background-color: #181818; border: none; }}\
        .mcx .mcx-bar dropdown > button {{ background-color: #181818; border: 1.5px solid {accent}; color: #ECEEF3; }}\
        .mcx .mcx-panel {{ background-color: #121212; border: 1px solid #242424; border-radius: 16px; padding: 16px; }}\
        .mcx .mc-row {{ background-color: #181818; border: 1px solid #242424; border-radius: 12px; }}\
        .mcx .mc-row:hover {{ border-color: {accent}; }}\
        .mcx .mc-empty {{ background-color: #181818; border: 1px dashed #242424; border-radius: 16px; }}\
        .mcx .skeleton-tile {{ background-color: #181818; border: 1px solid #242424; }}\
        .mcx .store-row {{ background-color: #181818; border: 1px solid #242424; }}\
        .mcx .settings-tab {{ min-height: 40px; }}\
        .mcx switch:checked {{ background-color: {accent}; border-color: {accent}; }}\
        .mcx switch:checked > slider {{ background-color: #FFFFFF; }}\
        .mcx check:checked {{ background-color: {accent}; border-color: {accent}; color: #FFFFFF; -gtk-icon-source: -gtk-icontheme(\"object-select-symbolic\"); }}\
        .mcx radio:checked {{ border-color: {accent}; }}\
        .mcx radio:checked > indicator {{ background-color: {accent}; }}\
        .mcx checkbutton.radio > check:checked, .mcx checkbutton > check.radio:checked {{ background-color: {accent}; border-color: {accent}; }}\
        .mcx-modal check:checked {{ background-color: #AA47BC; border-color: #AA47BC; color: #FFFFFF; -gtk-icon-source: -gtk-icontheme(\"object-select-symbolic\"); }}\
        .mcx-modal checkbutton.radio > check:checked {{ background-color: #AA47BC; border-color: #AA47BC; color: #FFFFFF; }}\
        .mcx-modal radio:checked {{ background-color: #AA47BC; border-color: #AA47BC; color: #FFFFFF; }}\
        .mcx-modal radio:checked > indicator {{ background-color: #AA47BC; }}\
        .mcx .mcx-origin {{ background-color: #272727; border: 1px solid #242424; border-radius: 8px; padding: 2px 8px; color: #ECEEF3; font-size: 12px; }}\
        .mcx-modal .mcx-dot-ok, .mcx-modal .mcx-dot-missing {{ min-width: 10px; min-height: 10px; padding: 0; margin: 0; border-radius: 999px; }}\
        .mcx-modal .mcx-dot-ok {{ background-color: #3FB950; }}\
        .mcx-modal .mcx-dot-missing {{ background-color: #E5484D; }}\
        .mcx scale highlight {{ background-color: {accent}; }}\
        .mcx scale slider {{ background-color: {accent}; border-color: {accent}; }}\
        .mcx .mcx-title {{ color: #ECEEF3; font-size: 20px; font-weight: 700; padding: 0; }}\
        .mcx .mcx-chip {{ background-color: #181818; border: 1px solid #242424; border-radius: 8px; padding: 2px 8px; min-height: 28px; }}\
        .mcx .mcx-chip-label {{ color: #A3A9B7; font-size: 10px; }}\
        .mcx .mcx-chip-value {{ color: #ECEEF3; font-size: 12px; font-weight: 600; }}\
        .mcx .mcx-fav-star {{ color: #FFD700; }}\
        .mcx .icon-pencil {{ opacity: 0; }}\
        .mcx .icon-wrap:hover .icon-pencil {{ opacity: 1; }}\
        .mcx flowboxchild {{ padding: 0; }}\
        .mcx-modal {{ background-color: #181818; border-radius: 16px; border: 1px solid #242424; }}\
        .mcx .mcx-mhead {{ min-height: 56px; }}\
        .mcx .modal-title {{ font-size: 20px; font-weight: 800; }}\
        .mcx .close-btn {{ color: #ECEEF3; }}\
        .mcx .close-btn:hover {{ background-color: #272727; opacity: 1; }}\
        .mcx separator {{ background-color: #242424; min-height: 1px; }}\
        .mcx .mcx-section {{ color: #ECEEF3; font-size: 16px; font-weight: 700; }}\
        .mcx .mcx-section-sm {{ color: #ECEEF3; font-size: 14px; font-weight: 700; }}\
        .mcx .mcx-danger {{ color: #E5484D; background-color: transparent; }}\
        .mcx .mcx-danger:hover {{ background-color: #272727; }}\
        .mcx .mcx-acc-row {{ background-color: #181818; border: 1px solid #242424; border-radius: 12px; padding: 8px 12px; }}\
        .mcx .mcx-seg-wrap {{ border: 1.5px solid {accent}; border-radius: 12px; padding: 4px; background-color: transparent; }}\
        .mcx .mcx-seg-tab {{ background-color: transparent; background-image: none; border: none; box-shadow: none; border-radius: 8px; padding: 8px 16px; color: #A3A9B7; font-size: 13px; font-weight: 700; }}\
        .mcx .mcx-seg-tab:hover {{ color: #ECEEF3; }}\
        .mcx .mcx-seg-tab:checked {{ background-color: {accent}; color: #FFFFFF; }}\
        .mcx-modal menubutton.mcx-add-account {{ background-color: transparent; background-image: none; border: none; box-shadow: none; padding: 0; margin: 8px 0 0 0; min-width: 0; min-height: 0; }}\
        .mcx-modal menubutton.mcx-add-account > button {{ background-color: #272727; border: 1px solid #242424; border-radius: 10px; color: #ECEEF3; font-size: 14px; font-weight: 700; min-height: 40px; padding: 0 16px; }}\
        .mcx-modal menubutton.mcx-add-account > button:hover {{ border-color: {accent}; }}\
        .mcx .java-row {{ border-radius: 10px; border: 1px solid transparent; border-left-width: 3px; padding: 0 12px; }}\
        .mcx checkbutton:checked .java-row {{ background-color: color-mix(in srgb, {accent} 12%, transparent); border-color: {accent}; }}\
        .mcx .java-row-title {{ color: #ECEEF3; font-size: 14px; font-weight: 600; }}\
        .mcx .java-row-sub {{ color: #A3A9B7; font-size: 12px; }}\
        .mcx-page scrollbar slider, .mcx-modal scrollbar slider {{ background-color: color-mix(in srgb, {accent} 55%, transparent); border-radius: 9999px; }}\
        .mcx-page scrollbar slider:hover, .mcx-modal scrollbar slider:hover {{ background-color: color-mix(in srgb, {accent} 85%, transparent); }}\
        .mcx-page scrollbar slider:active, .mcx-modal scrollbar slider:active {{ background-color: {accent}; }}\
        .mcx-page scrollbar trough, .mcx-modal scrollbar trough {{ background-color: color-mix(in srgb, {accent} 10%, transparent); border-radius: 9999px; }}"
    );
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
