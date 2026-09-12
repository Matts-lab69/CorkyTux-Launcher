use adw::prelude::*;
use gtk::prelude::*;
use gtk::{self, glib};

use crate::backend::theme::ThemeManager;
use crate::ui::helpers;

pub struct LogModal {
    dialog: adw::Dialog,
    log_buffer: gtk::TextBuffer,
    status_label: gtk::Label,
    game_name: String,
    search_mark: gtk::TextMark,
    parent: gtk::Widget,
}

impl LogModal {
    pub fn new(theme: &ThemeManager, game_name: &str, parent: &impl IsA<gtk::Widget>) -> Self {
        let dialog = adw::Dialog::new();
        dialog.set_title("Game Log");
        dialog.set_content_width(640);
        dialog.set_content_height(480);

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

        let (header_row, x_btn) = helpers::modal_header("Game Log");
        header_row.set_margin_top(4);
        {
            let dlg = dialog.clone();
            x_btn.connect_clicked(move |_| { dlg.close(); });
        }
        inner.append(&header_row);

        let status_label = gtk::Label::new(Some("Game stopped"));
        status_label.set_halign(gtk::Align::Start);
        status_label.add_css_class("details-title");
        inner.append(&status_label);

        let hint = gtk::Label::new(Some("If there are errors, you can figure it out yourself or send the log to the community."));
        hint.set_halign(gtk::Align::Start);
        hint.set_wrap(true);
        hint.set_opacity(0.6);
        hint.add_css_class("time-label");
        inner.append(&hint);

        // Lutris parity: error/warn levels get their own colors so real
        // failures pop out of Proton/umu noise.
        let log_view = gtk::TextView::new();
        log_view.add_css_class("log-view");
        log_view.set_editable(false);
        log_view.set_cursor_visible(false);
        log_view.set_wrap_mode(gtk::WrapMode::Word);
        log_view.set_monospace(true);
        log_view.set_top_margin(8);
        log_view.set_bottom_margin(8);
        log_view.set_left_margin(8);
        log_view.set_right_margin(8);
        let log_buffer = log_view.buffer();
        {
            let tags = log_buffer.tag_table();
            let err_tag = gtk::TextTag::new(Some("log-err"));
            err_tag.set_foreground(Some(if theme.is_dark() { "#FF7A7A" } else { "#C92A2A" }));
            err_tag.set_weight(700);
            tags.add(&err_tag);
            let warn_tag = gtk::TextTag::new(Some("log-warn"));
            warn_tag.set_foreground(Some(if theme.is_dark() { "#FFC94D" } else { "#8A5A00" }));
            tags.add(&warn_tag);
            let hit_tag = gtk::TextTag::new(Some("log-hit"));
            hit_tag.set_background(Some("#1E88E5"));
            hit_tag.set_foreground(Some("#FFFFFF"));
            tags.add(&hit_tag);
        }
        let search_mark = log_buffer.create_mark(None, &log_buffer.start_iter(), true);

        // Lutris parity: search row (find first/next/previous + highlight).
        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_hexpand(true);
        search_entry.set_placeholder_text(Some("Search log (e.g. err:)"));
        search_row.append(&search_entry);
        let prev_btn = gtk::Button::with_label("▲");
        prev_btn.set_tooltip_text(Some("Previous match"));
        let next_btn = gtk::Button::with_label("▼");
        next_btn.set_tooltip_text(Some("Next match"));
        search_row.append(&prev_btn);
        search_row.append(&next_btn);
        inner.append(&search_row);
        {
            let buf_c = log_buffer.clone();
            let view_c = log_view.clone();
            let mark_c = search_mark.clone();
            let entry_c = search_entry.clone();
            let find = std::rc::Rc::new(move |forward: bool, reset: bool| {
                let needle = entry_c.text().to_string();
                if needle.is_empty() {
                    return;
                }
                // Clear old hit highlight.
                let (mut s, mut e) = buf_c.bounds();
                buf_c.remove_tag_by_name("log-hit", &mut s, &mut e);
                let mut it = if reset {
                    buf_c.start_iter()
                } else {
                    buf_c.iter_at_mark(&mark_c)
                };
                if !forward && !reset {
                    it.backward_chars(needle.chars().count() as i32);
                }
                let found = if forward {
                    it.forward_search(&needle, gtk::TextSearchFlags::CASE_INSENSITIVE, None)
                } else {
                    it.backward_search(&needle, gtk::TextSearchFlags::CASE_INSENSITIVE, None)
                };
                let found = found.or_else(|| {
                    let edge = if forward { buf_c.start_iter() } else { buf_c.end_iter() };
                    if forward {
                        edge.forward_search(&needle, gtk::TextSearchFlags::CASE_INSENSITIVE, None)
                    } else {
                        edge.backward_search(&needle, gtk::TextSearchFlags::CASE_INSENSITIVE, None)
                    }
                });
                if let Some((ms, me)) = found {
                    buf_c.apply_tag_by_name("log-hit", &ms, &me);
                    buf_c.select_range(&ms, &me);
                    view_c.scroll_to_iter(&mut me.clone(), 0.0, true, 0.0, 0.5);
                    buf_c.move_mark(&mark_c, &me);
                }
            });
            let find_c = find.clone();
            search_entry.connect_search_changed(move |_| find_c(true, true));
            let find_c = find.clone();
            next_btn.connect_clicked(move |_| find_c(true, false));
            let find_c = find.clone();
            prev_btn.connect_clicked(move |_| find_c(false, false));
        }

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&log_view));
        scroll.set_vexpand(true);
        scroll.set_min_content_height(300);
        inner.append(&scroll);

        // Buttons
        let btn_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        btn_row.set_halign(gtk::Align::Fill);

        let github_btn = gtk::Button::with_label("GitHub");
        github_btn.add_css_class("settings-btn");
        github_btn.set_hexpand(true);
        github_btn.connect_clicked(|_| {
            let _ = std::process::Command::new("xdg-open")
                .arg("https://github.com/Matts-lab69/corkytux/issues")
                .spawn();
        });

        // Fixed neon colors (NOT theme affected): red close, green save
        let close_btn = gtk::Button::with_label("Close");
        close_btn.add_css_class("neon-red");
        close_btn.set_hexpand(true);
        let dlg = dialog.clone();
        close_btn.connect_clicked(move |_| { dlg.close(); });

        let save_btn = gtk::Button::with_label("Save");
        save_btn.add_css_class("neon-green");
        save_btn.set_hexpand(true);
        let buf = log_buffer.clone();
        let game_c = game_name.to_string();
        let parent_c = parent.clone();
        save_btn.connect_clicked(move |_| {
            let (start, end) = buf.bounds();
            let text = buf.text(&start, &end, false).to_string();
            if text.is_empty() {
                return;
            }
            // Save straight into the launcher's own logs dir
            // (~/.local/share/CorkyTux/logs), never Documents.
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let ts = std::process::Command::new("date")
                .arg("+%Y-%m-%d-%H-%M")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "log".into());
            let dir = format!("{}/.local/share/CorkyTux/logs", home);
            std::fs::create_dir_all(&dir).ok();
            let path = format!("{}/{} ({}).log", dir, game_c, ts);
            {
                match std::fs::write(&path, &text) {
                    Ok(()) => {
                        crate::ui::helpers::present_msg(&parent_c, "Log saved", &path);
                    }
                    Err(e) => {
                        crate::ui::helpers::present_msg(&parent_c, "Could not save log", &e.to_string());
                    }
                }
            }
        });

        btn_row.append(&github_btn);
        btn_row.append(&close_btn);
        btn_row.append(&save_btn);
        inner.append(&btn_row);

        dialog.set_child(Some(&content));

        Self { dialog, log_buffer, status_label, game_name: game_name.to_string(), search_mark, parent: parent.upcast_ref::<gtk::Widget>().clone() }
    }

    pub fn set_log(&self, text: &str) {
        self.log_buffer.set_text(text);
        Self::paint_levels(&self.log_buffer);
        self.search_mark_reset();
    }

    fn search_mark_reset(&self) {
        let mut start = self.log_buffer.start_iter();
        self.log_buffer.move_mark(&self.search_mark, &mut start);
    }

    /// Lutris-style levels: err* lines red/bold, *warn* amber.
    fn paint_levels(buf: &gtk::TextBuffer) {
        let mut start = buf.start_iter();
        while !start.is_end() {
            let mut end = start;
            end.forward_to_line_end();
            let line = buf.text(&start, &end, false).to_string().to_lowercase();
            let tag = if line.contains("err:")
                || line.contains("error")
                || line.contains("exception")
                || line.contains("traceback")
                || line.contains("critical")
                || line.contains("failed")
                || line.contains("cannot open")
            {
                Some("log-err")
            } else if line.contains("warn") {
                Some("log-warn")
            } else {
                None
            };
            if let Some(t) = tag {
                buf.apply_tag_by_name(t, &start, &end);
            }
            if end.is_end() {
                break;
            }
            end.forward_char();
            start = end;
        }
    }

    pub fn set_running(&self, running: bool) {
        if running {
            self.status_label.set_text("Running...");
            self.status_label.set_css_classes(&["details-title"]);
        } else {
            self.status_label.set_text("Game stopped");
            self.status_label.set_css_classes(&["details-title"]);
        }
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }

    /// Refresh the view from the game's log file every 500ms (C++ QTimer
    /// parity) until the game stops.
    pub fn tail_game_log(
        &self,
        game_name: &str,
        proton: &crate::backend::proton::ProtonManager,
        header: &str,
    ) {
        let buf = self.log_buffer.clone();
        let status = self.status_label.clone();
        let game = game_name.to_string();
        let p = proton.clone();
        let head = header.to_string();
        buf.set_text(&format!("{}\nStarting...\n", head));
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let text = crate::backend::proton::ProtonManager::read_log(&game);
            if !text.is_empty() {
                buf.set_text(&format!("{}\n{}", head, text));
                Self::paint_levels(&buf);
            }
            if p.is_game_running() {
                glib::ControlFlow::Continue
            } else {
                status.set_text("Game stopped");
                glib::ControlFlow::Break
            }
        });
    }
}
