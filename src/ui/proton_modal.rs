use adw::prelude::*;
use gtk::prelude::*;

use crate::backend::proton::ProtonManager;

enum ModalMsg {
    Releases(Result<Vec<(String, String)>, String>),
    Progress(f64, f64),
    Downloaded(Result<String, String>),
}

pub struct ProtonModal {
    dialog: adw::Dialog,
}

impl ProtonModal {
    pub fn new(proton: &ProtonManager) -> Self {
        let dialog = adw::Dialog::new();
        dialog.set_title("Download Proton");
        dialog.set_content_width(480);
        dialog.set_content_height(560);

        // Full-bleed: outer carries .modal-bg to the dialog edges (no
        // gray Adwaita frame), inner holds the padded content.
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.set_hexpand(true);
        content.set_vexpand(true);
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
        inner.set_hexpand(true);
        inner.set_vexpand(true);
        inner.set_margin_top(12);
        inner.set_margin_bottom(12);
        inner.set_margin_start(16);
        inner.set_margin_end(16);
        content.append(&inner);

        let (header_row, x_btn) = crate::ui::helpers::modal_header("Download Proton");
        header_row.set_margin_top(4);
        {
            let dlg = dialog.clone();
            x_btn.connect_clicked(move |_| { dlg.close(); });
        }
        inner.append(&header_row);

        // Source selector
        let source_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let source_lbl = gtk::Label::new(Some("Source:"));
        source_lbl.set_halign(gtk::Align::Start);
        let source_dropdown = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
        let source_list = gtk::StringList::new(&["GE (GloriousEggroll)", "CachyOS"]);
        let source_model: gio::ListModel = source_list.clone().upcast();
        source_dropdown.set_model(Some(&source_model));
        source_dropdown.set_selected(0);
        source_dropdown.set_hexpand(true);
        source_box.append(&source_lbl);
        source_box.append(&source_dropdown);
        inner.append(&source_box);

        let status_label = gtk::Label::new(Some("Press Refresh to load releases"));
        status_label.set_halign(gtk::Align::Center);
        status_label.set_wrap(true);
        inner.append(&status_label);

        let progress_bar = gtk::ProgressBar::new();
        progress_bar.set_show_text(true);
        progress_bar.set_text(Some(""));
        inner.append(&progress_bar);

        // Releases list
        let releases_scroll = gtk::ScrolledWindow::new();
        releases_scroll.set_vexpand(true);
        releases_scroll.set_min_content_height(220);
        let releases_list = gtk::ListBox::new();
        releases_list.set_selection_mode(gtk::SelectionMode::None);
        releases_scroll.set_child(Some(&releases_list));
        inner.append(&releases_scroll);

        // Target dir row
        let target_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let target_lbl = gtk::Label::new(Some("Install to:"));
        target_lbl.set_halign(gtk::Align::Start);
        let target_entry = gtk::Entry::new();
        target_entry.set_text(&proton.default_download_dir().display().to_string());
        target_entry.set_hexpand(true);
        target_entry.set_editable(false);
        target_box.append(&target_lbl);
        target_box.append(&target_entry);
        inner.append(&target_box);

        // Buttons
        let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        btn_row.set_halign(gtk::Align::Fill);
        let refresh_btn = gtk::Button::with_label("Refresh");
        refresh_btn.add_css_class("suggested-action");
        refresh_btn.set_hexpand(true);
        let close_btn = gtk::Button::with_label("Close");
        close_btn.add_css_class("settings-btn");
        close_btn.set_hexpand(true);
        let dlg = dialog.clone();
        close_btn.connect_clicked(move |_| { dlg.close(); });
        btn_row.append(&refresh_btn);
        btn_row.append(&close_btn);
        inner.append(&btn_row);

        dialog.set_child(Some(&content));

        // Channel back to the UI thread, drained by a 100ms poller
        // (stops when the dialog closes — no zombie wakeups).
        let dlg_alive: std::rc::Rc<std::cell::Cell<bool>> = std::rc::Rc::new(std::cell::Cell::new(true));
        {
            let alive = dlg_alive.clone();
            let dlg_c = dialog.clone();
            dlg_c.connect_closed(move |_| { alive.set(false); });
        }
        let (tx, rx) = std::sync::mpsc::channel::<ModalMsg>();
        let tx_rows = tx.clone();
        let rx = std::rc::Rc::new(std::cell::RefCell::new(rx));
        {
            let status = status_label.clone();
            let bar = progress_bar.clone();
            let list = releases_list.clone();
            let proton_c = proton.clone();
            let target_e = target_entry.clone();
            let rx_poll = rx.clone();
            let alive_poll = dlg_alive.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                if !alive_poll.get() {
                    return glib::ControlFlow::Break;
                }
                let messages: Vec<ModalMsg> = {
                    let r = rx_poll.borrow();
                    let mut v = Vec::new();
                    while let Ok(msg) = r.try_recv() {
                        v.push(msg);
                    }
                    v
                };
                for msg in messages {
                    match msg {
                        ModalMsg::Releases(Ok(releases)) => {
                            while let Some(child) = list.first_child() {
                                list.remove(&child);
                            }
                            status.set_text(&format!("{} releases found", releases.len()));
                            for (tag, url) in releases {
                                let row = gtk::ListBoxRow::new();
                                let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                hbox.set_margin_top(4);
                                hbox.set_margin_bottom(4);
                                hbox.set_margin_start(8);
                                hbox.set_margin_end(8);
                                let lbl = gtk::Label::new(Some(&tag));
                                lbl.set_halign(gtk::Align::Start);
                                lbl.set_hexpand(true);
                                lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                                let dl_btn = gtk::Button::with_label("Download");
                                dl_btn.add_css_class("add-btn");
                                let tx2 = tx_rows.clone();
                                let proton_c2 = proton_c.clone();
                                let status2 = status.clone();
                                let bar2 = bar.clone();
                                let target2 = target_e.text().to_string();
                                let tag2 = tag.clone();
                                let url2 = url.clone();
                                dl_btn.connect_clicked(move |_| {
                                    status2.set_text(&format!("Downloading {}...", tag2));
                                    bar2.set_fraction(0.0);
                                    let tx3 = tx2.clone();
                                    let target3 = std::path::PathBuf::from(&target2);
                                    let tag3 = tag2.clone();
                                    let url3 = url2.clone();
                                    std::thread::spawn(move || {
                                        let res = ProtonManager::download_proton(
                                            &tag3,
                                            &url3,
                                            &target3,
                                            |f, bps| {
                                                let _ = tx3.send(ModalMsg::Progress(f, bps));
                                            },
                                        );
                                        let _ = tx3.send(ModalMsg::Downloaded(
                                            res.map(|p| {
                                                format!("Installed {} to {}", tag3, p.display())
                                            })
                                            .map_err(|e| e),
                                        ));
                                    });
                                });
                                hbox.append(&lbl);
                                hbox.append(&dl_btn);
                                row.set_child(Some(&hbox));
                                list.append(&row);
                            }
                        }
                        ModalMsg::Releases(Err(e)) => {
                            status.set_text(&format!("Failed to load releases: {}", e));
                        }
                        ModalMsg::Progress(f, bps) => {
                            bar.set_fraction(f.clamp(0.0, 1.0));
                            bar.set_text(Some(&format!(
                                "{:.0}% · {}",
                                f * 100.0,
                                ProtonManager::format_speed(bps)
                            )));
                        }
                        ModalMsg::Downloaded(Ok(msg)) => {
                            proton_c.refresh_installed();
                            status.set_text(&msg);
                            bar.set_fraction(0.0);
                            bar.set_text(Some(""));
                        }
                        ModalMsg::Downloaded(Err(e)) => {
                            status.set_text(&format!("Download failed: {}", e));
                        }
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        // Refresh button: fetch in background thread
        {
            let status = status_label.clone();
            let src = source_dropdown.clone();
            refresh_btn.connect_clicked(move |_| {
                status.set_text("Loading releases...");
                while let Some(child) = releases_list.first_child() {
                    releases_list.remove(&child);
                }
                let tx2 = tx.clone();
                let source = if src.selected() == 1 { "cachyos" } else { "ge" }.to_string();
                std::thread::spawn(move || {
                    let res = ProtonManager::fetch_releases(&source);
                    let _ = tx2.send(ModalMsg::Releases(res));
                });
            });
        }

        Self { dialog }
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }
}
