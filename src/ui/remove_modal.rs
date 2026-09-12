use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::center::CenterHandle;
use crate::ui::details_panel::DetailsPanel;
use crate::ui::helpers;
use crate::ui::sidebar::Sidebar;
use crate::AppState;

pub fn show_remove_modal(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    sidebar: &Rc<RefCell<Option<Sidebar>>>,
    center: &Rc<RefCell<Option<CenterHandle>>>,
    details: &Rc<RefCell<Option<DetailsPanel>>>,
) {
    let selected = state.selected_game.borrow().clone();
    if selected.is_empty() { return; }

    let dialog = adw::Dialog::new();
    dialog.set_title("Remove Game");
    dialog.set_content_width(460);
    dialog.set_content_height(320);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modal-bg");
    content.set_hexpand(true);
    content.set_vexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
    inner.set_hexpand(true);
    inner.set_vexpand(true);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    content.append(&inner);

    // Header with title + X (launcher reference style)
    let (header_row, x_btn) = helpers::modal_header("Remove Game");
    header_row.set_margin_top(4);
    {
        let dlg = dialog.clone();
        x_btn.connect_clicked(move |_| { dlg.close(); });
    }
    inner.append(&header_row);

    let msg = gtk::Label::new(Some(&format!("Remove \"{}\" from launcher?", selected)));
    msg.set_wrap(true);
    msg.set_halign(gtk::Align::Start);
    inner.append(&msg);

    // Check if game is an emulator game (has executor, no proton)
    let is_emu = state.config.game_value(&selected, "Executor")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    // UseSharedPrefix: el prefix es de OTROS juegos también — nunca se borra
    // desde este diálogo.
    let use_shared_prefix = state.config.game_value(&selected, "UseSharedPrefix")
        .map(|v| v == "true")
        .unwrap_or(false);

    let chk_prefix = gtk::CheckButton::with_label("Remove game prefix (Wine/Proton data)");
    chk_prefix.set_visible(!is_emu && !use_shared_prefix);
    inner.append(&chk_prefix);

    let chk_files = gtk::CheckButton::with_label("Remove game files from disk");
    inner.append(&chk_files);

    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    inner.append(&spacer);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    btn_box.set_halign(gtk::Align::Fill);

    // Fixed neon colors (NOT theme affected)
    let no_btn = gtk::Button::with_label("Cancel");
    no_btn.add_css_class("neon-green");
    no_btn.set_hexpand(true);
    let yes_btn = gtk::Button::with_label("Remove");
    yes_btn.add_css_class("neon-red");
    yes_btn.set_hexpand(true);

    let state_clone = state.clone();
    let dialog_clone = dialog.clone();
    let sel = selected.clone();
    let sidebar_clone = sidebar.clone();
    let center_clone = center.clone();
    let details_clone = details.clone();
    yes_btn.connect_clicked(move |_| {
        let remove_prefix = chk_prefix.is_active();
        let remove_files = chk_files.is_active();

        if remove_prefix {
            let prefix = state_clone.proton.prefix_path(&sel);
            if prefix.exists() {
                let _ = state_clone.integration.remove_dir_recursive(&prefix.display().to_string());
            }
        }
        if remove_files {
            if let Some(main_path) = state_clone.game_model.get_game(&sel).map(|g| g.main_path) {
                if !main_path.is_empty() {
                    let _ = state_clone.integration.remove_dir_recursive(&main_path);
                }
            }
        }

        state_clone.game_model.remove_game(&sel);
        state_clone.recent_model.refresh(30);
        *state_clone.selected_game.borrow_mut() = String::new();

        if let Some(ref sb) = *sidebar_clone.borrow() {
            sb.apply_current_filter();
        }
        // The card must vanish from Recently Played too, and the details
        // panel must close (it would otherwise open for a deleted game).
        if let Some(ref c) = *center_clone.borrow() {
            c.rebuild(&state_clone, &details_clone);
        }
        if let Some(ref d) = *details_clone.borrow() {
            d.hide();
        }

        dialog_clone.close();
    });

    let dialog_clone2 = dialog.clone();
    no_btn.connect_clicked(move |_| { dialog_clone2.close(); });

    btn_box.append(&no_btn);
    btn_box.append(&yes_btn);
    inner.append(&btn_box);

    dialog.set_child(Some(&content));
    dialog.present(Some(parent));
}
