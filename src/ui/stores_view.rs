use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::AppState;
use crate::backend::external::StoreManager;
use crate::backend::import_move::{self, ExecPlan, ImportMode, MoveCandidate, MoveOutcome, Preflight};
use crate::backend::plugin_process::{PluginEvent, ProcessKiller};
use crate::ui::deps_modal::DepId;
use crate::ui::helpers;
use crate::ui::import_manager;

#[derive(Clone)]
struct StoreGame {
    app_id: String,
    title: String,
    /// Género simple de GOG (`category`); vacío si no viene. Epic no lo usa.
    category: String,
    /// Sistemas de GOG (`worksOn` filtrado a trues); vacío = no mostrar.
    systems: Vec<String>,
    version: String,
    installed: bool,
    stale_registry: bool,
    install_path: String,
    executable: String,
    cover: String,
    description: String,
}



pub struct StoresView {
    pub widget: gtk::ScrolledWindow,
    handles: Rc<RefCell<Vec<StorePageHandle>>>,
    deals: Rc<RefCell<std::collections::HashMap<String, DealsState>>>,
}

impl Clone for StoresView {
    fn clone(&self) -> Self {
        Self {
            widget: self.widget.clone(),
            handles: self.handles.clone(),
            deals: self.deals.clone(),
        }
    }
}

impl StoresView {
    /// Entering the page: reload anything unload() dropped.
    pub fn page_shown(&self) {
        for h in self.handles.borrow().iter() {
            h.ensure_loaded();
        }
        // Trigger del modal de dependencias: chequeo async (no frena la
        // apertura), modal si falta algo, nunca si está completo.
        self.chequear_dependencias();
        // (a) lightweight installed-status re-verify on entry: local
        // registry + disk, no network, debounced, applied in place.
        self.schedule_focus_checks();
        if let Some(ds) = self.deals.borrow().get("epic") {
            if ds.list.first_child().is_none() {
                ds.page.set(1);
                ds.lbl.set_text("Loading…");
                if let Some(g) = ds.goto.borrow().as_ref() {
                    g(1);
                }
            }
        }
    }

    /// Trigger del modal de dependencias: pre-chequeo barato en cada entrada
    /// (sin red), chequeo completo + modal solo si el pre-chequeo marca algo.
    ///
    /// Corre UNA vez por entrada a Stores: ni loop ni polling mientras el
    /// usuario está parado en la pantalla. Lo pendiente nunca se cachea, así
    /// que borrar algo afuera y reentrar siempre lo detecta.
    pub fn chequear_dependencias(&self) {
        let vista = self.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Chequeo>();
        std::thread::spawn(move || {
            let pre = precheck_actual();
            if pre.hay_algo() {
                let _ = tx.send(Chequeo::Falta(faltantes_actuales()));
            } else {
                let _ = tx.send(Chequeo::TodoBien);
            }
        });
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Chequeo::TodoBien) => {
                // Todo presente: UI normal. Si algo se había bloqueado y se
                // restauró afuera, hay que cargar lo que el bloqueo frenó.
                for h in vista.handles.borrow().iter() {
                    h.aplicar_faltantes(&Faltantes::default());
                    if !h.bloqueado.get() {
                        h.ensure_loaded();
                    }
                }
                glib::ControlFlow::Break
            }
            Ok(Chequeo::Falta(f)) => {
                for h in vista.handles.borrow().iter() {
                    h.aplicar_faltantes(&f);
                }
                if f.hay_algo() {
                    let filas = f.filas_modal();
                    let helper = login_helper_path().ok();
                    let v = vista.clone();
                    let padre = v.widget.clone();
                    crate::ui::deps_modal::present(
                        &padre,
                        filas,
                        helper,
                        move |salida| match salida {
                            crate::ui::deps_modal::InstallOutcome::TodoOk => {
                                // Re-chequeo de verdad y refresco de lo
                                // desbloqueado tras instalar.
                                v.chequear_dependencias();
                                for h in v.handles.borrow().iter() {
                                    h.refresh_auth(false);
                                    h.refresh_library(true);
                                }
                            }
                            crate::ui::deps_modal::InstallOutcome::Skip => {}
                        },
                    );
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    /// Window focus returned to the launcher: lightweight installed-status
    /// checks on every loaded store page (local, no network, in place).
    /// Pages that were never shown have no tiles to update and are skipped.
    pub fn schedule_focus_checks(&self) {
        for h in self.handles.borrow().iter() {
            if h.loaded.get() {
                h.schedule_status_check();
            }
        }
    }

    /// Leaving the page: drop tiles/rows/covers so no RAM is held
    /// while the store isn't visible. Next visit reloads via page_shown().
    pub fn unload(&self) {
        for h in self.handles.borrow().iter() {
            h.unload();
        }
        if let Some(ds) = self.deals.borrow().get("epic") {
            ds.epoch.set(ds.epoch.get().wrapping_add(1));
            ds.page.set(1);
            while let Some(c) = ds.list.first_child() {
                ds.list.remove(&c);
            }
            ds.lbl.set_text("Loading…");
        }
    }
}

fn note(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_halign(gtk::Align::Start);
    l.set_wrap(true);
    l.set_opacity(0.6);
    l.add_css_class("time-label");
    l
}

/// Diagonal "Reclamado" corner ribbon over a free-promos card when the
/// title is already in the Epic library. The band is a rotated rect
/// anchored near the top-right corner (offset inward so it cuts the
/// corner); excess is clipped by the card's overflow. The DrawingArea is
/// non-targetable so clicks still reach the card button underneath.
fn claimed_overlay(card: gtk::Button, theme: &crate::backend::theme::ThemeManager) -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&card));
    let band = helpers::parse_rgba(theme.success());
    let ink = helpers::parse_rgba(if theme.is_dark() { "#FFFFFF" } else { theme.text_main() });
    let da = gtk::DrawingArea::new();
    da.set_can_target(false);
    da.set_halign(gtk::Align::Fill);
    da.set_valign(gtk::Align::Fill);
    da.set_hexpand(true);
    da.set_vexpand(true);
    da.set_draw_func(move |_, cr, w, h| {
        let thickness = 24.0_f64;
        let off = 38.0_f64;
        let n = std::f64::consts::FRAC_1_SQRT_2;
        cr.save().ok();
        cr.translate(w as f64 - off * n, off * n);
        cr.rotate(std::f64::consts::FRAC_PI_4);
        let span = (w as f64).hypot(h as f64);
        cr.rectangle(-span, -thickness / 2.0, span * 2.0, thickness);
        cr.set_source_rgba(band.red() as f64, band.green() as f64, band.blue() as f64, 1.0);
        cr.fill_preserve().ok();
        cr.set_line_width(1.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.18);
        cr.stroke().ok();
        cr.set_font_size(11.0);
        cr.set_source_rgba(ink.red() as f64, ink.green() as f64, ink.blue() as f64, 1.0);
        if let Ok(te) = cr.text_extents("Reclamado") {
            cr.move_to(
                -te.x_bearing() - te.width() / 2.0,
                -(te.y_bearing() + te.height() / 2.0),
            );
            let _ = cr.show_text("Reclamado");
        }
        cr.restore().ok();
    });
    overlay.add_overlay(&da);
    overlay
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn card(title: &str) -> (gtk::Frame, gtk::Box) {
    let f = gtk::Frame::new(None);
    f.add_css_class("page-card");
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
    inner.set_margin_top(6);
    inner.set_margin_bottom(6);
    inner.set_margin_start(8);
    inner.set_margin_end(8);
    let t = gtk::Label::new(Some(title));
    t.set_halign(gtk::Align::Start);
    t.add_css_class("frame-title");
    inner.append(&t);
    f.set_child(Some(&inner));
    (f, inner)
}

