// Layout probe for the library sidebar: window, sidebar, list frame,
// scrollbar slider, separator, center column and Recently Played
// allocations at fixed window widths.
// #[ignore]: needs a live display; never runs in CI or default suites.
// Run: HOME=<fake> XDG_CONFIG_HOME=<fake>/.config cargo test --bin corkytux
//   temp_measure_sidebar_layout -- --ignored --test-threads=1 --nocapture
use crate::ui::center::build_center;
use crate::ui::sidebar::Sidebar;
use crate::AppState;
use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

fn pump(ms: u64) {
    let ctx = glib::MainContext::default();
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn find_by_class(w: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    if w.css_classes().iter().any(|c| c == class) {
        return Some(w.clone());
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Some(found) = find_by_class(&c, class) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

fn find_by_name(w: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if w.css_name() == name {
        return Some(w.clone());
    }
    let mut child = w.first_child();
    while let Some(c) = child {
        if let Some(found) = find_by_name(&c, name) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

fn size(w: &gtk::Widget) -> (i32, i32) {
    let a = w.allocation();
    (a.width(), a.height())
}

#[test]
#[ignore]
fn temp_measure_sidebar_layout() {
    let _env = crate::test_env::lock();
    gtk::init().unwrap();
    let state = AppState::new();
    crate::ui::helpers::apply_theme_css(&state.theme);
    let app = adw::Application::builder()
        .application_id("com.corkytux.layout.probe")
        .build();
    app.register(gtk::gio::Cancellable::NONE).unwrap();

    for width in [640, 800, 1000, 1250, 1600] {
        let window = adw::ApplicationWindow::new(&app);
        window.set_opacity(0.0);
        window.set_focusable(false);
        window.set_focus_on_click(false);
        window.set_size_request(640, 400);
        window.set_default_size(width, 700);

        let game_cb: crate::ui::sidebar::GameCallback = Rc::new(RefCell::new(None));
        let filter_cb: crate::ui::sidebar::FilterCallback = Rc::new(RefCell::new(None));
        let page_cb: crate::ui::sidebar::PageCallback = Rc::new(RefCell::new(None));
        let sidebar = Sidebar::new_with_callbacks(
            &state.config,
            &state.theme,
            &game_cb,
            &filter_cb,
            &page_cb,
        );
        let names: Vec<String> = (0..20).map(|i| format!("Probe Game {:02}", i)).collect();
        sidebar.refresh_list(&names);
        // Same layout role as in main: fixed at its natural width.
        sidebar.widget.set_hexpand(false);
        sidebar.widget.set_halign(gtk::Align::Start);

        let details_ref = Rc::new(RefCell::new(None));
        let center = build_center(&state, &details_ref);

        let separator = gtk::Separator::new(gtk::Orientation::Vertical);
        separator.set_margin_start(12);
        separator.set_margin_end(12);

        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        body.set_hexpand(true);
        body.append(&sidebar.widget);
        body.append(&separator);
        body.append(&center.widget);

        {
            let sep_c = separator.clone();
            let center_c = center.clone();
            sidebar.widget.connect_visible_notify(move |sb| {
                let visible = sb.is_visible();
                sep_c.set_visible(visible);
                center_c.set_sidebar_visible(visible);
            });
        }

        if let Ok(cond) = adw::BreakpointCondition::parse("max-width: 700px") {
            let bp = adw::Breakpoint::new(cond);
            bp.add_setter(
                &sidebar.widget,
                "visible",
                Some(&gtk::glib::Value::from(false)),
            );
            window.add_breakpoint(bp);
        }

        window.set_content(Some(&body));
        window.present();
        for _ in 0..400 {
            if window.is_mapped() {
                break;
            }
            glib::MainContext::default().iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        pump(250);
        assert!(window.is_mapped(), "probe window never mapped");

        let win = size(&window.clone().upcast::<gtk::Widget>());
        let sb = size(&sidebar.widget.clone().upcast::<gtk::Widget>());
        let frame = find_by_class(
            &sidebar.widget.clone().upcast::<gtk::Widget>(),
            "sidebar-frame",
        )
        .map(|f| size(&f))
        .unwrap_or((0, 0));
        let (bar, slider) = match find_by_class(
            &sidebar.widget.clone().upcast::<gtk::Widget>(),
            "sidebar-scroll",
        ) {
            Some(w) => match w.downcast::<gtk::ScrolledWindow>() {
                Ok(scrolled) => {
                    let bar = size(&scrolled.vscrollbar().clone().upcast::<gtk::Widget>());
                    let slider = find_by_name(
                        &scrolled.vscrollbar().clone().upcast::<gtk::Widget>(),
                        "slider",
                    )
                    .map(|s| size(&s))
                    .unwrap_or((0, 0));
                    (bar, slider)
                }
                Err(_) => ((0, 0), (0, 0)),
            },
            None => ((0, 0), (0, 0)),
        };
        let sep = size(&separator.clone().upcast::<gtk::Widget>());
        let center_col = size(&center.widget.clone().upcast::<gtk::Widget>());
        let recent = find_by_class(
            &center.widget.clone().upcast::<gtk::Widget>(),
            "recent-frame",
        )
        .map(|f| size(&f))
        .unwrap_or((0, 0));

        eprintln!(
            "LAYOUT width={} window={}x{} sidebar={}x{} list-frame={}x{} scrollbar={}x{} slider={}x{} separator={}x{} center={}x{} recent={}x{}",
            width,
            win.0,
            win.1,
            sb.0,
            sb.1,
            frame.0,
            frame.1,
            bar.0,
            bar.1,
            slider.0,
            slider.1,
            sep.0,
            sep.1,
            center_col.0,
            center_col.1,
            recent.0,
            recent.1,
        );

        window.destroy();
        pump(100);
    }
}
