use crate::backend::theme::ThemeManager;

pub fn install(theme: &ThemeManager) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    // Theme-driven values (dark values match the previous hardcoded palette
    // so dark mode looks identical; light mode now follows the theme).
    // Replaces: bg #000000, well #181818, panel #121212, hover #272727,
    // border #242424, text #ECEEF3, secondary #A3A9B7.
    let accent = theme.accent_color();
    let bg = theme.bg();
    let panel = theme.panel();
    let well = theme.well();
    let hover = theme.hover();
    let border = theme.border();
    let text_main = theme.text_main();
    let text_sec = theme.text_sec();
    let css = format!(
        ".mcx-page {{ background-color: {bg}; }}\
        .mcx .mc-head {{ background-color: {well}; border: 1px solid {border}; border-radius: 16px; padding: 10px 12px; }}\
        .mcx .mcx-headcard {{ background-color: {well}; border: 1px solid {border}; border-radius: 12px; padding: 16px; }}\
        .mcx .mc-tile {{ background-color: {well}; border: 1px solid {border}; border-radius: 16px; }}\
        .mcx .mc-tile:hover {{ border-color: {accent}; }}\
        .mcx .mc-tile-name {{ color: {text_main}; font-size: 14px; font-weight: 700; }}\
        .mcx .mc-tile-sub {{ color: {text_sec}; font-size: 11px; }}\
        .mcx .time-label {{ color: {text_sec}; }}\
        .mcx .frame-title {{ color: {text_sec}; font-size: 11px; font-weight: 800; letter-spacing: 0.8px; text-transform: uppercase; }}\
        .mcx .add-btn {{ background-color: {accent}; color: #FFFFFF; border: none; border-radius: 10px; font-size: 14px; font-weight: 800; }}\
        .mcx .mcx-bar {{ border: none; background: none; padding: 0; }}\
        .mcx .mcx-bar > button, .mcx .mcx-bar > entry, .mcx .mcx-bar > dropdown, .mcx .mcx-bar > menubutton {{ min-height: 40px; padding-top: 0; padding-bottom: 0; border-radius: 10px; font-size: 14px; }}\
        .mcx .mcx-bar > button {{ min-width: 40px; padding-left: 12px; padding-right: 12px; }}\
        .mcx .mcx-bar entry {{ background-color: {well}; border: 1.5px solid {accent}; color: {text_main}; min-width: 120px; }}\
        .mcx .mcx-bar entry placeholder {{ color: {text_sec}; }}\
        .mcx .mcx-bar dropdown {{ min-width: 96px; background-color: {well}; border: none; }}\
        .mcx .mcx-bar dropdown > button {{ background-color: {well}; border: 1.5px solid {accent}; color: {text_main}; }}\
        .mcx .mcx-panel {{ background-color: {panel}; border: 1px solid {border}; border-radius: 16px; padding: 16px; }}\
        .mcx .mc-row {{ background-color: {well}; border: 1px solid {border}; border-radius: 12px; }}\
        .mcx .mc-row:hover {{ border-color: {accent}; }}\
        .mcx .mc-empty {{ background-color: {well}; border: 1px dashed {border}; border-radius: 16px; }}\
        .mcx .skeleton-tile {{ background-color: {well}; border: 1px solid {border}; }}\
        .mcx .store-row {{ background-color: {well}; border: 1px solid {border}; }}\
        .mcx .settings-tab {{ min-height: 40px; }}\
        .mcx switch:checked {{ background-color: {accent}; border-color: {accent}; }}\
        .mcx switch:checked > slider {{ background-color: #FFFFFF; }}\
        .mcx check:checked {{ background-color: {accent}; border-color: {accent}; color: #FFFFFF; -gtk-icon-source: -gtk-icontheme(\"object-select-symbolic\"); }}\
        .mcx radio:checked {{ border-color: {accent}; }}\
        .mcx radio:checked > indicator {{ background-color: {accent}; }}\
        .mcx checkbutton.radio > check:checked, .mcx checkbutton > check.radio:checked {{ background-color: {accent}; border-color: {accent}; }}\
        .mcx-modal check:checked {{ background-color: {accent}; border-color: {accent}; color: #FFFFFF; -gtk-icon-source: -gtk-icontheme(\"object-select-symbolic\"); }}\
        .mcx-modal checkbutton.radio > check:checked {{ background-color: {accent}; border-color: {accent}; color: #FFFFFF; }}\
        .mcx-modal radio:checked {{ background-color: {accent}; border-color: {accent}; color: #FFFFFF; }}\
        .mcx-modal radio:checked > indicator {{ background-color: {accent}; }}\
        .mcx .mcx-origin {{ background-color: {hover}; border: 1px solid {border}; border-radius: 8px; padding: 2px 8px; color: {text_main}; font-size: 12px; }}\
        .mcx-modal .mcx-dot-ok, .mcx-modal .mcx-dot-missing {{ min-width: 10px; min-height: 10px; padding: 0; margin: 0; border-radius: 999px; }}\
        .mcx-modal .mcx-dot-ok {{ background-color: #3FB950; }}\
        .mcx-modal .mcx-dot-missing {{ background-color: #E5484D; }}\
        .mcx scale highlight {{ background-color: {accent}; }}\
        .mcx scale slider {{ background-color: {accent}; border-color: {accent}; }}\
        .mcx .mcx-title {{ color: {text_main}; font-size: 20px; font-weight: 700; padding: 0; }}\
        .mcx .mcx-chip {{ background-color: {well}; border: 1px solid {border}; border-radius: 8px; padding: 2px 8px; min-height: 28px; }}\
        .mcx .mcx-chip-label {{ color: {text_sec}; font-size: 10px; }}\
        .mcx .mcx-chip-value {{ color: {text_main}; font-size: 12px; font-weight: 600; }}\
        .mcx .mcx-fav-star {{ color: #FFD700; }}\
        .mcx .icon-pencil {{ opacity: 0; }}\
        .mcx .icon-wrap:hover .icon-pencil {{ opacity: 1; }}\
        .mcx flowboxchild {{ padding: 0; }}\
        .mcx-modal {{ background-color: {well}; border-radius: 16px; border: 1px solid {border}; }}\
        .mcx .mcx-mhead {{ min-height: 56px; }}\
        .mcx .modal-title {{ font-size: 20px; font-weight: 800; }}\
        .mcx .close-btn:hover {{ background-color: {hover}; opacity: 1; }}\
        .mcx separator {{ background-color: {border}; min-height: 1px; }}\
        .mcx .mcx-section {{ color: {text_main}; font-size: 16px; font-weight: 700; }}\
        .mcx .mcx-section-sm {{ color: {text_main}; font-size: 14px; font-weight: 700; }}\
        .mcx .mcx-acc-row {{ background-color: {well}; border: 1px solid color-mix(in srgb, {accent} 35%, transparent); border-radius: 12px; padding: 8px 12px; }}\
        .mcx .mcx-seg-wrap {{ border: 1.5px solid {accent}; border-radius: 12px; padding: 4px; background-color: transparent; }}\
        .mcx .mcx-seg-tab {{ background-color: transparent; background-image: none; border: none; box-shadow: none; border-radius: 8px; padding: 8px 16px; color: {text_sec}; font-size: 13px; font-weight: 700; }}\
        .mcx .mcx-seg-tab:hover {{ color: {text_main}; }}\
        .mcx .mcx-seg-tab:checked {{ background-color: {accent}; color: #FFFFFF; }}\
        .mcx-modal menubutton.mcx-add-account {{ background-color: transparent; background-image: none; border: none; box-shadow: none; padding: 0; margin: 8px 0 0 0; min-width: 0; min-height: 0; }}\
        .mcx-modal menubutton.mcx-add-account > button {{ background-color: {hover}; border: 1px solid {border}; border-radius: 10px; color: {text_main}; font-size: 14px; font-weight: 700; min-height: 40px; padding: 0 16px; }}\
        .mcx-modal menubutton.mcx-add-account > button:hover {{ border-color: {accent}; }}\
        .mcx .java-row {{ border-radius: 10px; border: 1px solid transparent; border-left-width: 3px; padding: 0 12px; }}\
        .mcx checkbutton:checked .java-row {{ background-color: color-mix(in srgb, {accent} 12%, transparent); border-color: {accent}; }}\
        .mcx .java-row-title {{ color: {text_main}; font-size: 14px; font-weight: 600; }}\
        .mcx .java-row-sub {{ color: {text_sec}; font-size: 12px; }}\
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