impl StoresView {
    pub fn new(
        state: &AppState,
        parent: &adw::ApplicationWindow,
        sidebar: &Rc<RefCell<Option<crate::ui::sidebar::Sidebar>>>,
        center: &Rc<RefCell<Option<crate::ui::center::CenterHandle>>>,
        details: &Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>>,
    ) -> Self {
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_has_frame(false);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
        col.set_margin_top(12);
        col.set_margin_bottom(12);
        col.set_margin_start(12);
        col.set_margin_end(12);
        col.set_hexpand(true);
        scroll.set_child(Some(&col));

        // header: bloque centrado con ícono grande + título grande. Sin botón
        // manual de instalación: todo el flujo es automático por detección
        // (modal al entrar + "Setup incomplete" por tab).
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        head.set_halign(gtk::Align::Center);
        let hicon = gtk::Image::from_icon_name("corkytux-system-software-install-symbolic");
        hicon.set_pixel_size(64);
        hicon.set_valign(gtk::Align::Center);
        head.append(&hicon);
        let title = gtk::Label::new(None);
        title.set_markup("<span size=\"xx-large\" weight=\"bold\">Stores</span>");
        title.add_css_class("details-title");
        title.set_valign(gtk::Align::Center);
        head.append(&title);
        head.add_css_class("mc-head");
        col.append(&head);
        let handles_slot: Rc<RefCell<Vec<StorePageHandle>>> = Rc::new(RefCell::new(Vec::new()));
        let status = note("Epic + GOG via legendary/gogdl (Heroic pattern). Games install into the native library.");
        status.set_halign(gtk::Align::Center);
        status.set_justify(gtk::Justification::Center);
        col.append(&status);

        // tabs (segmented, same pattern as MC Addons tabs)
        let tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        tabbar.add_css_class("seg-bar");
        let stack = gtk::Stack::new();
        stack.set_vexpand(false);
        let mut btns: Vec<gtk::ToggleButton> = Vec::new();
        for (label, _id, icon) in [("Epic Games", "epic", "epicgames-symbolic"), ("GOG", "gog", "gogdotcom-symbolic")] {
            let btn = gtk::ToggleButton::new();
            btn.add_css_class("seg");
            btn.set_hexpand(true);
            let c = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            c.set_halign(gtk::Align::Center);
            c.set_valign(gtk::Align::Center);
            let im = gtk::Image::from_icon_name(icon);
            im.set_pixel_size(16);
            c.append(&im);
            c.append(&gtk::Label::new(Some(label)));
            btn.set_child(Some(&c));
            if label == "Epic Games" {
                btn.set_active(true);
            }
            tabbar.append(&btn);
            btns.push(btn);
        }
        if btns.len() == 2 {
            btns[1].set_group(Some(&btns[0]));
        }
        let ids = ["epic", "gog"];
        col.append(&tabbar);

        let deals_map: Rc<RefCell<std::collections::HashMap<String, DealsState>>> =
            Rc::new(RefCell::new(std::collections::HashMap::new()));
        let mut handles: Vec<StorePageHandle> = Vec::new();
        for store in ["epic", "gog"] {
            let (page, handle) = Self::store_page(state, parent, sidebar, center, details, store, &deals_map);
            stack.add_named(&page, Some(store));
            handles.push(handle);
        }
        // Lazy: libraries load on first tab show, never at startup (a slow
        // `legendary list` used to pin the main loop at 100% on launch).
        for (i, id) in ids.iter().enumerate() {
            let h = handles.get(i).cloned();
            let tid = id.to_string();
            let st = stack.clone();
            let b0 = btns[i].clone();
            b0.connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name(&tid);
                    if let Some(ref hh) = h {
                        hh.ensure_loaded();
                    }
                }
            });
        }
        col.append(&stack);
        if let Some(first) = handles.first() {
            first.ensure_loaded();
        }
        *handles_slot.borrow_mut() = handles;
        let vista = Self {
            widget: scroll,
            handles: handles_slot.clone(),
            deals: deals_map.clone(),
        };
        // Los botones "Install dependencies" (panel incomplete + fila del
        // login) reabren el modal con un re-chequeo fresco.
        for h in vista.handles.borrow().iter() {
            let v = vista.clone();
            *h.reabrir.borrow_mut() = Some(Rc::new(move || v.chequear_dependencias()));
        }
        vista
    }

    fn store_page(
        state: &AppState,
        parent: &adw::ApplicationWindow,
        sidebar: &Rc<RefCell<Option<crate::ui::sidebar::Sidebar>>>,
        center: &Rc<RefCell<Option<crate::ui::center::CenterHandle>>>,
        details: &Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>>,
        store: &str,
        deals_map: &Rc<RefCell<std::collections::HashMap<String, DealsState>>>,
    ) -> (gtk::Box, StorePageHandle) {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let s = store.to_string();

        // auth card
        let (auth_frame, auth_inner) = card(if store == "epic" { "Epic Games account" } else { "GOG account" });
        let login_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let auth_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let auth_badge = gtk::Label::new(Some("Checking…"));
        auth_badge.set_hexpand(true);
        auth_badge.set_halign(gtk::Align::Start);
        auth_row.append(&auth_badge);
        let login_btn = gtk::Button::with_label("Log in");
        login_btn.add_css_class("settings-btn");
        auth_row.append(&login_btn);
        login_box.append(&auth_row);
        auth_inner.append(&login_box);
        // Sin navegador no hay login, pero la biblioteca sigue visible: fila
        // compacta con botón para reabrir el modal. Oculta por defecto.
        let browser_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let browser_lbl = note("Login needs the login browser.");
        browser_lbl.set_hexpand(true);
        browser_lbl.set_halign(gtk::Align::Start);
        browser_row.append(&browser_lbl);
        let browser_btn = gtk::Button::with_label("Install dependencies");
        browser_btn.add_css_class("settings-btn");
        browser_row.append(&browser_btn);
        browser_row.set_visible(false);
        auth_inner.append(&browser_row);
        // logged-in session row: avatar + name + logout
        let acc_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let acc_avatar = gtk::Label::new(Some(""));
        acc_avatar.add_css_class("account-avatar");
        acc_avatar.set_xalign(0.5);
        acc_avatar.set_yalign(0.5);
        acc_avatar.set_valign(gtk::Align::Center);
        acc_row.append(&acc_avatar);
        let acc_name = gtk::Label::new(Some(""));
        acc_name.set_halign(gtk::Align::Start);
        acc_name.set_hexpand(true);
        acc_name.add_css_class("details-title");
        acc_row.append(&acc_name);
        let logout_btn = gtk::Button::with_label("Log out");
        logout_btn.add_css_class("settings-btn");
        acc_row.append(&logout_btn);
        acc_row.set_visible(false);
        auth_inner.append(&acc_row);
        let auth_hint = note("");
        auth_hint.set_visible(false);
        auth_inner.append(&auth_hint);
        page.append(&auth_frame);

        // free games (Epic only, truly 100% off) + buy section
        if store == "epic" {
            let (promo_frame, promo_inner) = card("Free games now");
            let promo_lbl = note("Loading…");
            promo_inner.append(&promo_lbl);
            // Scrollable list so long catalogs don't stretch the page.
            let promo_list = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let promo_scroll = gtk::ScrolledWindow::new();
            promo_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
            promo_scroll.set_min_content_height(170);
            promo_scroll.set_propagate_natural_height(true);
            promo_scroll.set_vexpand(false);
            promo_scroll.set_child(Some(&promo_list));
            promo_inner.append(&promo_scroll);
            page.append(&promo_frame);
            let (tx, rx) = std::sync::mpsc::channel::<Vec<(String, String, String, String, bool)>>();
            std::thread::spawn(move || {
                let promos = StoreManager::free_promos().ok()
                    .and_then(|d| d.get("free_now").cloned())
                    .and_then(|v| v.as_array().cloned()).unwrap_or_default()
                    .into_iter().filter_map(|p| {
                        Some((p.get("title")?.as_str()?.to_string(),
                              p.get("description")?.as_str().unwrap_or("").to_string(),
                              p.get("cover")?.as_str().unwrap_or("").to_string(),
                              p.get("store_url")?.as_str().unwrap_or("").to_string()))
                    }).collect::<Vec<_>>();
                // Owned titles (casefolded) for the "Reclamado" ribbon.
                // Library failure => empty set => no ribbons (never a
                // false positive when the library can't be read).
                let owned: std::collections::HashSet<String> = StoreManager::library("epic", false)
                    .ok()
                    .and_then(|d| d.get("games").cloned())
                    .and_then(|v| v.as_array().cloned()).unwrap_or_default()
                    .into_iter()
                    .filter_map(|g| g.get("title").and_then(|x| x.as_str()).map(|s| s.to_lowercase()))
                    .collect();
                let _ = tx.send(promos.into_iter().map(|(t, d, c, u)| {
                    let claimed = owned.contains(&t.to_lowercase());
                    (t, d, c, u, claimed)
                }).collect::<Vec<_>>());
            });
            let st = state.clone();
            let no_det_f: Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>> = Rc::new(RefCell::new(None));
            let no_sel_f: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
            crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
                Ok(list) => {
                    while let Some(c) = promo_list.first_child() {
                        promo_list.remove(&c);
                    }
                    if list.is_empty() {
                        promo_lbl.set_text("Nothing free right now.");
                    } else {
                        promo_lbl.set_text("");
                        for (t, d, cover, u, claimed) in list {
                            let entry = crate::backend::game_model::GameEntry {
                                name: t.clone(),
                                banner: cover.clone(),
                                source: crate::backend::game_model::GameSource::Epic,
                                ..Default::default()
                            };
                            let uc = u.clone();
                            let stc2 = st.clone();
                            let open: Rc<dyn Fn()> = Rc::new(move || {
                                stc2.integration.open_url(&uc);
                            });
                            let card = crate::ui::game_card::build_game_card(&entry, &st.config, &no_det_f, &no_sel_f, Some("GRATIS".to_string()), Some(open), false);
                            card.set_tooltip_text(Some(&format!("{}\n{}", t, d)));
                            if claimed {
                                promo_list.append(&claimed_overlay(card, &st.theme));
                            } else {
                                promo_list.append(&card);
                            }
                        }
                    }
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
            // All real Epic offers (any % off, verified dates/prices).
            let (deals_frame, deals_inner) = card("Deals");
            let deals_lbl = note("Loading…");
            deals_inner.append(&deals_lbl);
            // Scrollable list so 80 deals don't stretch the page.
            let deals_list = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let deals_scroll = gtk::ScrolledWindow::new();
            deals_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
            deals_scroll.set_min_content_height(215);
            deals_scroll.set_propagate_natural_height(true);
            deals_scroll.set_vexpand(false);
            deals_scroll.set_child(Some(&deals_list));
            deals_inner.append(&deals_scroll);
            // Numbered pager: 10 deals per page, switching pages drops
            // the previous rows (RAM-friendly no matter how many offers).
            let pager_bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            pager_bar.set_halign(gtk::Align::Center);
            pager_bar.set_margin_top(4);
            deals_inner.append(&pager_bar);
            page.append(&deals_frame);
            {
                // Paged deals: 10 per page over the full on-sale catalog
                // (~2000). Titles deduped across pages (verified promos
                // repeat inside catalog pages).
                const PER_PAGE: u64 = 10;
                let seen: Rc<RefCell<std::collections::HashSet<String>>> =
                    Rc::new(RefCell::new(std::collections::HashSet::new()));
                let next: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
                let total: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(u64::MAX));
                let loading: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
                let shown: Rc<std::cell::Cell<usize>> = Rc::new(std::cell::Cell::new(0));
                let page: Rc<std::cell::Cell<usize>> = Rc::new(std::cell::Cell::new(1));
                let pages: Rc<std::cell::Cell<usize>> = Rc::new(std::cell::Cell::new(1));
                let loader: Rc<RefCell<Option<Rc<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
                let epoch: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
                let goto_slot: Rc<RefCell<Option<Rc<dyn Fn(usize)>>>> = Rc::new(RefCell::new(None));
                let rebuild_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
                {
                    let list_c = deals_list.clone();
                    let epoch_c = epoch.clone();
                    let lbl_c = deals_lbl.clone();
                    let seen_c = seen.clone();
                    let next_c = next.clone();
                    let total_c = total.clone();
                    let loading_c = loading.clone();
                    let shown_c = shown.clone();
                    let st_c = state.clone();
                    let page_c = page.clone();
                    let pages_c = pages.clone();
                    let rebuild_c = rebuild_slot.clone();
                    *loader.borrow_mut() = Some(Rc::new(move |start: u64| {
                        if loading_c.get() {
                            return;
                        }
                        loading_c.set(true);
                        let (tx, rx) = std::sync::mpsc::channel::<serde_json::Value>();
                        std::thread::spawn(move || {
                            let _ = tx.send(StoreManager::epic_deals(start, PER_PAGE).unwrap_or_default());
                        });
                        let list_cc = list_c.clone();
                        let lbl_cc = lbl_c.clone();
                        let seen_cc = seen_c.clone();
                        let next_cc = next_c.clone();
                        let total_cc = total_c.clone();
                        let loading_cc = loading_c.clone();
                        let shown_cc = shown_c.clone();
                        let st_cc = st_c.clone();
                        let epoch_cc = epoch_c.clone();
                        let my_epoch = epoch_c.get();
                        let page_cc = page_c.clone();
                        let pages_cc = pages_c.clone();
                        let rebuild_cc = rebuild_c.clone();
                        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
                            Ok(doc) => {
                                if epoch_cc.get() != my_epoch {
                                    loading_cc.set(false);
                                    return glib::ControlFlow::Break;
                                }
                                let arr = doc.get("deals").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                let fetched = doc.get("fetched").and_then(|v| v.as_u64()).unwrap_or(arr.len() as u64);
                                let has_more = doc.get("has_more").and_then(|v| v.as_bool()).unwrap_or(false);
                                let start = doc.get("start").and_then(|v| v.as_u64()).unwrap_or(start);
                                if doc.get("deals").is_none() {
                                    total_cc.set(next_cc.get());
                                } else {
                                    let real_total = doc.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
                                    next_cc.set(start + fetched);
                                    if real_total > 0 {
                                        total_cc.set(real_total.max(start + fetched));
                                    } else {
                                        total_cc.set(start + fetched + if has_more { PER_PAGE } else { 0 });
                                    }
                                }
                                let mut added = 0usize;
                                let no_det: Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>> = Rc::new(RefCell::new(None));
                                let no_sel: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
                                for p in arr {
                                    let t = match p.get("title").and_then(|x| x.as_str()) {
                                        Some(s) if !s.is_empty() => s.to_string(),
                                        _ => continue,
                                    };
                                    if !seen_cc.borrow_mut().insert(t.clone()) {
                                        continue;
                                    }
                                    let d = p.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let cover = p.get("cover").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let pct = p.get("discount").and_then(|x| x.as_i64()).unwrap_or(0);
                                    let price = p.get("price").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let base = p.get("base_price").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let ends = p.get("ends").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let u = p.get("store_url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let entry = crate::backend::game_model::GameEntry {
                                        name: t.clone(),
                                        banner: cover.clone(),
                                        source: crate::backend::game_model::GameSource::Epic,
                                        ..Default::default()
                                    };
                                    let badge = if pct > 0 { Some(format!("-{}%", pct)) } else { None };
                                    let uc = u.clone();
                                    let stc2 = st_cc.clone();
                                    let open: Rc<dyn Fn()> = Rc::new(move || {
                                        stc2.integration.open_url(&uc);
                                    });
                                    let card = crate::ui::game_card::build_game_card(&entry, &st_cc.config, &no_det, &no_sel, badge, Some(open), false);
                                    card.set_tooltip_text(Some(&format!("{}\n{}", t, d)));
                                    let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
                                    cell.append(&card);
                                    let priceline = if ends.is_empty() {
                                        format!("{} (was {})", price, base)
                                    } else {
                                        format!("{} (was {}) • ends {}", price, base, ends)
                                    };
                                    let pl = gtk::Label::new(Some(&priceline));
                                    pl.set_halign(gtk::Align::Start);
                                    pl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                    pl.set_max_width_chars(28);
                                    pl.add_css_class("info-value");
                                    cell.append(&pl);
                                    list_cc.append(&cell);
                                    added += 1;
                                }
                                shown_cc.set(shown_cc.get() + added);
                                loading_cc.set(false);
                                let s = shown_cc.get();
                                let npages = ((total_cc.get() + PER_PAGE - 1) / PER_PAGE).max(1) as usize;
                                pages_cc.set(npages);
                                if s == 0 {
                                    lbl_cc.set_text("No offers right now.");
                                } else if has_more {
                                    lbl_cc.set_text(&format!("Page {} of ~{}", page_cc.get().max(1), npages));
                                } else {
                                    lbl_cc.set_text(&format!("Page {} of {}", page_cc.get().max(1), npages));
                                }
                                if let Some(r) = rebuild_cc.borrow().as_ref() {
                                    r();
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(_) => glib::ControlFlow::Break,
                        });
                    }) as Rc<dyn Fn(u64)>);
                }
                // Goto page: drop current rows (frees RAM), bump the epoch
                // so late loads can't repopulate, then fetch the page.
                {
                    let list_c = deals_list.clone();
                    let lbl_c = deals_lbl.clone();
                    let page_c = page.clone();
                    let pages_c = pages.clone();
                    let seen_c = seen.clone();
                    let loader_c = loader.clone();
                    let epoch_c = epoch.clone();
                    *goto_slot.borrow_mut() = Some(Rc::new(move |p: usize| {
                        let n = pages_c.get().max(1);
                        let p = p.clamp(1, n);
                        epoch_c.set(epoch_c.get().wrapping_add(1));
                        while let Some(c) = list_c.first_child() {
                            list_c.remove(&c);
                        }
                        seen_c.borrow_mut().clear();
                        page_c.set(p);
                        lbl_c.set_text("Loading…");
                        if let Some(f) = loader_c.borrow().as_ref() {
                            f(((p - 1) as u64) * PER_PAGE);
                        }
                    }) as Rc<dyn Fn(usize)>);
                }
                // Pager buttons: « 1 … c-1 c c+1 … N ».
                {
                    let bar_c = pager_bar.clone();
                    let page_c = page.clone();
                    let pages_c = pages.clone();
                    let goto_c = goto_slot.clone();
                    *rebuild_slot.borrow_mut() = Some(Rc::new(move || {
                        while let Some(c) = bar_c.first_child() {
                            bar_c.remove(&c);
                        }
                        let cur = page_c.get().max(1);
                        let n = pages_c.get().max(1);
                        let nav = |label: &str, target: usize, sensitive: bool| {
                            let b = gtk::Button::with_label(label);
                            b.add_css_class("settings-btn");
                            b.set_sensitive(sensitive);
                            let goto_cc = goto_c.clone();
                            b.connect_clicked(move |_| {
                                if let Some(g) = goto_cc.borrow().as_ref() {
                                    g(target);
                                }
                            });
                            bar_c.append(&b);
                        };
                        nav("«", cur.saturating_sub(1).max(1), cur > 1);
                        let mut nums = vec![1usize, n];
                        for d in -2i32..=2 {
                            let t = cur as i32 + d;
                            if t >= 1 && (t as usize) <= n {
                                nums.push(t as usize);
                            }
                        }
                        nums.sort_unstable();
                        nums.dedup();
                        let mut last = 0usize;
                        for t in nums {
                            if t > last + 1 {
                                let e = gtk::Label::new(Some("…"));
                                e.add_css_class("time-label");
                                bar_c.append(&e);
                            }
                            let b = gtk::Button::with_label(&t.to_string());
                            b.add_css_class(if t == cur { "add-btn" } else { "settings-btn" });
                            b.set_sensitive(t != cur);
                            let goto_cc = goto_c.clone();
                            b.connect_clicked(move |_| {
                                if let Some(g) = goto_cc.borrow().as_ref() {
                                    g(t);
                                }
                            });
                            bar_c.append(&b);
                            last = t;
                        }
                        nav("»", (cur + 1).min(n), cur < n);
                    }) as Rc<dyn Fn()>);
                }
                // Register pager state so leaving the page can drop rows/covers.
                deals_map.borrow_mut().insert(store.to_string(), DealsState {
                    list: deals_list.clone(),
                    lbl: deals_lbl.clone(),
                    page: page.clone(),
                    pages: pages.clone(),
                    loader: loader.clone(),
                    goto: goto_slot.clone(),
                    epoch: epoch.clone(),
                });
                let first = goto_slot.borrow().as_ref().cloned();
                if let Some(g) = first {
                    g(1);
                }
            }
            // Epic has no public search API: browser search with the query.
            let (shop_frame, shop_inner) = card("Buy on Epic");
            let shop_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let shop_q = gtk::SearchEntry::new();
            shop_q.set_placeholder_text(Some("Search the Epic store…"));
            shop_q.set_hexpand(true);
            shop_row.append(&shop_q);
            let shop_go = gtk::Button::with_label("Search");
            shop_go.add_css_class("settings-btn");
            shop_row.append(&shop_go);
            shop_inner.append(&shop_row);
            page.append(&shop_frame);
            let st2 = state.clone();
            let go_search = Rc::new(RefCell::new(Box::new(move || {}) as Box<dyn Fn()>));
            *go_search.borrow_mut() = {
                let q = shop_q.clone();
                Box::new(move || {
                    let query: String = url_encode(&q.text().to_string());
                    if !query.is_empty() {
                        st2.integration.open_url(&format!("https://store.epicgames.com/en-US/browse?q={}&sortBy=relevancy", query));
                    }
                })
            };
            {
                let g = go_search.clone();
                shop_go.connect_clicked(move |_| g.borrow()());
            }
            {
                let g = go_search.clone();
                shop_q.connect_activate(move |_| g.borrow()());
            }
        }
        if store == "gog" {
            // GOG public catalog: real search with covers, prices, buy links.
            let (shop_frame, shop_inner) = card("Buy on GOG");
            let shop_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let shop_q = gtk::SearchEntry::new();
            shop_q.set_placeholder_text(Some("Search the GOG store…"));
            shop_q.set_hexpand(true);
            shop_row.append(&shop_q);
            let shop_go = gtk::Button::with_label("Search");
            shop_go.add_css_class("settings-btn");
            shop_row.append(&shop_go);
            shop_inner.append(&shop_row);
            let shop_status = note("");
            shop_inner.append(&shop_status);
            let shop_results = gtk::Box::new(gtk::Orientation::Vertical, 6);
            shop_inner.append(&shop_results);
            page.append(&shop_frame);
            let st3 = state.clone();
            let do_shop = Rc::new(RefCell::new(Box::new(move || {}) as Box<dyn Fn()>));
            *do_shop.borrow_mut() = {
                let q = shop_q.clone();
                let res0 = shop_results.clone();
                let lbl0 = shop_status.clone();
                let st0 = st3.clone();
                Box::new(move || {
                    let res = res0.clone();
                    let lbl = lbl0.clone();
                    let stc = st0.clone();
                    let query = q.text().to_string().trim().to_string();
                    if query.is_empty() {
                        return;
                    }
                    while let Some(c) = res.first_child() {
                        res.remove(&c);
                    }
                    lbl.set_text("Searching GOG…");
                    let (tx, rx) = std::sync::mpsc::channel::<Vec<(String, String, String, String, String, String)>>();
                    std::thread::spawn(move || {
                        let _ = tx.send(StoreManager::gog_store_search(&query, 10).ok()
                            .and_then(|d| d.get("results").cloned())
                            .and_then(|v| v.as_array().cloned()).unwrap_or_default()
                            .into_iter().map(|p| (
                                p.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                p.get("cover").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                p.get("price").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                p.get("developer").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                p.get("store_url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                p.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            )).collect::<Vec<_>>());
                    });
                    crate::backend::plugin_process::poll_once_local(rx, move |r2| match r2 {
                        Ok(items) => {
                            while let Some(c) = res.first_child() {
                                res.remove(&c);
                            }
                            if items.is_empty() {
                                lbl.set_text("No results.");
                            } else {
                                lbl.set_text(&format!("{} result(s) — buying happens on gog.com", items.len()));
                            }
                            for (t, cover, price, dev, url, gid) in items {
                                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                if !cover.is_empty() {
                                    let img = gtk::Image::new();
                                    img.set_pixel_size(64);
                                    img.set_valign(gtk::Align::Center);
                                    crate::ui::minecraft_view::load_mod_icon(&cover, &format!("gogshop-{}", gid), &img, 64);
                                    row.append(&img);
                                }
                                let mid = gtk::Box::new(gtk::Orientation::Vertical, 2);
                                mid.set_hexpand(true);
                                let lbl = gtk::Label::new(Some(&t));
                                lbl.set_halign(gtk::Align::Start);
                                lbl.add_css_class("details-title");
                                mid.append(&lbl);
                                let sub = gtk::Label::new(Some(&format!("{}  •  {}", dev, price)));
                                sub.set_halign(gtk::Align::Start);
                                sub.set_opacity(0.6);
                                sub.add_css_class("time-label");
                                mid.append(&sub);
                                row.append(&mid);
                                if !url.is_empty() {
                                    let buy = gtk::Button::with_label(if price == "$0.00" { "Get" } else { "Buy" });
                                    buy.add_css_class("add-btn");
                                    buy.set_valign(gtk::Align::Center);
                                    let stcc = stc.clone();
                                    let uc = url.clone();
                                    buy.connect_clicked(move |_| { stcc.integration.open_url(&uc); });
                                    row.append(&buy);
                                }
                                res.append(&row);
                            }
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => glib::ControlFlow::Break,
                    });
                })
            };
            {
                let g = do_shop.clone();
                shop_go.connect_clicked(move |_| g.borrow()());
            }
            {
                let g = do_shop.clone();
                shop_q.connect_activate(move |_| g.borrow()());
            }
        }

        // library card
        let (lib_frame, lib_inner) = card("Library");
        let lib_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let lib_status = note("Press Refresh.");
        lib_status.set_hexpand(true);
        lib_row.append(&lib_status);
        let refresh_btn = gtk::Button::with_label("Refresh");
        refresh_btn.add_css_class("settings-btn");
        lib_row.append(&refresh_btn);
        lib_inner.append(&lib_row);
        // Toolbar Fase 1, solo tab GOG: búsqueda + orden en memoria sobre la
        // lista completa (sin red). Epic mantiene su UI intacta. El cableado
        // va tras construir el handle (necesita el `view`).
        let gog_search: Option<gtk::SearchEntry>;
        let gog_sort_dd: Option<gtk::DropDown>;
        if store == "gog" {
            let tools = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let search = gtk::SearchEntry::new();
            search.set_hexpand(true);
            search.set_placeholder_text(Some("Search GOG library…"));
            tools.append(&search);
            let sort_dd = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
            let sorts = gtk::StringList::new(&["Title A–Z", "Title Z–A", "Installed first"]);
            let model: gtk::gio::ListModel = sorts.upcast();
            sort_dd.set_model(Some(&model));
            sort_dd.set_selected(0);
            tools.append(&sort_dd);
            lib_inner.append(&tools);
            gog_search = Some(search);
            gog_sort_dd = Some(sort_dd);
        } else {
            gog_search = None;
            gog_sort_dd = None;
        }
        let flow = gtk::FlowBox::new();
        flow.set_max_children_per_line(10);
        flow.set_min_children_per_line(1);
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_row_spacing(8);
        flow.set_column_spacing(8);
        flow.set_halign(gtk::Align::Fill);
        flow.set_hexpand(true);
        lib_inner.append(&flow);
        page.append(&lib_frame);

        // Estado "Setup incomplete": tapa el tab cuando falta el binario.
        // Oculto por defecto; lo muestra `aplicar_faltantes`.
        let incomplete_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        incomplete_box.set_halign(gtk::Align::Center);
        incomplete_box.set_valign(gtk::Align::Center);
        incomplete_box.set_vexpand(true);
        let incomplete_title = gtk::Label::new(Some("Setup incomplete"));
        incomplete_title.add_css_class("details-title");
        incomplete_box.append(&incomplete_title);
        let incomplete_text = note("");
        incomplete_text.set_halign(gtk::Align::Center);
        incomplete_text.set_wrap(true);
        incomplete_box.append(&incomplete_text);
        let incomplete_btn = gtk::Button::with_label("Install dependencies");
        incomplete_btn.add_css_class("suggested-action");
        incomplete_btn.set_halign(gtk::Align::Center);
        incomplete_box.append(&incomplete_btn);
        incomplete_box.set_visible(false);
        page.append(&incomplete_box);

        // Lo pone StoresView tras construir los handles.
        let reabrir: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        {
            let r = reabrir.clone();
            incomplete_btn.connect_clicked(move |_| {
                if let Some(f) = r.borrow().as_ref() {
                    f();
                }
            });
        }
        {
            let r = reabrir.clone();
            browser_btn.connect_clicked(move |_| {
                if let Some(f) = r.borrow().as_ref() {
                    f();
                }
            });
        }

        let view = StorePageHandle {
            state: state.clone(),
            parent: parent.clone(),
            sidebar: sidebar.clone(),
            center: center.clone(),
            details: details.clone(),
            store: s.clone(),
            flow: flow.clone(),
            lib_status: lib_status.clone(),
            auth_badge: auth_badge.clone(),
            auth_hint: auth_hint.clone(),
            login_btn: login_btn.clone(),
            login_box: login_box.clone(),
            acc_row: acc_row.clone(),
            acc_name: acc_name.clone(),
            acc_avatar: acc_avatar.clone(),
            auth_frame: auth_frame.clone(),
            lib_frame: lib_frame.clone(),
            incomplete_box: incomplete_box.clone(),
            incomplete_text: incomplete_text.clone(),
            browser_row: browser_row.clone(),
            bloqueado: Rc::new(std::cell::Cell::new(false)),
            reabrir,
            all_games: Rc::new(RefCell::new(Vec::new())),
            shown_games: Rc::new(RefCell::new(Vec::new())),
            gog_query: Rc::new(RefCell::new(String::new())),
            gog_sort: Rc::new(std::cell::Cell::new(0)),
            loaded: Rc::new(std::cell::Cell::new(false)),
            desc_killer: Rc::new(RefCell::new(None)),
            desc_running: Rc::new(std::cell::Cell::new(false)),
            desc_gen: Rc::new(std::cell::Cell::new(0)),
            tiles: Rc::new(RefCell::new(std::collections::HashMap::new())),
            status_src: Rc::new(RefCell::new(None)),
            status_gen: Rc::new(std::cell::Cell::new(0)),
            status_running: Rc::new(std::cell::Cell::new(false)),
        };

        // auth wiring: login automatizado en un Firefox real, para ambas
        // tiendas. El plugin devuelve la URL (`auth` sin código → evento
        // `auth_url`) y a partir de ahí se la pasa a `webdriver_login`, que
        // abre su propia ventana, espera al login y devuelve el código. No hay
        // webview embebido porque el SSO de Google se cuelga en WebKitGTK, y
        // no hay modal de código: el código se captura solo o se explica por qué
        // no se pudo.
        {
            let vh = view.clone();
            login_btn.connect_clicked(move |_| {
                vh.begin_browser_login();
            });
        }
        {
            let vh = view.clone();
            logout_btn.connect_clicked(move |_| {
                vh.auth_hint.set_visible(true);
                vh.auth_hint.set_text("Logging out…");
                let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
                let store = vh.store.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(StoreManager::logout(&store).map(|_| ()).map_err(|e| e.to_string()));
                });
                let vv = vh.clone();
                crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
                    Ok(_) => {
                        vv.refresh_auth(false);
                        vv.refresh_library(false);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        {
            let vh = view.clone();
            refresh_btn.connect_clicked(move |_| {
                if vh.store == "epic" {
                    vh.refresh_descriptions();
                } else {
                    vh.refresh_library(false);
                }
            });
        }
        // Full status at page build: --quick keeps `accounts` empty (quick=true
        // => accounts={"epic":""}) so the account row would fall back to the
        // store literal ("epic"). Non-quick fills the real displayName (e.g. "Matyy_y").
        //
        // Cableado del toolbar GOG + apertura de ficha por click en tarjeta.
        // Solo tab GOG (Epic no tiene toolbar ni tarjetas clicables).
        if s == "gog" {
            if let (Some(search), Some(sort_dd)) = (gog_search, gog_sort_dd) {
                let vv = view.clone();
                search.connect_search_changed(move |e| {
                    *vv.gog_query.borrow_mut() = e.text().to_string();
                    vv.render_filtered();
                });
                let vv2 = view.clone();
                sort_dd.connect_selected_notify(move |dd| {
                    vv2.gog_sort.set(dd.selected());
                    vv2.render_filtered();
                });
            }
            let vv3 = view.clone();
            let flow_c = flow.clone();
            flow_c.connect_child_activated(move |_, child| {
                let idx = child.index() as usize;
                if let Some(g) = vv3.shown_games.borrow().get(idx).cloned() {
                    vv3.show_game_info(&g);
                }
            });
        }
        view.refresh_auth(false);
        (page, view)
    }
}

/// Deals pager state (Epic page only) so leaving the Stores page can
/// drop rows/covers and coming back reloads from page 0.
#[derive(Clone)]
struct DealsState {
    list: gtk::Box,
    lbl: gtk::Label,
    page: Rc<std::cell::Cell<usize>>,
    pages: Rc<std::cell::Cell<usize>>,
    loader: Rc<RefCell<Option<Rc<dyn Fn(u64)>>>>,
    goto: Rc<RefCell<Option<Rc<dyn Fn(usize)>>>>,
    epoch: Rc<std::cell::Cell<u64>>,
}

/// Live widgets of one library tile that the lightweight installed-status
/// check updates in place: the badge row (gains/loses the "installed"
/// label), the Install/Import button label, and nothing else. All edits
/// happen on the GTK main loop via poll_once_local.
struct TileStatus {
    brow: gtk::Box,
    inst_lbl: Rc<RefCell<Option<gtk::Label>>>,
    btn: gtk::Button,
}

impl TileStatus {
    fn apply_installed(&self, installed: bool) {
        self.btn.set_label(if installed { "Import" } else { "Install" });
        if installed {
            if self.inst_lbl.borrow().is_none() {
                let ib = gtk::Label::new(Some("installed"));
                ib.set_opacity(0.6);
                ib.add_css_class("time-label");
                self.brow.append(&ib);
                *self.inst_lbl.borrow_mut() = Some(ib);
            }
        } else if let Some(lb) = self.inst_lbl.borrow_mut().take() {
            self.brow.remove(&lb);
        }
    }
}

#[derive(Clone)]
struct StorePageHandle {
    state: AppState,
    parent: adw::ApplicationWindow,
    sidebar: Rc<RefCell<Option<crate::ui::sidebar::Sidebar>>>,
    center: Rc<RefCell<Option<crate::ui::center::CenterHandle>>>,
    details: Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>>,
    store: String,
    flow: gtk::FlowBox,
    lib_status: gtk::Label,
    auth_badge: gtk::Label,
    auth_hint: gtk::Label,
    login_btn: gtk::Button,
    login_box: gtk::Box,
    acc_row: gtk::Box,
    acc_name: gtk::Label,
    acc_avatar: gtk::Label,
    // Estado "Setup incomplete": `incomplete_box` tapa el tab cuando falta el
    // binario (ni login ni biblioteca); `browser_row` solo tapa el login
    // cuando falta el navegador. `bloqueado` frena cargas inútiles del plugin.
    // `reabrir` lo pone StoresView y reabre el modal de dependencias.
    auth_frame: gtk::Frame,
    lib_frame: gtk::Frame,
    incomplete_box: gtk::Box,
    incomplete_text: gtk::Label,
    browser_row: gtk::Box,
    bloqueado: Rc<std::cell::Cell<bool>>,
    reabrir: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    // Grilla GOG Fase 1: lista completa + vista filtrada/ordenada en memoria,
    // query y modo de orden del toolbar (solo tab GOG; Epic no los usa).
    all_games: Rc<RefCell<Vec<StoreGame>>>,
    shown_games: Rc<RefCell<Vec<StoreGame>>>,
    gog_query: Rc<RefCell<String>>,
    gog_sort: Rc<std::cell::Cell<u32>>,
    loaded: Rc<std::cell::Cell<bool>>,
    // Background description batch (library card "Refresh" on Epic).
    // ref_cell holds the current process-killer; gen is bumped on every
    // start/cancel so stale events from a killed batch are ignored.
    desc_killer: Rc<RefCell<Option<ProcessKiller>>>,
    desc_running: Rc<std::cell::Cell<bool>>,
    desc_gen: Rc<std::cell::Cell<u64>>,
    // Lightweight installed-status overlay. tiles maps app_id -> the live
    // widgets of the rendered tile so the badge/button can be updated in
    // place (no re-render, no flicker); status_src holds the debounce
    // timer; status_gen is bumped on every reschedule so a stale queued
    // check from an older schedule is dropped; status_running prevents
    // stacking duplicate checks.
    tiles: Rc<RefCell<std::collections::HashMap<String, Rc<TileStatus>>>>,
    status_src: Rc<RefCell<Option<glib::SourceId>>>,
    status_gen: Rc<std::cell::Cell<u64>>,
    status_running: Rc<std::cell::Cell<bool>>,
}

impl StorePageHandle {
    fn state_toast(&self, heading: &str, body: &str) {
        helpers::present_msg(&self.parent, heading, body);
    }

    fn refresh_auth(&self, quick: bool) {
        self.auth_badge.set_text("Checking login…");
        let (tx, rx) = std::sync::mpsc::channel::<(bool, bool, String)>();
        let store = self.store.clone();
        std::thread::spawn(move || {
            let st = StoreManager::status(quick).unwrap_or_default();
            let logged = st.get("logged").cloned().unwrap_or_default();
            let bins = st.get("bins").cloned().unwrap_or_default();
            let accs = st.get("accounts").cloned().unwrap_or_default();
            let _ = tx.send((
                logged.get(&store).and_then(|x| x.as_bool()).unwrap_or(false),
                bins.get(&store).and_then(|x| x.as_bool()).unwrap_or(false),
                accs.get(&store).and_then(|x| x.as_str()).unwrap_or("").to_string(),
            ));
        });
        let badge = self.auth_badge.clone();
        let hint = self.auth_hint.clone();
        let store = self.store.clone();
        let login_box = self.login_box.clone();
        let acc_row = self.acc_row.clone();
        let acc_name = self.acc_name.clone();
        let acc_avatar = self.acc_avatar.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok((is_logged, has_bin, name)) => {
                if !has_bin {
                    badge.set_text("Tools missing.");
                    hint.set_visible(true);
                    hint.set_text("Re-enter Stores to install the missing tools.");
                    login_box.set_visible(true);
                    acc_row.set_visible(false);
                } else if is_logged {
                    login_box.set_visible(false);
                    acc_row.set_visible(true);
                    acc_name.set_text(if name.is_empty() { &store } else { &name });
                    let initial = name.trim().chars().next()
                        .map(|c| c.to_uppercase().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| store.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default());
                    acc_avatar.set_text(&initial);
                    hint.set_visible(false);
                } else {
                    badge.set_text("Not logged in.");
                    login_box.set_visible(true);
                    acc_row.set_visible(false);
                    hint.set_visible(true);
                    hint.set_text(&format!("Log in to {} to list your games.", store));
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn ensure_loaded(&self) {
        if self.loaded.get() || self.bloqueado.get() {
            return;
        }
        self.loaded.set(true);
        self.refresh_library(false);
    }

    /// Aplica el estado de dependencias al tab: panel "Setup incomplete" si
    /// falta el binario (ni login ni biblioteca), o solo bloqueo del login si
    /// falta el navegador. Sin faltantes, UI normal.
    fn aplicar_faltantes(&self, f: &Faltantes) {
        let nombre = if self.store == "epic" { "Epic" } else { "GOG" };
        if f.tab_bloqueado(&self.store) {
            let pendientes = f.para_tienda(&self.store).join(", ");
            self.incomplete_text.set_text(&format!(
                "{} needs: {}. Log in and library are unavailable until installation completes.",
                nombre, pendientes
            ));
            self.incomplete_box.set_visible(true);
            self.auth_frame.set_visible(false);
            self.lib_frame.set_visible(false);
            self.browser_row.set_visible(false);
            self.bloqueado.set(true);
        } else {
            self.incomplete_box.set_visible(false);
            self.auth_frame.set_visible(true);
            self.lib_frame.set_visible(true);
            let sin_browser = f.login_bloqueado();
            self.login_btn.set_visible(!sin_browser);
            self.browser_row.set_visible(sin_browser);
            self.bloqueado.set(false);
        }
    }

    /// Drop library tiles/covers so no RAM is held while the store
    /// page isn't visible. Next visit reloads via ensure_loaded().
    fn unload(&self) {
        self.loaded.set(false);
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        self.lib_status.set_text("");
        self.tiles.borrow_mut().clear();
        if let Some(src) = self.status_src.borrow_mut().take() {
            src.remove();
        }
    }

    /// Debounced lightweight installed-status refresh. Every call bumps
    /// the generation and re-arms a short timer, so rapid-fire triggers
    /// (page entry + window focus regain) collapse into one check; a
    /// stale timer from an older generation is dropped.
    fn schedule_status_check(&self) {
        let gen = self.status_gen.get() + 1;
        self.status_gen.set(gen);
        // Only remove a source that is still pending. glib's SourceId
        // panics if remove() is called on a source that already fired;
        // the timer closure below clears the slot the moment it runs, so
        // the slot only ever holds a live (removable) id here.
        if let Some(src) = self.status_src.borrow_mut().take() {
            src.remove();
        }
        let vh = self.clone();
        let id = glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
            if vh.status_gen.get() == gen {
                // This source is firing/being destroyed: drop the stored
                // id *before* running, so a later schedule never tries to
                // remove() a dead source.
                *vh.status_src.borrow_mut() = None;
                vh.run_status_check();
            }
            glib::ControlFlow::Break
        });
        *self.status_src.borrow_mut() = Some(id);
    }

    /// Run one local installed-status check off the GTK main loop and
    /// apply the result in place. Never stacks: while a check is running
    /// new triggers are ignored (the tiles already render current state).
    fn run_status_check(&self) {
        if self.status_running.get() {
            return;
        }
        self.status_running.set(true);
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
        let store = self.store.clone();
        std::thread::spawn(move || {
            let _ = tx.send(StoreManager::installed_status(&store));
        });
        let vh = self.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok(doc)) => {
                vh.status_running.set(false);
                vh.apply_installed_status(&doc);
                glib::ControlFlow::Break
            }
            Ok(Err(_)) => {
                // Lightweight check is best-effort; the full library
                // render carries authoritative errors. Silently keep the
                // current state rather than toasting on focus events.
                vh.status_running.set(false);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                vh.status_running.set(false);
                glib::ControlFlow::Break
            }
        });
    }

    /// Apply a plugin `installed-status` payload to the tracked tiles.
    fn apply_installed_status(&self, doc: &serde_json::Value) {
        let Some(map) = doc.get("installed").and_then(|x| x.as_object()) else {
            return;
        };
        for (app_id, status) in map {
            let installed = status.get("installed").and_then(|x| x.as_bool()).unwrap_or(false);
            if let Some(ts) = self.tiles.borrow().get(app_id) {
                ts.apply_installed(installed);
            }
        }
    }

    /// Login automatizado en un navegador real, sin paso manual.
    ///
    /// El plugin emite la URL (`auth` sin código → evento `auth_url`) y a partir
    /// de ahí no interviene: el launcher se la pasa a `webdriver_login`, que
    /// abre su propia ventana de Chromium, espera a que la persona termine de
    /// autenticarse y devuelve el código por stdout.
    ///
    /// Chromium y no el navegador del sistema, y no Firefox: el hCaptcha de
    /// Epic rechaza el reto si `navigator.webdriver` es `true`, y cualquier
    /// Firefox gobernado por WebDriver lo pone en `true` sin forma de
    /// desactivarlo. Chromium lanzado a mano por CDP deja el valor en `false`.
    /// El comentario de `src/bin/webdriver_login.rs` tiene la tabla completa.
    ///
    /// No hay ruta manual. Ni modal de código, ni copiar y pegar, ni se le pide
    /// al usuario que desactive nada. Si el login no se puede completar, se
    /// dice por qué y el botón "Log in" vuelve a quedar pulsable.
    fn begin_browser_login(&self) {
        self.login_btn.set_sensitive(false);
        let rx = StoreManager::spawn_auth_begin(self.store.clone());
        let vh = self.clone();
        vh.auth_hint.set_visible(true);
        vh.auth_hint.set_text("Preparing the browser login…");
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| match ev {
            PluginEvent::Custom(val) => {
                if val.get("type").and_then(|x| x.as_str()) != Some("auth_url") {
                    return true;
                }
                let url = val.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                if url.is_empty() {
                    vh.login_btn.set_sensitive(true);
                    vh.auth_hint.set_visible(true);
                    vh.auth_hint.set_text("The store helper did not return a login URL.");
                    return false;
                }
                vh.auth_hint.set_text("Opening a login window — sign in there.");
                vh.watch_login_helper(url);
                false
            }
            PluginEvent::Done(val) => {
                // El plugin ya quedó autenticado (p. ej. una sesión viva).
                let who = val
                    .get("account")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                vh.login_btn.set_sensitive(true);
                vh.auth_hint.set_visible(true);
                let msg = if who.is_empty() {
                    "Already logged in.".to_string()
                } else {
                    format!("Logged in as {}", who)
                };
                vh.auth_hint.set_text(&msg);
                vh.refresh_auth(false);
                vh.refresh_library(false);
                false
            }
            PluginEvent::Error { message, .. } => {
                vh.login_btn.set_sensitive(true);
                vh.auth_hint.set_visible(true);
                vh.auth_hint.set_text(&format!("Login could not start: {}", message));
                false
            }
            _ => true,
        });
    }

    /// Lanza `webdriver_login` en segundo plano y espera a que devuelva el código.
    ///
    /// El helper es un proceso aparte a propósito: hablar WebDriver es asíncrono
    /// y el launcher es síncrono, así que meter ese runtime en el binario
    /// principal significaría arrastrar tokio/hyper a toda la app. Aquí solo se
    /// leen sus stdout y su código de salida.
    ///
    /// El watchdog cubre el caso de que el proceso muera sin escribir nada. El
    /// límite de tiempo del login lo pone el propio helper (180 s), que además
    /// devuelve en su mensaje la última URL que vio; el de aquí es un margen.
    fn watch_login_helper(&self, url: String) {
        let rx = spawn_login_helper(url);
        let vh = self.clone();
        let guard = Rc::new(std::cell::Cell::new(true));
        let guard_t = guard.clone();
        let vh_t = vh.clone();
        glib::timeout_add_local(std::time::Duration::from_secs(210), move || {
            if guard_t.replace(false) {
                vh_t.login_btn.set_sensitive(true);
                vh_t.auth_hint.set_visible(true);
                vh_t.auth_hint.set_text(
                    "The browser login helper stopped responding. Press Log in to try again.",
                );
            }
            glib::ControlFlow::Break
        });
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok(code)) => {
                guard.set(false);
                vh.auth_hint.set_visible(true);
                vh.auth_hint.set_text("Got the code — signing in…");
                vh.do_login(&code);
                glib::ControlFlow::Break
            }
            Ok(Err((why, detail))) => {
                guard.set(false);
                vh.login_btn.set_sensitive(true);
                vh.auth_hint.set_visible(true);
                vh.auth_hint
                    .set_text(&login_failure_message(&why, &detail));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                guard.set(false);
                vh.login_btn.set_sensitive(true);
                vh.auth_hint.set_visible(true);
                vh.auth_hint.set_text(&login_failure_message(
                    "session",
                    "the helper process ended without giving a code",
                ));
                glib::ControlFlow::Break
            }
        });
    }

    fn do_login(&self, code: &str) {
        let rx = StoreManager::spawn_auth(self.store.clone(), code.to_string());
        let vh = self.clone();
        // Watchdog guard: Done and Error disable it, so "Validating code…"
        // always ends (success, visible error or visible timeout) and never
        // hangs forever.
        let guard = Rc::new(std::cell::Cell::new(true));
        let guard_t = guard.clone();
        let vh_t = vh.clone();
        glib::timeout_add_local(std::time::Duration::from_secs(150), move || {
            if guard_t.replace(false) {
                vh_t.login_btn.set_sensitive(true);
                vh_t.auth_hint.set_visible(true);
                vh_t.auth_hint.set_text("Validation timed out — no response from the login helper. Retry with a fresh code.");
            }
            glib::ControlFlow::Break
        });
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Custom(_) => true,
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    guard.set(false);
                    vh.login_btn.set_sensitive(true);
                    let who = val.get("account").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    vh.state_toast("Logged in", if who.is_empty() { &vh.store } else { &who });
                    vh.refresh_auth(false);
                    vh.refresh_library(true);
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, code, .. } => {
                    guard.set(false);
                    vh.login_btn.set_sensitive(true);
                    vh.auth_hint.set_visible(true);
                    // Suffix only for real code rejections (die 4): a missing
                    // binary (5), network (6) or other failures are already
                    // self-explanatory.
                    if code == Some(4) {
                        vh.auth_hint.set_text(&format!("{} (codes expire fast — open ONE fresh tab and retry)", message));
                    } else {
                        vh.auth_hint.set_text(&message);
                    }
                    false
                }
                _ => true,
            }
        });
    }

    fn refresh_library(&self, force: bool) {
        self.lib_status.set_text("Loading library…");
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        let (tx, rx) = std::sync::mpsc::channel::<Result<(Vec<StoreGame>, Option<String>), String>>();
        let store = self.store.clone();
        std::thread::spawn(move || {
            let _ = tx.send(StoreManager::library(&store, force).map(|d| {
                let warn = d.get("warning").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                let games = d.get("games").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                    .into_iter().map(|g| StoreGame {
                        app_id: g.get("app_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        title: g.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        category: g.get("category").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        systems: g.get("systems").and_then(|x| x.as_array()).map(|a| {
                            a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect()
                        }).unwrap_or_default(),
                        version: g.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        installed: g.get("installed").and_then(|x| x.as_bool()).unwrap_or(false),
                        stale_registry: g.get("stale_registry").and_then(|x| x.as_bool()).unwrap_or(false),
                        install_path: g.get("install_path").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        executable: g.get("executable").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        cover: g.get("cover").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        description: g.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    }).collect::<Vec<_>>();
                (games, warn)
            }).map_err(|e| e.to_string()));
        });
        let vh = self.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok((games, warn))) => {
                *vh.all_games.borrow_mut() = games.clone();
                if vh.store == "gog" {
                    vh.render_filtered();
                } else {
                    vh.render_games(&games);
                }
                // Degradación parcial del plugin: nota no bloqueante, nunca
                // página de error (el `die(3)` se eliminó en Fase 1).
                if let Some(w) = warn {
                    let cur = vh.lib_status.text().to_string();
                    vh.lib_status.set_text(&format!("{} · partial: {}", cur, w.chars().take(80).collect::<String>()));
                }
                crate::refresh_warn_buttons(&vh.state);
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                vh.lib_status.set_text(&e);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    /// Background batch re-resolution of every library description (Epic).
    /// The plugin streams `library` (fresh grid data), `progress`
    /// (done/total/stage) and `done`. Re-pressing Refresh cancels the
    /// running batch (SIGTERM) and starts a new one; the checker lets the
    /// kill finish without piling up duplicate work.
    fn refresh_descriptions(&self) {
        let gen = self.desc_gen.get() + 1;
        self.desc_gen.set(gen);
        let mut killer = self.desc_killer.borrow_mut();
        if let Some(k) = killer.take() {
            k.kill();
        }
        drop(killer);
        self.desc_running.set(true);
        self.lib_status.set_text("Actualizando descripciones…");
        let (rx, k) = StoreManager::spawn_refresh_library(self.store.clone(), "spanish".to_string());
        *self.desc_killer.borrow_mut() = Some(k);
        let vh = self.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            if vh.desc_gen.get() != gen {
                return false;
            }
            match ev {
                PluginEvent::Custom(val) => {
                    if val.get("type").and_then(|x| x.as_str()) == Some("library") {
                        let games = val.get("games")
                            .and_then(|x| x.as_array()).cloned().unwrap_or_default()
                            .into_iter()
                            .map(|g| StoreGame {
                                app_id: g.get("app_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                title: g.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                // Lote Epic: sin categoría/sistemas (campos GOG).
                                category: String::new(),
                                systems: Vec::new(),
                                version: g.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                installed: g.get("installed").and_then(|x| x.as_bool()).unwrap_or(false),
                                stale_registry: g.get("stale_registry").and_then(|x| x.as_bool()).unwrap_or(false),
                                install_path: g.get("install_path").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                executable: g.get("executable").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                cover: g.get("cover").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                description: g.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            })
                            .collect::<Vec<_>>();
                        vh.render_games(&games);
                    }
                    true
                }
                PluginEvent::Progress { extra, .. } => {
                    let done = extra.get("done").and_then(|x| x.as_u64()).unwrap_or(0);
                    let total = extra.get("total").and_then(|x| x.as_u64()).unwrap_or(0);
                    let stage = extra.get("stage").and_then(|x| x.as_str()).unwrap_or("");
                    vh.lib_status.set_text(&format!(
                        "Actualizando descripciones {}/{} · {stage}",
                        done, total
                    ));
                    true
                }
                PluginEvent::Done(val) => {
                    vh.desc_running.set(false);
                    *vh.desc_killer.borrow_mut() = None;
                    let updated = val.get("updated").and_then(|x| x.as_u64()).unwrap_or(0);
                    let total = val.get("total").and_then(|x| x.as_u64()).unwrap_or(0);
                    let msg = format!("{updated} actualizadas de {total}");
                    vh.lib_status.set_text(&format!("Descripciones actualizadas · {msg}"));
                    vh.state_toast("Descripciones actualizadas", &msg);
                    false
                }
                PluginEvent::Error { message, .. } => {
                    vh.desc_running.set(false);
                    *vh.desc_killer.borrow_mut() = None;
                    vh.lib_status.set_text(&format!("Error: {message}"));
                    vh.state_toast("No se pudieron actualizar las descripciones", &message);
                    false
                }
                _ => true,
            }
        });
    }

    /// Re-render GOG desde la lista completa con query y orden actuales.
    /// Todo en memoria, sin red. Epic no pasa por acá.
    fn render_filtered(&self) {
        let all = self.all_games.borrow().clone();
        let q = self.gog_query.borrow().clone();
        let v = Self::filtrar_ordenar(&all, &q, self.gog_sort.get());
        *self.shown_games.borrow_mut() = v.clone();
        self.render_gog_cards(&v, !all.is_empty());
    }

    /// Línea "category · systems" de la tarjeta GOG: cada mitad se oculta si
    /// está vacía (los campos son opcionales en la respuesta de GOG).
    fn render_gog_cards(&self, games: &[StoreGame], hay_mas: bool) {
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        if games.is_empty() {
            self.lib_status.set_text(if hay_mas {
                "No games match the filter."
            } else {
                "No games. Log in and Refresh."
            });
            return;
        }
        self.lib_status.set_text(&format!("{} game(s)", games.len()));
        for g in games {
            let tile = gtk::FlowBoxChild::new();
            tile.set_width_request(170);
            let inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
            inner.set_margin_top(8);
            inner.set_margin_bottom(8);
            inner.set_margin_start(8);
            inner.set_margin_end(8);
            inner.add_css_class("page-card");
            if !g.cover.is_empty() {
                let img = gtk::Image::new();
                img.set_pixel_size(150);
                img.set_halign(gtk::Align::Center);
                crate::ui::minecraft_view::load_mod_icon(&g.cover, &format!("store-gog-{}", g.app_id), &img, 150);
                inner.append(&img);
            }
            let name = gtk::Label::new(Some(&g.title));
            name.set_halign(gtk::Align::Center);
            name.set_wrap(true);
            name.set_lines(2);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(16);
            name.set_size_request(-1, 44);
            name.add_css_class("details-title");
            inner.append(&name);
            let meta = Self::meta_line(&g.category, &g.systems);
            if !meta.is_empty() {
                let meta_lbl = gtk::Label::new(Some(&meta));
                meta_lbl.set_halign(gtk::Align::Center);
                meta_lbl.set_opacity(0.6);
                meta_lbl.add_css_class("time-label");
                inner.append(&meta_lbl);
            }
            if g.installed {
                let ib = gtk::Label::new(Some("✓ installed"));
                ib.set_halign(gtk::Align::Center);
                ib.set_opacity(0.6);
                ib.add_css_class("time-label");
                inner.append(&ib);
            }
            // Sin botones en la tarjeta (diseño GOG Fase 1): click abre la
            // ficha, que sí tiene Install/Import. Las tarjetas Epic quedan
            // intactas con sus botones.
            tile.set_child(Some(&inner));
            self.flow.insert(&tile, -1);
        }
    }

