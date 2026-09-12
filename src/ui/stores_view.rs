use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::AppState;
use crate::backend::external::StoreManager;
use crate::ui::helpers;

#[derive(Clone)]
struct StoreGame {
    app_id: String,
    title: String,
    version: String,
    installed: bool,
    cover: String,
    description: String,
}



pub struct StoresView {
    pub widget: gtk::ScrolledWindow,
}

impl Clone for StoresView {
    fn clone(&self) -> Self {
        Self { widget: self.widget.clone() }
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
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
    inner.set_margin_top(8);
    inner.set_margin_bottom(8);
    inner.set_margin_start(10);
    inner.set_margin_end(10);
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
        let col = gtk::Box::new(gtk::Orientation::Vertical, 10);
        col.set_margin_top(24);
        col.set_margin_bottom(24);
        col.set_margin_start(24);
        col.set_margin_end(24);
        col.set_hexpand(true);
        scroll.set_child(Some(&col));

        // header
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let hicon = gtk::Image::from_icon_name("system-software-install-symbolic");
        hicon.set_pixel_size(28);
        head.append(&hicon);
        let title = gtk::Label::new(Some("Stores"));
        title.add_css_class("details-title");
        title.set_halign(gtk::Align::Start);
        title.set_hexpand(true);
        head.append(&title);
        let setup_btn = gtk::Button::with_label("Install tools");
        setup_btn.set_tooltip_text(Some("Download legendary + gogdl (independent from Heroic)"));
        setup_btn.add_css_class("settings-btn");
        setup_btn.set_valign(gtk::Align::Center);
        head.append(&setup_btn);
        head.add_css_class("mc-head");
        col.append(&head);
        let setup_status = note("");
        setup_status.set_visible(false);
        col.append(&setup_status);
        // Filled with the per-store handles below; setup success refreshes
        // both account badges (no more stale "Tools missing").
        let handles_slot: Rc<RefCell<Vec<StorePageHandle>>> = Rc::new(RefCell::new(Vec::new()));
        {
            let st = setup_status.clone();
            let slot_c = handles_slot.clone();
            setup_btn.connect_clicked(move |_| {
                st.set_visible(true);
                st.set_text("Downloading legendary + gogdl…");
                let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
                std::thread::spawn(move || {
                    let _ = tx.send(StoreManager::setup().map(|d| {
                        let got = d.get("installed").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
                        format!("Tools ready ({}).", got)
                    }).map_err(|e| e.to_string()));
                });
                let sc = st.clone();
                let slot_cc = slot_c.clone();
                crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
                    Ok(Ok(msg)) => {
                        sc.set_text(&msg);
                        for h in slot_cc.borrow().iter() {
                            h.refresh_auth();
                            h.refresh_library(true);
                        }
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        sc.set_text(&format!("Setup failed: {}", e));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        let status = note("Epic + GOG via legendary/gogdl (Heroic pattern). Games install into the native library.");
        col.append(&status);

        // tabs
        let tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let stack = gtk::Stack::new();
        stack.set_vexpand(false);
        let mut btns: Vec<gtk::ToggleButton> = Vec::new();
        let mut inds: Vec<gtk::Box> = Vec::new();
        for (label, _id, icon) in [("Epic Games", "epic", "application-x-executable-symbolic"), ("GOG", "gog", "application-x-executable-symbolic")] {
            let wrap = gtk::Box::new(gtk::Orientation::Vertical, 1);
            wrap.set_hexpand(true);
            let btn = gtk::ToggleButton::new();
            btn.add_css_class("settings-tab");
            btn.set_hexpand(true);
            let c = gtk::Box::new(gtk::Orientation::Vertical, 2);
            c.set_halign(gtk::Align::Center);
            let im = gtk::Image::from_icon_name(icon);
            im.set_pixel_size(18);
            c.append(&im);
            let lbl = gtk::Label::new(Some(label));
            lbl.add_css_class("time-label");
            c.append(&lbl);
            let ind = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            ind.add_css_class("settings-tab-indicator");
            ind.set_visible(label == "Epic Games");
            c.append(&ind);
            btn.set_child(Some(&c));
            if label == "Epic Games" {
                btn.set_active(true);
            }
            wrap.append(&btn);
            tabbar.append(&wrap);
            btns.push(btn);
            inds.push(ind);
        }
        if btns.len() == 2 {
            btns[1].set_group(Some(&btns[0]));
        }
        let ids = ["epic", "gog"];
        col.append(&tabbar);

        let mut handles: Vec<StorePageHandle> = Vec::new();
        for store in ["epic", "gog"] {
            let (page, handle) = Self::store_page(state, parent, sidebar, center, details, store);
            stack.add_named(&page, Some(store));
            handles.push(handle);
        }
        // Lazy: libraries load on first tab show, never at startup (a slow
        // `legendary list` used to pin the main loop at 100% on launch).
        for (i, id) in ids.iter().enumerate() {
            let h = handles.get(i).cloned();
            let tid = id.to_string();
            let st = stack.clone();
            let all = inds.clone();
            let mine = inds[i].clone();
            let b0 = btns[i].clone();
            b0.connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name(&tid);
                    for ind in &all {
                        ind.set_visible(false);
                    }
                    mine.set_visible(true);
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
        Self { widget: scroll }
    }

    fn store_page(
        state: &AppState,
        parent: &adw::ApplicationWindow,
        sidebar: &Rc<RefCell<Option<crate::ui::sidebar::Sidebar>>>,
        center: &Rc<RefCell<Option<crate::ui::center::CenterHandle>>>,
        details: &Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>>,
        store: &str,
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
        let code_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let code_entry = gtk::Entry::new();
        code_entry.set_placeholder_text(Some("Or paste code manually"));
        code_entry.set_hexpand(true);
        code_row.append(&code_entry);
        let code_btn = gtk::Button::with_label("Confirm code");
        code_btn.add_css_class("add-btn");
        code_row.append(&code_btn);
        login_box.append(&code_row);
        auth_inner.append(&login_box);
        // logged-in session row: avatar + name + logout
        let acc_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let acc_icon = gtk::Image::from_icon_name("avatar-default-symbolic");
        acc_icon.set_pixel_size(32);
        acc_row.append(&acc_icon);
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
            let promo_list = gtk::Box::new(gtk::Orientation::Vertical, 8);
            let promo_scroll = gtk::ScrolledWindow::new();
            promo_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            promo_scroll.set_min_content_height(180);
            promo_scroll.set_max_content_height(480);
            promo_scroll.set_propagate_natural_height(true);
            promo_scroll.set_vexpand(false);
            promo_scroll.set_child(Some(&promo_list));
            promo_inner.append(&promo_scroll);
            page.append(&promo_frame);
            let (tx, rx) = std::sync::mpsc::channel::<Vec<(String, String, String, String)>>();
            std::thread::spawn(move || {
                let _ = tx.send(StoreManager::free_promos().ok()
                    .and_then(|d| d.get("free_now").cloned())
                    .and_then(|v| v.as_array().cloned()).unwrap_or_default()
                    .into_iter().filter_map(|p| {
                        Some((p.get("title")?.as_str()?.to_string(),
                              p.get("description")?.as_str().unwrap_or("").to_string(),
                              p.get("cover")?.as_str().unwrap_or("").to_string(),
                              p.get("store_url")?.as_str().unwrap_or("").to_string()))
                    }).collect::<Vec<_>>());
            });
            let st = state.clone();
            crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
                Ok(list) => {
                    while let Some(c) = promo_list.first_child() {
                        promo_list.remove(&c);
                    }
                    if list.is_empty() {
                        promo_lbl.set_text("Nothing free right now.");
                    } else {
                        promo_lbl.set_text("");
                        for (t, d, cover, u) in list {
                            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                            if !cover.is_empty() {
                                let img = gtk::Image::new();
                                img.set_pixel_size(52);
                                img.set_valign(gtk::Align::Center);
                                crate::ui::minecraft_view::load_mod_icon(&cover, &format!("promo-{}", t), &img, 52);
                                row.append(&img);
                            }
                            let mid = gtk::Box::new(gtk::Orientation::Vertical, 2);
                            mid.set_hexpand(true);
                            let lbl = gtk::Label::new(Some(&t));
                            lbl.set_halign(gtk::Align::Start);
                            lbl.add_css_class("details-title");
                            mid.append(&lbl);
                            if !d.is_empty() {
                                let dl = gtk::Label::new(Some(&d));
                                dl.set_halign(gtk::Align::Start);
                                dl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                dl.set_max_width_chars(60);
                                dl.set_opacity(0.6);
                                dl.add_css_class("time-label");
                                mid.append(&dl);
                            }
                            row.append(&mid);
                            if !u.is_empty() {
                                let claim = gtk::Button::with_label("Claim free");
                                claim.add_css_class("add-btn");
                                claim.set_valign(gtk::Align::Center);
                                let stc = st.clone();
                                let uc = u.clone();
                                claim.connect_clicked(move |_| { stc.integration.open_url(&uc); });
                                row.append(&claim);
                            }
                            promo_list.append(&row);
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
            let deals_list = gtk::Box::new(gtk::Orientation::Vertical, 8);
            let deals_scroll = gtk::ScrolledWindow::new();
            deals_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            deals_scroll.set_min_content_height(300);
            deals_scroll.set_max_content_height(640);
            deals_scroll.set_propagate_natural_height(true);
            deals_scroll.set_vexpand(false);
            deals_scroll.set_child(Some(&deals_list));
            deals_inner.append(&deals_scroll);
            page.append(&deals_frame);
            {
                // Paged deals + infinite scroll: 40 per page over the full
                // on-sale catalog (~2000). Titles deduped across pages
                // (verified promos repeat inside catalog pages).
                let seen: Rc<RefCell<std::collections::HashSet<String>>> =
                    Rc::new(RefCell::new(std::collections::HashSet::new()));
                let next: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(0));
                let total: Rc<std::cell::Cell<u64>> = Rc::new(std::cell::Cell::new(u64::MAX));
                let loading: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
                let shown: Rc<std::cell::Cell<usize>> = Rc::new(std::cell::Cell::new(0));
                let loader: Rc<RefCell<Option<Rc<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
                {
                    let list_c = deals_list.clone();
                    let lbl_c = deals_lbl.clone();
                    let seen_c = seen.clone();
                    let next_c = next.clone();
                    let total_c = total.clone();
                    let loading_c = loading.clone();
                    let shown_c = shown.clone();
                    let st_c = state.clone();
                    *loader.borrow_mut() = Some(Rc::new(move |start: u64| {
                        if loading_c.get() {
                            return;
                        }
                        loading_c.set(true);
                        let (tx, rx) = std::sync::mpsc::channel::<serde_json::Value>();
                        std::thread::spawn(move || {
                            let _ = tx.send(StoreManager::epic_deals(start, 40).unwrap_or_default());
                        });
                        let list_cc = list_c.clone();
                        let lbl_cc = lbl_c.clone();
                        let seen_cc = seen_c.clone();
                        let next_cc = next_c.clone();
                        let total_cc = total_c.clone();
                        let loading_cc = loading_c.clone();
                        let shown_cc = shown_c.clone();
                        let st_cc = st_c.clone();
                        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
                            Ok(doc) => {
                                let arr = doc.get("deals").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                if doc.get("deals").is_none() {
                                    // Backend error: stop paging.
                                    total_cc.set(next_cc.get());
                                } else {
                                    next_cc.set(doc.get("next").and_then(|v| v.as_u64()).unwrap_or(start));
                                    total_cc.set(doc.get("total").and_then(|v| v.as_u64()).unwrap_or(next_cc.get()));
                                }
                                let mut added = 0usize;
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
                                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                    if !cover.is_empty() {
                                        let img = gtk::Image::new();
                                        img.set_pixel_size(52);
                                        img.set_valign(gtk::Align::Center);
                                        crate::ui::minecraft_view::load_mod_icon(&cover, &format!("deal-{}", t), &img, 52);
                                        row.append(&img);
                                    }
                                    let mid = gtk::Box::new(gtk::Orientation::Vertical, 2);
                                    mid.set_hexpand(true);
                                    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                                    let lbl = gtk::Label::new(Some(&t));
                                    lbl.set_halign(gtk::Align::Start);
                                    lbl.add_css_class("details-title");
                                    top.append(&lbl);
                                    let badge = gtk::Label::new(Some(&format!("-{}%", pct)));
                                    badge.add_css_class("proton-path-badge");
                                    top.append(&badge);
                                    mid.append(&top);
                                    if !d.is_empty() {
                                        let dl = gtk::Label::new(Some(&d));
                                        dl.set_halign(gtk::Align::Start);
                                        dl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                        dl.set_max_width_chars(60);
                                        dl.set_opacity(0.6);
                                        dl.add_css_class("time-label");
                                        mid.append(&dl);
                                    }
                                    let subtext = if ends.is_empty() {
                                        format!("{} (was {})", price, base)
                                    } else {
                                        format!("{} (was {}) • ends {}", price, base, ends)
                                    };
                                    let sub = gtk::Label::new(Some(&subtext));
                                    sub.set_halign(gtk::Align::Start);
                                    sub.set_opacity(0.6);
                                    sub.add_css_class("time-label");
                                    mid.append(&sub);
                                    row.append(&mid);
                                    if !u.is_empty() {
                                        let buy = gtk::Button::with_label("View deal");
                                        buy.add_css_class("add-btn");
                                        buy.set_valign(gtk::Align::Center);
                                        let stc = st_cc.clone();
                                        let uc = u.clone();
                                        buy.connect_clicked(move |_| { stc.integration.open_url(&uc); });
                                        row.append(&buy);
                                    }
                                    list_cc.append(&row);
                                    added += 1;
                                }
                                shown_cc.set(shown_cc.get() + added);
                                loading_cc.set(false);
                                let s = shown_cc.get();
                                if s == 0 {
                                    lbl_cc.set_text("No offers right now.");
                                } else if next_cc.get() < total_cc.get() {
                                    lbl_cc.set_text(&format!("Showing {} of ~{} — scroll for more", s, total_cc.get()));
                                } else {
                                    lbl_cc.set_text(&format!("Showing all {} offers", s));
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(_) => glib::ControlFlow::Break,
                        });
                    }) as Rc<dyn Fn(u64)>);
                }
                if let Some(f) = loader.borrow().as_ref() {
                    f(0);
                }
                {
                    let adj = deals_scroll.vadjustment();
                    let load_c = loader.clone();
                    let next_c = next.clone();
                    let total_c = total.clone();
                    let loading_c = loading.clone();
                    adj.connect_value_changed(move |a| {
                        if loading_c.get() || next_c.get() >= total_c.get() {
                            return;
                        }
                        if a.value() + a.page_size() >= a.upper() - 300.0 {
                            if let Some(f) = load_c.borrow().as_ref() {
                                f(next_c.get());
                            }
                        }
                    });
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
            loaded: Rc::new(std::cell::Cell::new(false)),
        };

        // auth wiring: embedded login (Heroic-style, auto-captures the
        // code) — no external browser tabs at all.
        {
            let vh = view.clone();
            login_btn.connect_clicked(move |_| {
                vh.show_embedded_login();
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
                        vv.refresh_auth();
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
            let code_e = code_entry.clone();
            code_btn.connect_clicked(move |_| {
                let code = code_e.text().to_string().trim().to_string();
                code_e.set_text("");
                if code.is_empty() {
                    vh.state_toast("Code required", "Paste the code from the browser first.");
                    return;
                }
                vh.do_login(&code);
            });
        }
        {
            let vh = view.clone();
            refresh_btn.connect_clicked(move |_| vh.refresh_library(false));
        }
        view.refresh_auth();
        (page, view)
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
    loaded: Rc<std::cell::Cell<bool>>,
}

impl StorePageHandle {
    fn state_toast(&self, heading: &str, body: &str) {
        helpers::present_msg(&self.parent, heading, body);
    }

    fn refresh_auth(&self) {
        self.auth_badge.set_text("Checking login…");
        let (tx, rx) = std::sync::mpsc::channel::<(bool, bool, String)>();
        let store = self.store.clone();
        std::thread::spawn(move || {
            let st = StoreManager::status().unwrap_or_default();
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
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok((is_logged, has_bin, name)) => {
                if !has_bin {
                    badge.set_text("Tools missing.");
                    hint.set_visible(true);
                    hint.set_text("Press Install tools above to download legendary + gogdl.");
                    login_box.set_visible(true);
                    acc_row.set_visible(false);
                } else if is_logged {
                    login_box.set_visible(false);
                    acc_row.set_visible(true);
                    acc_name.set_text(if name.is_empty() { &store } else { &name });
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
        if self.loaded.get() {
            return;
        }
        self.loaded.set(true);
        self.refresh_library(false);
    }

    fn show_embedded_login(&self) {
        // Login window runs in the plugin (PyGObject WebKit): it grabs the
        // code automatically, no copy-paste, no duplicate browser tabs.
        let rx = StoreManager::spawn_login_window(self.store.clone());
        let vh = self.clone();
        vh.auth_hint.set_visible(true);
        vh.auth_hint.set_text("Login window opened — sign in there, the code is captured automatically.");
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Custom(val) => {
                    if val.get("type").and_then(|x| x.as_str()) == Some("code") {
                        if let Some(code) = val.get("code").and_then(|x| x.as_str()) {
                            if !code.is_empty() {
                                vh.do_login(code);
                            }
                        }
                    }
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    if let Some(code) = val.get("code").and_then(|x| x.as_str()) {
                        if !code.is_empty() {
                            vh.do_login(code);
                        }
                    }
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    vh.auth_hint.set_visible(true);
                    vh.auth_hint.set_text(&format!("Login window: {}", message));
                    false
                }
                _ => true,
            }
        });
    }

    fn do_login(&self, code: &str) {
        let rx = StoreManager::spawn_auth(self.store.clone(), code.to_string());
        let vh = self.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Custom(val) => {
                    if let Some(url) = val.get("url").and_then(|x| x.as_str()) {
                        vh.state.integration.open_url(url);
                        vh.auth_hint.set_visible(true);
                        if let Some(instr) = val.get("instructions").and_then(|x| x.as_str()) {
                            vh.auth_hint.set_text(&format!("Tab opened — use THAT tab (don't click Log in again). {}", instr));
                        } else {
                            vh.auth_hint.set_text("Tab opened — use THAT tab (don't click Log in again).");
                        }
                    }
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    vh.login_btn.set_sensitive(true);
                    let who = val.get("account").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    vh.state_toast("Logged in", if who.is_empty() { &vh.store } else { &who });
                    vh.refresh_auth();
                    vh.refresh_library(true);
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    vh.login_btn.set_sensitive(true);
                    vh.auth_hint.set_visible(true);
                    vh.auth_hint.set_text(&format!("{} (codes expire fast — open ONE fresh tab and retry)", message));
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
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<StoreGame>, String>>();
        let store = self.store.clone();
        std::thread::spawn(move || {
            let _ = tx.send(StoreManager::library(&store, force).map(|d| {
                d.get("games").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                    .into_iter().map(|g| StoreGame {
                        app_id: g.get("app_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        title: g.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        version: g.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        installed: g.get("installed").and_then(|x| x.as_bool()).unwrap_or(false),
                        cover: g.get("cover").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        description: g.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    }).collect::<Vec<_>>()
            }).map_err(|e| e.to_string()));
        });
        let vh = self.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok(games)) => {
                vh.render_games(&games);
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
            if g.installed {
                let ib = gtk::Label::new(Some("installed"));
                ib.set_opacity(0.6);
                ib.add_css_class("time-label");
                brow.append(&ib);
            }
            inner.append(&brow);
            let name = gtk::Label::new(Some(&g.title));
            name.set_halign(gtk::Align::Center);
            name.set_wrap(true);
            name.set_max_width_chars(18);
            name.add_css_class("details-title");
            inner.append(&name);
            let btnrow = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            btnrow.set_halign(gtk::Align::Center);
            btnrow.set_homogeneous(true);
            let btn = gtk::Button::with_label(if g.installed { "Import" } else { "Install" });
            btn.add_css_class("add-btn");
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
        body.append(&note("Loading info…"));
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
        let (store, app) = (self.store.clone(), game.app_id.clone());
        let (ftitle, fcover, fdesc, fver) = (game.title.clone(), game.cover.clone(), game.description.clone(), game.version.clone());
        std::thread::spawn(move || {
            let _ = tx.send(StoreManager::game_info(&store, &app).map_err(|e| e.to_string()));
        });
        let vh = self.clone();
        let game_c = game.clone();
        let dd0 = dlg.clone();
        crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
            Ok(Ok(doc)) => {
                while let Some(c) = body.first_child() {
                    body.remove(&c);
                }
                let info = doc.get("info").cloned().unwrap_or_default();
                let get = |k: &str, fb: &str| info.get(k).and_then(|x| x.as_str()).unwrap_or(fb).to_string();
                let title = get("title", &ftitle);
                let cover = get("cover", &fcover);
                let mut desc = get("description", &fdesc);
                if desc.trim() == title.trim() {
                    desc.clear();
                }
                let ver = get("version", &fver);
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
                tcol.set_valign(gtk::Align::Start);
                let tt = gtk::Label::new(Some(&title));
                tt.set_halign(gtk::Align::Start);
                tt.set_wrap(true);
                tt.add_css_class("details-title");
                tcol.append(&tt);
                if !ver.is_empty() {
                    let vs = gtk::Label::new(Some(&ver));
                    vs.set_halign(gtk::Align::Start);
                    vs.set_opacity(0.6);
                    vs.add_css_class("time-label");
                    tcol.append(&vs);
                }
                if !desc.is_empty() {
                    let dl = gtk::Label::new(Some(&desc));
                    dl.set_halign(gtk::Align::Start);
                    dl.set_wrap(true);
                    dl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    dl.set_lines(6);
                    dl.set_opacity(0.85);
                    tcol.append(&dl);
                }
                top.append(&tcol);
                body.append(&top);
                if desc.lines().count() > 6 {
                    let exp = gtk::Expander::new(Some("Read more"));
                    let full = gtk::Label::new(Some(&desc));
                    full.set_halign(gtk::Align::Start);
                    full.set_wrap(true);
                    let scr = gtk::ScrolledWindow::new();
                    scr.set_min_content_height(120);
                    scr.set_vexpand(true);
                    scr.set_child(Some(&full));
                    exp.set_child(Some(&scr));
                    body.append(&exp);
                }
                let brow = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                brow.set_homogeneous(true);
                if let Some(url) = info.get("store_url").and_then(|x| x.as_str()) {
                    if !url.is_empty() {
                        let open = gtk::Button::with_label("Open store page");
                        open.add_css_class("settings-btn");
                        let st = vh.state.clone();
                        let u = url.to_string();
                        open.connect_clicked(move |_| { st.integration.open_url(&u); });
                        brow.append(&open);
                    }
                }
                let ib = gtk::Button::with_label(if game_c.installed { "Import to library" } else { "Install" });
                ib.add_css_class("add-btn");
                let vv = vh.clone();
                let gc = game_c.clone();
                let dd = dd0.clone();
                ib.connect_clicked(move |_| {
                    dd.close();
                    vv.install_or_import(&gc);
                });
                brow.append(&ib);
                body.append(&brow);
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                while let Some(c) = body.first_child() {
                    body.remove(&c);
                }
                body.append(&note(&format!("Failed: {}", e)));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
        dlg.present(Some(&self.parent));
    }

    fn default_games_dir() -> String {
        format!("{}/Games/Heroic", std::env::var("HOME").unwrap_or_default())
    }

    fn install_or_import(&self, game: &StoreGame) {
        if game.installed || self.state.game_model.get_game(&game.title).is_some() {
            self.import_to_library(&game.title, &self.store, &game.app_id, "", "", false, false);
            return;
        }
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
                    vh.refresh_library(false);
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

    fn import_to_library(&self, title: &str, store: &str, app_id: &str, install_path: &str, exe: &str, eac: bool, battleye: bool) {
        if !install_path.is_empty() && !std::path::Path::new(install_path).exists() {
            self.state_toast("Not found on disk", &format!("{} is gone — reinstall it first.", install_path));
            return;
        }
        let name = if self.state.game_model.get_game(title).is_some() {
            format!("{} ({})", title, if store == "epic" { "Epic" } else { "GOG" })
        } else {
            title.to_string()
        };
        if self.state.game_model.get_game(&name).is_some() {
            self.state_toast("Already in library", &name);
            return;
        }
        let main_path = if install_path.is_empty() {
            format!("{}/{}", Self::default_games_dir(), app_id)
        } else {
            install_path.to_string()
        };
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
        self.state.recent_model.refresh(30);
        if let Some(ref sb) = *self.sidebar.borrow() {
            sb.apply_current_filter();
        }
        if let Some(ref c) = *self.center.borrow() {
            c.rebuild(&self.state, &self.details);
        }
        self.state_toast("Added to library", &name);
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