/// Línea "category · systems" de la tarjeta GOG: cada mitad se oculta si está
/// vacía, porque ambos campos son opcionales en la respuesta de GOG
/// (`worksOn` viene roto seguido, `category` puede venir vacío).
fn meta_line(category: &str, systems: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let cat = category.trim();
    if !cat.is_empty() {
        parts.push(cat.to_string());
    }
    let sys: Vec<String> = systems
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !sys.is_empty() {
        parts.push(sys.join(" · "));
    }
    parts.join(" · ")
}

/// Filtro por título/categoría + orden en memoria para la grilla GOG.
/// Puro y testeable: la UI solo lo aplica y renderiza.
fn filtrar_ordenar(juegos: &[StoreGame], query: &str, sort: u32) -> Vec<StoreGame> {
    let q = query.trim().to_lowercase();
    let mut v: Vec<StoreGame> = juegos
        .iter()
        .filter(|g| {
            q.is_empty()
                || g.title.to_lowercase().contains(&q)
                || g.category.to_lowercase().contains(&q)
        })
        .cloned()
        .collect();
    match sort {
        // Title Z–A.
        1 => v.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase())),
        // Installed first, después A–Z.
        2 => v.sort_by(|a, b| {
            b.installed
                .cmp(&a.installed)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        // Title A–Z (default, 0 y cualquier otro).
        _ => v.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
    }
    v
}

    fn render_games(&self, games: &[StoreGame]) {
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        if games.is_empty() {
            self.lib_status.set_text("No games. Log in and Refresh.");
            return;
        }
        self.lib_status.set_text(&format!("{} game(s)", games.len()));
        for g in games {
            let tile = gtk::FlowBoxChild::new();
            tile.set_width_request(190);
            let inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
            inner.set_margin_top(8);
            inner.set_margin_bottom(8);
            inner.set_margin_start(8);
            inner.set_margin_end(8);
            inner.add_css_class("page-card");
            inner.add_css_class("mc-tile");
            if !g.cover.is_empty() {
                let img = gtk::Image::new();
                img.set_pixel_size(120);
                img.set_halign(gtk::Align::Center);
                crate::ui::minecraft_view::load_mod_icon(&g.cover, &format!("store-{}-{}", self.store, g.app_id), &img, 120);
                inner.append(&img);
            }
            let brow = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            brow.set_halign(gtk::Align::Center);
            let badge = gtk::Label::new(Some(if self.store == "epic" { "EPIC" } else { "GOG" }));
            badge.add_css_class("proton-path-badge");
            brow.append(&badge);
            let inst_lbl: Rc<RefCell<Option<gtk::Label>>> = Rc::new(RefCell::new(None));
            if g.installed {
                let ib = gtk::Label::new(Some("installed"));
                ib.set_opacity(0.6);
                ib.add_css_class("time-label");
                brow.append(&ib);
                *inst_lbl.borrow_mut() = Some(ib);
            }
            inner.append(&brow);
            let name = gtk::Label::new(Some(&g.title));
            name.set_halign(gtk::Align::Center);
            name.set_wrap(true);
            name.set_lines(2);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            name.set_max_width_chars(18);
            name.set_size_request(-1, 48);
            name.add_css_class("details-title");
            inner.append(&name);
            let btnrow = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            btnrow.set_halign(gtk::Align::Center);
            btnrow.set_homogeneous(true);
            let btn = gtk::Button::with_label(if g.installed { "Import" } else { "Install" });
            btn.add_css_class("add-btn");
            // Track the tile's live widgets so the lightweight
            // installed-status check can flip badge + button in place
            // without a full re-render.
            self.tiles.borrow_mut().insert(
                g.app_id.clone(),
                Rc::new(TileStatus { brow: brow.clone(), inst_lbl: inst_lbl.clone(), btn: btn.clone() }),
            );
            let vh = self.clone();
            let game = g.clone();
            btn.connect_clicked(move |_| vh.install_or_import(&game));
            btnrow.append(&btn);
            let view = gtk::Button::with_label("View");
            view.add_css_class("settings-btn");
            let vh2 = self.clone();
            let game2 = g.clone();
            view.connect_clicked(move |_| vh2.show_game_info(&game2));
            btnrow.append(&view);
            inner.append(&btnrow);
            tile.set_child(Some(&inner));
            self.flow.insert(&tile, -1);
        }
    }

    fn show_game_info(&self, game: &StoreGame) {
        let dlg = adw::Dialog::new();
        dlg.set_title(&game.title);
        dlg.set_content_width(560);
        // No fixed height: the dialog sizes itself to the content
        // (cover + description + buttons); long descs use "Read more".
        let (header, x_btn) = helpers::modal_header(&game.title);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_top(12);
        body.set_margin_bottom(16);
        body.set_margin_start(16);
        body.set_margin_end(16);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        self.load_game_info(&body, game, &dlg);
        dlg.present(Some(&self.parent));
    }

    /// Loads plugin info into the modal body, clearing it first.
    fn load_game_info(&self, body: &gtk::Box, game: &StoreGame, dlg: &adw::Dialog) {
        while let Some(c) = body.first_child() {
            body.remove(&c);
        }
        body.append(&note("Loading info…"));
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
        let (store, app) = (self.store.clone(), game.app_id.clone());
        std::thread::spawn(move || {
            let _ = tx.send(StoreManager::game_info(&store, &app).map_err(|e| e.to_string()));
        });
        let vh = self.clone();
        let game_c = game.clone();
        let dd0 = dlg.clone();
        let b2 = body.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok(doc)) => {
                vh.render_game_info(&b2, &doc, &game_c, &dd0);
                // Degradación parcial (Fase 1): si la ficha vino incompleta se
                // avisa en vez de mostrarla como si estuviera entera.
                if doc.get("partial").and_then(|x| x.as_bool()).unwrap_or(false) {
                    let w = doc.get("warning").and_then(|x| x.as_str()).unwrap_or("");
                    b2.append(&note(&format!("Incomplete info{}.", if w.is_empty() { String::new() } else { format!(": {}", w.chars().take(90).collect::<String>()) })));
                }
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                while let Some(c) = b2.first_child() {
                    b2.remove(&c);
                }
                b2.append(&note(&format!("Failed: {}", e)));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn render_game_info(&self, body: &gtk::Box, doc: &serde_json::Value, game_c: &StoreGame, dd0: &adw::Dialog) {
        while let Some(c) = body.first_child() {
            body.remove(&c);
        }
        let info = doc.get("info").cloned().unwrap_or_default();
        let get = |k: &str, fb: &str| info.get(k).and_then(|x| x.as_str()).unwrap_or(fb).to_string();
        let title = get("title", &game_c.title);
        let cover = get("cover", &game_c.cover);
        let mut desc = get("description", &game_c.description);
                if desc.trim() == title.trim() {
                    desc.clear();
                }
                let ver = get("version", &game_c.version);
                let last_upd = get("last_updated", "");
                let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                if !cover.is_empty() {
                    let img = gtk::Image::new();
                    img.set_pixel_size(160);
                    img.set_valign(gtk::Align::Start);
                    crate::ui::minecraft_view::load_mod_icon(&cover, &format!("store-{}-info", game_c.app_id), &img, 160);
                    top.append(&img);
                }
                let tcol = gtk::Box::new(gtk::Orientation::Vertical, 4);
                tcol.set_hexpand(true);
                tcol.set_valign(gtk::Align::Fill);
                let tt = gtk::Label::new(Some(&title));
                tt.set_halign(gtk::Align::Start);
                tt.set_wrap(true);
                tt.add_css_class("details-title");
                tcol.append(&tt);
                // Version line only when a real description is present; otherwise
                // the cascaded fallback below already carries it.
                if !ver.is_empty() && !desc.is_empty() {
                    let vs = gtk::Label::new(Some(&ver));
                    vs.set_halign(gtk::Align::Start);
                    vs.set_opacity(0.6);
                    vs.add_css_class("time-label");
                    tcol.append(&vs);
                }
                top.append(&tcol);
                body.append(&top);
                // Cascade: real description -> catalog version/date -> neutral message.
                // "Catálogo de Epic" clarifies the date is Epic's own record,
                // not the user's local install time.
                let na_text = if !ver.is_empty() {
                    if last_upd.is_empty() {
                        format!("Última versión: {ver} (dato del catálogo de Epic)")
                    } else {
                        format!(
                            "Última versión: {ver} (última actualización conocida por Epic: {last_upd})"
                        )
                    }
                } else {
                    "Sin descripción disponible".to_string()
                };
                let has_desc = !desc.is_empty();
                let shown = if has_desc { desc } else { na_text };
                // Description (or its fallback) lives in its own scrolling
                // container so the full text is always reachable. A GtkLabel
                // can't drive a vertical ScrolledWindow: its natural height is
                // measured at its natural (unwrapped) width, so the window
                // collapses to ~2 lines, clips the text and never shows a bar.
                // A read-only TextView reports its true wrapped height, so the
                // viewport measures min(full_height, max_content_height) and
                // GTK shows the scrollbar from the natural height alone — no
                // char/line heuristics, robust to font size and width.
                let tv = gtk::TextView::new();
                tv.set_wrap_mode(gtk::WrapMode::WordChar);
                tv.set_editable(false);
                tv.set_cursor_visible(false);
                tv.set_focusable(false);
                tv.set_opacity(if has_desc { 1.0 } else { 0.6 });
                tv.add_css_class("time-label");
                tv.buffer().set_text(&shown);
                let scr = gtk::ScrolledWindow::new();
                scr.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
                scr.set_min_content_height(120);
                scr.set_max_content_height(300);
                scr.set_propagate_natural_height(true);
                scr.set_vexpand(true);
                scr.set_child(Some(&tv));
                tcol.append(&scr);
                // Source attribution: a single "Fuente: X" link. Steam/Wikipedia
                // get a clickable label (hand cursor, hover underline,
                // opens the source page); Epic stays plain text.
                let desc_src = info.get("description_source").and_then(|x| x.as_str()).unwrap_or("");
                if has_desc && !desc_src.is_empty() && desc_src != "fallback" {
                    let label = match desc_src {
                        "steam" => "Fuente: Steam",
                        "wikipedia" => "Fuente: Wikipedia",
                        _ => "Fuente: Epic Store",
                    };
                    let url = info.get("source_url").and_then(|x| x.as_str())
                        .filter(|u| !u.is_empty())
                        .map(str::to_string);
                    let tip = match desc_src {
                        "wikipedia" => Some("Texto de Wikipedia — CC BY-SA"),
                        _ => None,
                    };
                    tcol.append(&helpers::source_link(&self.state, label, url, tip));
                }
        let brow = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        brow.set_homogeneous(true);
        if let Some(url) = info.get("store_url").and_then(|x| x.as_str()) {
            if !url.is_empty() {
                let open = gtk::Button::with_label("Open store page");
                open.add_css_class("settings-btn");
                let st = self.state.clone();
                let u = url.to_string();
                open.connect_clicked(move |_| { st.integration.open_url(&u); });
                brow.append(&open);
            }
        }
        let ib = gtk::Button::with_label(if game_c.installed { "Import to library" } else { "Install" });
        ib.add_css_class("add-btn");
        let vv = self.clone();
        let gc = game_c.clone();
        let dd = dd0.clone();
        ib.connect_clicked(move |_| {
            dd.close();
            vv.install_or_import(&gc);
        });
        brow.append(&ib);
        body.append(&brow);
    }

    /// Reads an app's saved description from the plugin's descriptions.json
    /// (the plugin is the only writer; Rust just reads). Returns
    /// (description, source, source_url, lang) for a positive entry.
    fn descriptions_entry(app_id: &str) -> Option<(String, String, String, String)> {
        let path = crate::backend::config::ConfigManager::new()
            .config_dir()
            .join("plugins/heroic-store/descriptions.json");
        let content = std::fs::read_to_string(path).ok()?;
        let doc: serde_json::Value = serde_json::from_str(&content).ok()?;
        let e = doc.get(app_id)?;
        let desc = e.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if desc.is_empty() {
            return None;
        }
        let src = e.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = e.get("source_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let lang = e.get("lang").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Some((desc, src, url, lang))
    }

    fn default_games_dir() -> String {
        format!("{}/Games/Heroic", std::env::var("HOME").unwrap_or_default())
    }

    fn install_or_import(&self, game: &StoreGame) {
        // `installed` is the plugin-verified flag (registry + folder +
        // executable on disk). Only a verified game is imported, with its
        // real folder/exe; a stale registry entry or a game the store
        // cannot verify (e.g. GOG installed outside the launcher) falls
        // through to the real Install flow. Importing with an empty
        // install_path is what created broken entries, so it is never
        // attempted here — `import_to_library` also guards it.
        if game.installed {
            self.import_with_manager(game);
            return;
        }
        if game.stale_registry && self.store == "epic" {
            // The legendary record points at a folder that is gone; legendary
            // would silently reuse the OLD install_path (ignoring the base
            // path) on install. Drop the record first, after confirmation.
            self.confirm_cleanup_then_install(game);
            return;
        }
        self.start_install(game);
    }

    // Fresh-install flow shared by every store: progress bar + streaming
    // plugin events, importing into the native library on success.
    fn start_install(&self, game: &StoreGame) {
        let (bar, status) = self.progress(&format!("Installing {}", game.title));
        std::fs::create_dir_all(Self::default_games_dir()).ok();
        let rx = StoreManager::spawn_install(self.store.clone(), game.app_id.clone(), Self::default_games_dir());
        let vh = self.clone();
        let game_c = game.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Progress { stage, percent, .. } => {
                    match percent {
                        Some(pc) => {
                            bar.set_fraction((pc / 100.0).clamp(0.0, 1.0));
                            bar.set_text(Some(&format!("{}%", pc as u32)));
                        }
                        None => bar.pulse(),
                    }
                    status.set_text(&stage.unwrap_or_default());
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    bar.set_fraction(1.0);
                    status.set_text("Adding to library…");
                    let ipath = val.get("install_path").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let exe = val.get("executable").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    vh.import_to_library(&game_c.title, &vh.store, &game_c.app_id, &ipath, &exe, false, false);
                    // In-place status update instead of a full re-render:
                    // the tile flips to "installed"/Import without flicker.
                    vh.schedule_status_check();
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    bar.set_fraction(0.0);
                    status.set_text(&message);
                    vh.state_toast("Install failed", &message);
                    false
                }
                _ => true,
            }
        });
    }

    fn confirm_cleanup_then_install(&self, game: &StoreGame) {
        let dlg = adw::MessageDialog::new(
            Some(&self.parent),
            Some("Stale record"),
            Some(&format!(
                "The record for {} is obsolete. It will be cleaned and the game reinstalled from scratch.",
                game.title
            )),
        );
        if let Some(root) = self.parent.root() {
            if let Ok(win) = root.downcast::<gtk::Window>() {
                dlg.set_transient_for(Some(&win));
                dlg.set_modal(true);
            }
        }
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("clean", "Clean and install");
        dlg.set_response_appearance("clean", adw::ResponseAppearance::Destructive);
        dlg.set_default_response(Some("cancel"));
        dlg.set_close_response("cancel");
        let vh = self.clone();
        let game_c = game.clone();
        dlg.connect_response(None, move |d, resp| {
            if resp == "clean" {
                vh.cleanup_then_install(&game_c);
            }
            d.close();
        });
        dlg.present();
    }

    fn cleanup_then_install(&self, game: &StoreGame) {
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
        let store = self.store.clone();
        let app_id = game.app_id.clone();
        std::thread::spawn(move || {
            let _ = tx.send(StoreManager::cleanup_stale(&store, &app_id));
        });
        let vh = self.clone();
        let game_c = game.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok(doc)) => {
                let cleaned = doc.get("cleaned").and_then(|x| x.as_bool()).unwrap_or(false);
                let reason = doc.get("reason").and_then(|x| x.as_str()).unwrap_or("");
                if cleaned || reason == "no_record" {
                    // Record dropped (or already gone): a fresh install now
                    // respects the base path instead of the stale folder.
                    vh.start_install(&game_c);
                } else {
                    vh.state_toast("Stale record", "Unexpected state; the game was not installed.");
                }
                glib::ControlFlow::Break
            }
            Ok(Err(msg)) => {
                // Guarded (disk unmounted / folder present / lock held) or
                // a real failure: never install.
                vh.state_toast("Could not clean the record", &msg);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn import_to_library(&self, title: &str, store: &str, app_id: &str, install_path: &str, exe: &str, eac: bool, battleye: bool) {
        let name = if self.state.game_model.get_game(title).is_some() {
            format!("{} ({})", title, if store == "epic" { "Epic" } else { "GOG" })
        } else {
            title.to_string()
        };
        if self.state.game_model.get_game(&name).is_some() {
            self.state_toast("Already in library", &name);
            return;
        }
        // Never create a broken entry from an empty install folder. A
        // verified installed game always arrives with its real path; a
        // stale registry entry must go through Install instead.
        if install_path.trim().is_empty() {
            self.state_toast("No install path", &format!("{} has no verified install folder. Install it first, then import.", title));
            return;
        }
        if !std::path::Path::new(install_path).exists() {
            self.state_toast("Not found on disk", &format!("{} is gone — reinstall it first.", install_path));
            return;
        }
        let main_path = install_path.to_string();
        // Empty exe: leave empty so `legendary launch` uses the manifest
        // default (a directory path here would break direct launches too).
        let executable = if exe.is_empty() {
            String::new()
        } else if exe.starts_with('/') || exe.contains(':') {
            exe.to_string()
        } else {
            format!("{}/{}", main_path.trim_end_matches('/'), exe)
        };
        let proton = self.state.config.launcher_value("defaultProton").unwrap_or_default();
        let prefix = self.state.config.base_path_for("prefixes").join(&name).display().to_string();
        let entry = crate::backend::game_model::GameEntry {
            name: name.clone(),
            executable,
            main_path,
            prefix_path: prefix,
            proton,
            overrides: String::new(),
            steam_id: String::new(),
            banner: String::new(),
            icon: String::new(),
            time_spent: 0,
            last_played: 0,
            favorite: false,
            source: crate::backend::game_model::GameSource::from_str(if store == "epic" { "Epic" } else { "GOG" }),
            executor: String::new(),
            emu_settings: std::collections::HashMap::new(),
            lutris_runner: String::new(),
            environment: String::new(),
            args_before: String::new(),
            args_after: String::new(),
            steam_overlay: false,
            steam_runtime: false,
            use_umu: false,
            use_shared_prefix: false,
            shared_prefix_name: String::new(),
            wined3d: false,
            native_wayland: false,
            game_mode: false,
            mango_hud: false,
            lutris_slug: String::new(),
            fake_steam_id: String::new(),
            install_size: String::new(),
            heroic_store: store.to_string(),
            heroic_app_id: app_id.to_string(),
            heroic_eac: eac,
            heroic_battleye: battleye,
        };
        self.state.game_model.add_game(entry);
        // Copy the saved description (descriptions.json) into the native
        // record field-by-field so the game carries its text offline.
        // add_game already persisted time_spent/last_played above and
        // set_game_value only writes this field, so nothing else is
        // overwritten.
        if store == "epic" {
            if let Some((desc, src, url, lang)) = Self::descriptions_entry(app_id) {
                let cfg = self.state.config.clone();
                cfg.set_game_value(&name, "Description", &desc);
                if !src.is_empty() {
                    cfg.set_game_value(&name, "DescriptionSource", &src);
                }
                if !url.is_empty() {
                    cfg.set_game_value(&name, "DescriptionUrl", &url);
                }
                if !lang.is_empty() {
                    cfg.set_game_value(&name, "DescriptionLang", &lang);
                }
            }
        }
        self.state.recent_model.refresh(30);
        crate::refresh_warn_buttons(&self.state);
        if let Some(ref sb) = *self.sidebar.borrow() {
            sb.apply_current_filter();
        }
        if let Some(ref c) = *self.center.borrow() {
            c.rebuild(&self.state, &self.details);
        }
        self.state_toast("Added to library", &name);
    }

    /// Boton "Import" de un juego ya instalado en Heroic. Pasa por el Import
    /// Manager antes de registrar nada: el modo test es el comportamiento
    /// actual, el permanente mueve los archivos a ~/Games.
    fn import_with_manager(&self, game: &StoreGame) {
        let cands = vec![MoveCandidate {
            label: game.title.clone(),
            store_tag: if self.store == "epic" { "Epic" } else { "GOG" }.to_string(),
            install_path: PathBuf::from(&game.install_path),
            // Heroic: el prefix_path que CorkyTux registra es suyo, no del
            // launcher, asi que no se mueve (decision D3).
            prefix_path: None,
            executable: PathBuf::from(&game.executable),
        }];
        let games_dir = import_manager::games_root();

        // El preflight recorre el arbol para medirlo, asi que va fuera del
        // hilo de GTK.
        let (tx, rx) = std::sync::mpsc::channel::<Preflight>();
        let wc = cands.clone();
        let wd = games_dir.clone();
        std::thread::spawn(move || {
            let _ = tx.send(import_move::preflight(&wc, &wd));
        });

        let vh = self.clone();
        let game_c = game.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(pf) => {
                // cands se clona: el closure es FnMut y puede correr mas de
                // una vez, asi que no se puede mover la captura.
                vh.ask_import_mode(&game_c, cands.clone(), pf);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn ask_import_mode(&self, game: &StoreGame, cands: Vec<MoveCandidate>, pf: Preflight) {
        let vh = self.clone();
        let game_c = game.clone();
        // Duenos para el closure: ask toma &pf como argumento hermano y el
        // closure tiene que seguir siendo Fn, asi que duena copias y clona
        // por invocacion.
        let cands_c = cands.clone();
        let pf_c = pf.clone();
        import_manager::ask(&self.parent, &game.title, &pf, move |mode| {
            let Some(mode) = mode else { return };
            if mode == ImportMode::Test {
                // Sin cambios: exactamente lo que hacia el boton antes.
                vh.import_to_library(
                    &game_c.title,
                    &vh.store,
                    &game_c.app_id,
                    &game_c.install_path,
                    &game_c.executable,
                    false,
                    false,
                );
                return;
            }
            vh.start_move(&game_c, cands_c.clone(), pf_c.clone(), mode);
        });
    }

    /// Modo permanente: mueve y despues registra con las rutas nuevas. El
    /// progreso es indeterminado porque la copia va con `cp -a`, que no
    /// reporta bytes; el total se muestra al final.
    fn start_move(&self, game: &StoreGame, cands: Vec<MoveCandidate>, pf: Preflight, mode: ImportMode) {
        let (bar, status) = self.progress(&format!("Moving {}", game.title));
        status.set_text("Preparing the move…");
        let games_dir = import_manager::games_root();

        let (tx, rx) = std::sync::mpsc::channel::<(ExecPlan, Vec<(String, MoveOutcome)>)>();
        std::thread::spawn(move || {
            let ep = import_move::plan_for_mode(&pf, &cands, &games_dir, mode);
            let res = import_move::execute(&ep, &games_dir);
            let _ = tx.send((ep, res));
        });

        let vh = self.clone();
        let game_c = game.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok((ep, results)) => {
                bar.set_fraction(1.0);
                vh.finish_move(&game_c, &ep, results);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Un pulso por vuelta del poll (cada 50 ms): barra viva sin
                // lanzar un timer extra que haya que cancelar despues.
                bar.pulse();
                glib::ControlFlow::Continue
            }
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn finish_move(&self, game: &StoreGame, ep: &ExecPlan, results: Vec<(String, MoveOutcome)>) {
        let mine = results
            .iter()
            .find(|(label, _)| *label == game.title)
            .map(|(_, o)| o.clone());

        let plan = ep
            .singles
            .iter()
            .find(|p| p.candidate.label == game.title)
            .or_else(|| {
                ep.groups
                    .iter()
                    .flat_map(|g| g.members.iter())
                    .find(|p| p.candidate.label == game.title)
            });

        match mine {
            Some(MoveOutcome::Failed(msg)) => {
                // No se importa nada: el original esta intacto y el usuario
                // decide si reintenta en modo test.
                self.state_toast("Could not move the game", &msg);
                return;
            }
            Some(MoveOutcome::WithWarning(msg)) => {
                self.state_toast("Moved with a warning", &msg);
            }
            _ => {}
        }

        let new_install = plan
            .map(|p| p.new_install_path())
            .unwrap_or_else(|| PathBuf::from(&game.install_path));
        let new_exe = import_manager::remap_exe(&game.executable, &game.install_path, &new_install);
        let moved = plan.map(|p| p.bytes).unwrap_or(0);
        if moved > 0 {
            self.state_toast(
                "Moved to Games",
                &format!("{} ({})", new_install.display(), import_move::human_bytes(moved)),
            );
        }
        self.import_to_library(
            &game.title,
            &self.store,
            &game.app_id,
            new_install.to_string_lossy().as_ref(),
            &new_exe,
            false,
            false,
        );
    }

    fn progress(&self, title: &str) -> (gtk::ProgressBar, gtk::Label) {
        let dlg = adw::Dialog::new();
        dlg.set_title(title);
        dlg.set_content_width(400);
        let (header, x_btn) = helpers::modal_header(title);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_top(12);
        body.set_margin_bottom(16);
        body.set_margin_start(16);
        body.set_margin_end(16);
        let status = note("Starting…");
        body.append(&status);
        let bar = gtk::ProgressBar::new();
        bar.set_show_text(true);
        body.append(&bar);
        let hide = gtk::Button::with_label("Hide");
        hide.add_css_class("settings-btn");
        body.append(&hide);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        {
            let d = dlg.clone();
            hide.connect_clicked(move |_| { d.close(); });
        }
        dlg.present(Some(&self.parent));
        (bar, status)
    }
}


/// Ejecuta `webdriver_login` y devuelve su código, o el motivo del fallo.
///
/// Vive en un hilo propio porque el helper puede tardar hasta 180 s: el hilo
/// se queda esperando en `output()` y el bucle de GTK sigue respondiendo igual.
///
/// La causa de que el proceso se quede colgado sin escribir nada la cubre el
/// watchdog del llamador, no este `wait()`.
fn spawn_login_helper(
    url: String,
) -> std::sync::mpsc::Receiver<Result<String, (String, String)>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_login_helper(&url));
    });
    rx
}

fn run_login_helper(url: &str) -> Result<String, (String, String)> {
    let bin = login_helper_path().map_err(|e| ("helper".to_string(), e))?;
    // stderr va a un archivo, no a una tubería: así no puede haber interbloqueo
    // por descriptores llenos y además queda el registro para diagnosticar.
    let log = login_helper_log_path();
    let stderr = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log)
        .ok();

    let out = std::process::Command::new(&bin)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(stderr.map_or_else(std::process::Stdio::null, std::process::Stdio::from))
        .output()
        .map_err(|e| {
            (
                "helper".to_string(),
                format!("could not start {}: {}", bin.display(), e),
            )
        })?;

    if !out.status.success() {
        let raw = String::from_utf8_lossy(&out.stderr).trim().to_string();
        // El helper responde `ERRO:<motivo>:<mensaje>`. Se separan los dos
        // campos para poder dar un mensaje distinto por causa.
        let (why, msg) = match raw.strip_prefix("ERRO:") {
            Some(rest) => match rest.split_once(':') {
                Some((w, m)) => (w.to_string(), m.to_string()),
                None => (rest.to_string(), String::new()),
            },
            None => (
                "helper".to_string(),
                if raw.is_empty() {
                    format!("exited with {} (see {})", out.status, log.display())
                } else {
                    raw
                },
            ),
        };
        return Err((why, msg));
    }

    let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if code.is_empty() {
        return Err((
            "session".to_string(),
            "the helper finished without giving a code".to_string(),
        ));
    }
    Ok(code)
}

/// Dónde está el binario helper.
///
/// Vive al lado del launcher: en desarrollo los dos están en `target/debug/` y
/// en una instalación los dos en `~/.local/share/corkytux/`. El override por
/// entorno existe para poder probar otro binario sin recompilar.
///
/// `pub(crate)` porque el modal de dependencias lo necesita para `--prefetch`.
pub(crate) fn login_helper_path() -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("CORKYTUX_LOGIN_HELPER") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
        return Err(format!("CORKYTUX_LOGIN_HELPER is not a file: {}", p.display()));
    }
    let name = "webdriver_login";
    let mut tried: Vec<String> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(name);
            if p.is_file() {
                return Ok(p);
            }
            tried.push(p.display().to_string());
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home).join(".local/share/corkytux").join(name);
        if p.is_file() {
            return Ok(p);
        }
        tried.push(p.display().to_string());
    }
    Err(format!(
        "the store login helper ({}) is missing; looked in: {}",
        name,
        tried.join(", ")
    ))
}

/// Registro de stderr del helper, para poder diagnosticar un login fallido.
fn login_helper_log_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = PathBuf::from(home).join(".local/share/corkytux/logs");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("login-helper.log")
}

// ─── dependencias de Stores (modal "Install dependencies") ─────────────────

/// Resultado del chequeo de entrada a Stores: pre-chequeo barato primero,
/// chequeo completo solo si el pre-chequeo marcó algo.
enum Chequeo {
    TodoBien,
    Falta(Faltantes),
}

/// Qué herramientas faltan para Stores. Todo el modal, el trigger y el estado
/// "Setup incomplete" se deciden con esto; no hay banderas de sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Faltantes {
    /// Falta el binario de Epic (legendary).
    legendary: bool,
    /// Falta el binario de GOG (gogdl).
    gogdl: bool,
    /// Falta la caché del Chromium de login.
    chromium: bool,
}

impl Faltantes {
    /// El modal aparece si y solo si falta algo. Sin memoria: el skip no deja
    /// marca, así que reentrar a Stores con algo pendiente lo muestra de nuevo.
    fn hay_algo(&self) -> bool {
        self.legendary || self.gogdl || self.chromium
    }

    /// Nombres a mostrar para una tienda ("epic"/"gog"): su binario si falta,
    /// más el navegador si falta. Orden fijo: binario, navegador.
    fn para_tienda(&self, store: &str) -> Vec<&'static str> {
        let mut v = Vec::new();
        if store == "epic" && self.legendary {
            v.push("legendary");
        }
        if store == "gog" && self.gogdl {
            v.push("gogdl");
        }
        if self.chromium {
            v.push("login browser");
        }
        v
    }

    /// Sin binario no hay nada que mostrar en el tab: ni login ni biblioteca.
    fn tab_bloqueado(&self, store: &str) -> bool {
        (store == "epic" && self.legendary) || (store == "gog" && self.gogdl)
    }

    /// Sin navegador solo se bloquea el login; lo ya logueado sigue andando.
    fn login_bloqueado(&self) -> bool {
        self.chromium
    }

    /// Filas del modal en orden DEPS: solo las que faltan.
    fn filas_modal(&self) -> Vec<DepId> {
        let mut v = Vec::new();
        if self.legendary {
            v.push(DepId::Legendary);
        }
        if self.gogdl {
            v.push(DepId::Gogdl);
        }
        if self.chromium {
            v.push(DepId::Chromium);
        }
        v
    }
}

/// Corre `webdriver_login --probe` y devuelve si el Chromium está cacheado.
///
/// `None` si el helper falta o responde ilegible: el llamador lo trata como
/// faltante (el modal ofrece instalarlo) en vez de asumir que está.
fn probe_chrome_cached() -> Option<bool> {
    let bin = login_helper_path().ok()?;
    let out = std::process::Command::new(&bin)
        .arg("--probe")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v.get("cached")?.as_bool()
}

/// Reúne qué falta: bins del plugin (`status` sin `--quick`, chequeo real,
/// sin la caché de 60 s) + caché de Chromium (`--probe`). Nada se reimplementa.
fn faltantes_actuales() -> Faltantes {
    let (leg_ok, gog_ok) = match StoreManager::status(false) {
        Ok(st) => {
            let bins = st.get("bins");
            let leg = bins
                .and_then(|b| b.get("epic"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let gog = bins
                .and_then(|b| b.get("gog"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            (leg, gog)
        }
        // Si el status falla no se puede afirmar que estén: se ofrecen.
        Err(_) => (false, false),
    };
    Faltantes {
        legendary: !leg_ok,
        gogdl: !gog_ok,
        chromium: match probe_chrome_cached() {
            Some(c) => !c,
            None => true,
        },
    }
}

/// Pre-chequeo liviano de entrada a Stores, sin red: presencia de bins
/// (subcomando `bins` del plugin, solo filesystem) + caché de Chromium
/// (`--probe`, solo filesystem). Es lo único que corre en cada entrada;
/// el chequeo completo con red solo sigue si esto marca algo.
fn precheck_actual() -> Faltantes {
    let (leg_ok, gog_ok) = match StoreManager::bins() {
        Ok(v) => {
            let bins = v.get("bins");
            let leg = bins
                .and_then(|b| b.get("legendary"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let gog = bins
                .and_then(|b| b.get("gogdl"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            (leg, gog)
        }
        // Si ni el `bins` responde no se puede afirmar que estén: se ofrecen.
        Err(_) => (false, false),
    };
    Faltantes {
        legendary: !leg_ok,
        gogdl: !gog_ok,
        chromium: match probe_chrome_cached() {
            Some(c) => !c,
            None => true,
        },
    }
}

/// Traduce el motivo de fallo del helper a un mensaje accionable.
///
/// Cada motivo que el helper puede devolver tiene su propio mensaje, y todos
/// dicen qué hacer y terminan pidiendo reintentar. Ninguno sugiere desactivar
/// nada: el perfil que usa el helper es efímero y no carga las extensiones ni
/// las protecciones del navegador del usuario.
fn login_failure_message(why: &str, detail: &str) -> String {
    let base = match why {
        "chrome" => "CorkyTux could not prepare the browser it uses to show the login \
                     window. On first use it downloads its own Chromium, so this usually \
                     means the download was blocked or there is no disk space. Press Log in \
                     to retry."
            .to_string(),
        "timeout" => "The sign-in did not finish in 3 minutes. Press Log in to try again. \
                      If the window is still open, finish signing in there before retrying."
            .to_string(),
        "nav" => "The store's login page could not be opened. Check your connection and \
                  press Log in to retry."
            .to_string(),
        "session" => "The browser opened but CorkyTux could not attach to it. Close any \
                      browser window that is already showing a store login and press Log in \
                      to retry."
            .to_string(),
        "cancelado" => "The login was cancelled and the browser was closed. Press Log in \
                        to start again."
            .to_string(),
        "cdp_perdido" => "The connection to the login window was lost and could not be \
                          re-established. Press Log in to start again."
            .to_string(),
        _ => "The automated store login failed. Press Log in to retry.".to_string(),
    };
    // El detalle solo se añade si aporta algo: el motivo de un timeout ya
    // incluye la última URL vista, que es lo que hace falta para diagnosticar.
    if detail.trim().is_empty() {
        base
    } else {
        format!("{} ({})", base, detail.trim())
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// Falta solo legendary: Epic bloqueado (binario + navegador pendientes),
    /// GOG solo sin login.
    #[test]
    fn granularidad_falta_solo_legendary() {
        let f = Faltantes { legendary: true, gogdl: false, chromium: true };
        assert!(f.hay_algo());
        assert!(f.tab_bloqueado("epic"));
        assert!(!f.tab_bloqueado("gog"));
        assert!(f.login_bloqueado());
        assert_eq!(f.para_tienda("epic"), vec!["legendary", "login browser"]);
        assert_eq!(f.para_tienda("gog"), vec!["login browser"]);
        assert_eq!(f.filas_modal(), vec![DepId::Legendary, DepId::Chromium]);
    }

    /// Falta solo gogdl: espejo del caso Epic.
    #[test]
    fn granularidad_falta_solo_gogdl() {
        let f = Faltantes { legendary: false, gogdl: true, chromium: true };
        assert!(!f.tab_bloqueado("epic"));
        assert!(f.tab_bloqueado("gog"));
        assert_eq!(f.para_tienda("epic"), vec!["login browser"]);
        assert_eq!(f.para_tienda("gog"), vec!["gogdl", "login browser"]);
        assert_eq!(f.filas_modal(), vec![DepId::Gogdl, DepId::Chromium]);
    }

    /// Falta solo Chromium: ningún tab bloqueado, solo el login. La
    /// biblioteca de lo ya logueado sigue visible.
    #[test]
    fn granularidad_falta_solo_chromium() {
        let f = Faltantes { legendary: false, gogdl: false, chromium: true };
        assert!(f.hay_algo());
        assert!(!f.tab_bloqueado("epic"));
        assert!(!f.tab_bloqueado("gog"));
        assert!(f.login_bloqueado());
        assert_eq!(f.filas_modal(), vec![DepId::Chromium]);
    }

    /// Falta todo: ambos tabs bloqueados, tres filas en orden.
    #[test]
    fn granularidad_falta_todo() {
        let f = Faltantes { legendary: true, gogdl: true, chromium: true };
        assert!(f.tab_bloqueado("epic"));
        assert!(f.tab_bloqueado("gog"));
        assert_eq!(f.filas_modal(), vec![DepId::Legendary, DepId::Gogdl, DepId::Chromium]);
    }

    /// Falta SOLO gogdl (legendary y Chromium presentes): una fila, GOG
    /// bloqueado, Epic intacto con login.
    #[test]
    fn granularidad_falta_solo_gogdl_sin_nada_mas() {
        let f = Faltantes { legendary: false, gogdl: true, chromium: false };
        assert!(f.hay_algo());
        assert!(!f.tab_bloqueado("epic"));
        assert!(f.tab_bloqueado("gog"));
        assert!(!f.login_bloqueado());
        assert_eq!(f.para_tienda("epic"), Vec::<&str>::new());
        assert_eq!(f.para_tienda("gog"), vec!["gogdl"]);
        assert_eq!(f.filas_modal(), vec![DepId::Gogdl]);
    }

    /// Falta SOLO legendary: espejo del caso GOG.
    #[test]
    fn granularidad_falta_solo_legendary_sin_nada_mas() {
        let f = Faltantes { legendary: true, gogdl: false, chromium: false };
        assert!(f.tab_bloqueado("epic"));
        assert!(!f.tab_bloqueado("gog"));
        assert!(!f.login_bloqueado());
        assert_eq!(f.para_tienda("epic"), vec!["legendary"]);
        assert_eq!(f.filas_modal(), vec![DepId::Legendary]);
    }

    /// Ciclo fallo→skip→reentrada→modal: el skip no deja marca persistente,
    /// así que reentrar con algo pendiente vuelve a pedir el modal. Solo
    /// completar (nada faltante) lo silencia.
    #[test]
    fn ciclo_skip_no_silencia_el_modal() {
        let pendiente = Faltantes { legendary: false, gogdl: true, chromium: true };
        assert!(pendiente.hay_algo(), "antes del skip: modal");
        // El skip no muta nada persistente: mismo valor, mismo modal.
        let tras_skip = pendiente;
        assert!(tras_skip.hay_algo(), "tras el skip: modal de nuevo al reentrar");
        let completo = Faltantes::default();
        assert!(!completo.hay_algo(), "completo: nunca más");
        assert!(completo.filas_modal().is_empty());
    }

    /// Escenario del bug del trigger: todo instalado → los archivos
    /// desaparecen por fuera del modal → re-chequeo → se detecta la falta y
    /// hay que mostrar el modal. (El `deps_ok` cacheado impedía re-chequear;
    /// eliminado: cada entrada evalúa de nuevo.)
    ///
    /// Hermético y sin red: HOME temporal + copia del script real del plugin
    /// + helper real localizado junto al binario de test. Toca procesos
    /// reales (`bins` en Python, `--probe` en Rust) contra ese sandbox.
    #[test]
    fn precheck_detecta_borrado_externo_y_dispara_modal() {
        use std::os::unix::fs::PermissionsExt;

        let orig_home = std::env::var("HOME").unwrap_or_default();
        let orig_helper = std::env::var("CORKYTUX_LOGIN_HELPER").ok();
        let base = std::env::temp_dir().join(format!(
            "corkytux-precheck-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);

        // Sandbox: copia del script del plugin + bins + caché de Chromium.
        let script_origen = std::path::PathBuf::from(&orig_home)
            .join(".local/share/CorkyTux/plugins/heroic-store/heroic-store");
        assert!(
            script_origen.is_file(),
            "falta el plugin instalado para copiar al sandbox: {}",
            script_origen.display()
        );
        let script = base.join(".local/share/CorkyTux/plugins/heroic-store/heroic-store");
        std::fs::create_dir_all(script.parent().expect("padre del script")).unwrap();
        std::fs::copy(&script_origen, &script).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let bin_dir = base.join(".config/CorkyTux/plugins/heroic-store/bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let leg = bin_dir.join("legendary");
        let gog = bin_dir.join("gogdl");
        std::fs::write(&leg, b"x").unwrap();
        std::fs::write(&gog, b"x").unwrap();

        // Helper real: el binario junto al harness de test (lo construye
        // `cargo test` al compilar todos los targets).
        let exe = std::env::current_exe().expect("current_exe");
        let helper = exe
            .parent()
            .and_then(|d| d.parent())
            .map(|d| d.join("webdriver_login"))
            .expect("ruta del helper");
        assert!(
            helper.is_file(),
            "falta el helper compilado para --probe: {}",
            helper.display()
        );
        let chrome = base.join(
            ".local/share/corkytux/chrome-154.0.8037.57/chrome-linux64/chrome",
        );
        std::fs::create_dir_all(chrome.parent().expect("padre de chrome")).unwrap();
        std::fs::write(&chrome, b"x").unwrap();

        std::env::set_var("HOME", &base);
        std::env::set_var("CORKYTUX_LOGIN_HELPER", &helper);

        // Estado inicial: todo instalado → sin modal.
        let antes = precheck_actual();
        assert!(
            !antes.hay_algo(),
            "con todo presente no hay modal: {:?}",
            antes
        );

        // Borrado externo, sin pasar por el modal (el escenario del bug).
        std::fs::remove_file(&leg).unwrap();
        std::fs::remove_file(&gog).unwrap();
        std::fs::remove_file(&chrome).unwrap();

        // Re-chequeo: detecta las tres faltas → modal.
        let despues = precheck_actual();
        assert!(
            despues.hay_algo(),
            "tras el borrado externo hay modal: {:?}",
            despues
        );
        assert_eq!(
            despues,
            Faltantes { legendary: true, gogdl: true, chromium: true },
            "las tres faltas detectadas"
        );
        assert_eq!(despues.filas_modal().len(), 3);

        match orig_helper {
            Some(v) => std::env::set_var("CORKYTUX_LOGIN_HELPER", v),
            None => std::env::remove_var("CORKYTUX_LOGIN_HELPER"),
        }
        std::env::set_var("HOME", &orig_home);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// meta_line: cada mitad se oculta si está vacía (campos opcionales).
    #[test]
    fn meta_line_oculta_mitades_vacias() {
        assert_eq!(
            super::StorePageHandle::meta_line("RPG", &["Windows".to_string()]),
            "RPG · Windows"
        );
        assert_eq!(
            super::StorePageHandle::meta_line("", &["Linux".to_string()]),
            "Linux"
        );
        assert_eq!(
            super::StorePageHandle::meta_line("Adv", &[]),
            "Adv"
        );
        assert_eq!(
            super::StorePageHandle::meta_line("  ", &[String::new()]),
            ""
        );
    }

    /// filtrar_ordenar: filtro por título/categoría + 3 órdenes.
    #[test]
    fn filtrar_ordenar_filtra_y_ordena() {
        let g = |t: &str, c: &str, inst: bool| super::StoreGame {
            app_id: t.to_string(),
            title: t.to_string(),
            category: c.to_string(),
            systems: Vec::new(),
            version: String::new(),
            installed: inst,
            stale_registry: false,
            install_path: String::new(),
            executable: String::new(),
            cover: String::new(),
            description: String::new(),
        };
        let juegos = vec![g("Zeta", "RPG", false), g("alpha", "Adv", true), g("Mid", "RPG", false)];
        // Vacío + default: todo A–Z insensible a mayúsculas.
        let v = super::StorePageHandle::filtrar_ordenar(&juegos, "", 0);
        assert_eq!(v.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), vec!["alpha", "Mid", "Zeta"]);
        // Filtro matchea título o categoría.
        let v = super::StorePageHandle::filtrar_ordenar(&juegos, "rpg", 0);
        assert_eq!(v.len(), 2);
        // Z–A.
        let v = super::StorePageHandle::filtrar_ordenar(&juegos, "", 1);
        assert_eq!(v[0].title, "Zeta");
        // Instalados primero.
        let v = super::StorePageHandle::filtrar_ordenar(&juegos, "", 2);
        assert!(v[0].installed);
    }
}
