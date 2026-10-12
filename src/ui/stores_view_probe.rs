// Layout probe for library cards (GOG/Epic tile size + cover fill).
// #[ignore]: needs a live display; never runs in CI or default suites.
// Run: HOME=/tmp/px/fake XDG_CONFIG_HOME=/tmp/px/fake/.config
//   XDG_DATA_HOME=/tmp/px/fake/.local/share GSK_RENDERER=cairo
//   cargo test --bin corkytux temp_card_measure -- --ignored --test-threads=1 --nocapture
// Prepare Launcher.ini (Theme/Accent) and a cached landscape cover named
// cover-store-gog-<appid>-icon.png under $HOME/.cache/CorkyTux/modicons
// before running; without it the pixel-fill check is skipped.
use super::*;
use gtk::prelude::*;

fn mk_game(app_id: &str, cover: &str) -> StoreGame {
    StoreGame {
        app_id: app_id.to_string(),
        title: format!("Game {}", app_id),
        category: "RPG".to_string(),
        systems: vec!["Windows".to_string()],
        version: String::new(),
        installed: false,
        stale_registry: false,
        install_path: String::new(),
        executable: String::new(),
        cover: cover.to_string(),
        description: String::new(),
    }
}

fn pump(ms: u64) {
    let ctx = glib::MainContext::default();
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn alloc(w: &gtk::Widget) -> (i32, i32) {
    let a = w.allocation();
    (a.width(), a.height())
}

// Snapshot a mapped widget to pixels via the window renderer.
// Test cover is solid red (220,30,30); box background is well #181818.
fn pixel_fill(
    renderer: &gtk::gsk::Renderer,
    pic: &gtk::Widget,
    zw: i32,
    zh: i32,
) -> (usize, usize, usize, String) {
    let paintable = gtk::WidgetPaintable::new(Some(pic));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, zw as f64, zh as f64);
    let node = match snapshot.to_node() {
        Some(n) => n,
        None => return (0, 0, 0, "no-node".to_string()),
    };
    let tex = renderer.render_texture(
        &node,
        Some(&gtk::graphene::Rect::new(0.0, 0.0, zw as f32, zh as f32)),
    );
    let (tw, th) = (tex.width(), tex.height());
    if tw <= 0 || th <= 0 {
        return (0, 0, 0, format!("zero-tex {}x{}", tw, th));
    }
    let mut data = vec![0u8; (tw * th * 4) as usize];
    tex.download(&mut data, (tw * 4) as usize);
    let (mut red, mut bg, mut other) = (0, 0, 0);
    for px in data.chunks_exact(4) {
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        if a < 16 {
            bg += 1;
            continue;
        }
        if r > 150 && g < 100 && b < 100 {
            red += 1;
        } else if (r as i32 - 24).abs() < 30
            && (g as i32 - 24).abs() < 30
            && (b as i32 - 24).abs() < 30
        {
            bg += 1;
        } else {
            other += 1;
        }
    }
    (red, bg, other, format!("tex-{}x{}", tw, th))
}

fn boxed_child(box_w: &gtk::Box, idx: usize) -> Option<gtk::Widget> {
    let mut c = box_w.first_child();
    let mut n = 0;
    while let Some(w) = c {
        if n == idx {
            return Some(w.upcast::<gtk::Widget>());
        }
        n += 1;
        c = w.next_sibling();
    }
    None
}

#[test]
#[ignore]
fn temp_measure_gog_cards() {
    let _env = crate::test_env::lock();
    gtk::init().unwrap();
    let state = AppState::new();
    crate::ui::helpers::apply_theme_css(&state.theme);
    let app = adw::Application::builder()
        .application_id("com.corkytux.probe")
        .build();
    app.register(gtk::gio::Cancellable::NONE).unwrap();
    let parent = adw::ApplicationWindow::new(&app);
    parent.set_opacity(0.0);
    parent.set_focusable(false);
    parent.set_focus_on_click(false);
    parent.set_default_size(1400, 900);
    let sb: Rc<RefCell<Option<crate::ui::sidebar::Sidebar>>> =
        Rc::new(RefCell::new(None));
    let ch: Rc<RefCell<Option<crate::ui::center::CenterHandle>>> =
        Rc::new(RefCell::new(None));
    let det: Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>> =
        Rc::new(RefCell::new(None));
    let deals: Rc<RefCell<std::collections::HashMap<String, DealsState>>> =
        Rc::new(RefCell::new(std::collections::HashMap::new()));
    let (page, handle) =
        StoresView::store_page(&state, &parent, &sb, &ch, &det, "gog", &deals);
    parent.set_content(Some(&page));
    parent.present();
    for _ in 0..400 {
        if parent.is_mapped() {
            break;
        }
        glib::MainContext::default().iteration(false);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(parent.is_mapped(), "probe window never mapped");
    let surface = parent.surface().expect("no surface after map");
    let renderer =
        gtk::gsk::Renderer::for_surface(&surface).expect("no renderer");
    let (tile_w, _img_px, cover_h) = handle.card_sizes();
    eprintln!(
        "TEMP-PROBE card_sizes: tile_w={} cover_h={}",
        tile_w, cover_h
    );
    for n in [2usize, 8, 30] {
        let games: Vec<StoreGame> = (0..n)
            .map(|i| mk_game(&format!("{}", 1001 + i), "https://example.invalid/x.jpg"))
            .collect();
        handle.render_gog_cards(&games, true);
        pump(400);
        let flow_alloc = alloc(&handle.flow.clone().upcast::<gtk::Widget>());
        eprintln!("TEMP-PROBE n={} flow={}x{}", n, flow_alloc.0, flow_alloc.1);
        let mut i = 0;
        let mut child = handle.flow.first_child();
        while let Some(tile) = child {
            let tile_alloc = alloc(&tile);
            let inner = tile
                .first_child()
                .expect("tile without inner box")
                .downcast::<gtk::Box>()
                .expect("inner not a box");
            let inner_alloc = alloc(&inner.clone().upcast::<gtk::Widget>());
            // coverbox is the first row when a cover URL exists.
            let coverbox = boxed_child(&inner, 0).expect("no coverbox");
            let cover_alloc =
                alloc(&coverbox);
            // Picture may sit under wrapper widgets (isolation
            // experiments); descend to it.
            fn find_picture(w: &gtk::Widget) -> Option<gtk::Widget> {
                if w.downcast_ref::<gtk::Picture>().is_some() {
                    return Some(w.clone());
                }
                let mut c = w.first_child();
                while let Some(k) = c {
                    if let Some(p) = find_picture(&k) {
                        return Some(p);
                    }
                    c = k.next_sibling();
                }
                None
            }
            let pic = find_picture(&coverbox).expect("no picture under coverbox");
            let pic_alloc = alloc(&pic);
            eprintln!(
                "TEMP-PROBE n={} card#{} tile={}x{} inner={}x{} coverbox={}x{} pic={}x{} pic-is-picture={}",
                n,
                i,
                tile_alloc.0,
                tile_alloc.1,
                inner_alloc.0,
                inner_alloc.1,
                cover_alloc.0,
                cover_alloc.1,
                pic_alloc.0,
                pic_alloc.1,
                pic.downcast_ref::<gtk::Picture>().is_some(),
            );
            if i == 0 {
                if let Some(p) = pic.downcast_ref::<gtk::Picture>() {
                    // Wait for the cached cover to land on the paintable.
                    for _ in 0..200 {
                        if p.paintable().is_some() {
                            break;
                        }
                        pump(50);
                    }
                    let has_paint = p.paintable().is_some();
                    let (pw, ph) = p
                        .paintable()
                        .map(|t| (t.intrinsic_width(), t.intrinsic_height()))
                        .unwrap_or((0, 0));
                    eprintln!(
                        "TEMP-PROBE n={} pic-mapped={} paintable={}x{}",
                        n,
                        p.is_mapped(),
                        pw,
                        ph
                    );
                    // Geometry + texture check: Cover always fills its
                    // allocation, so equal pic/coverbox allocation plus
                    // a solid texture proves full coverage.
                    let (tw2, th2) = p
                        .paintable()
                        .map(|t| (t.intrinsic_width(), t.intrinsic_height()))
                        .unwrap_or((0, 0));
                    let snap2 = gtk::Snapshot::new();
                    if let Some(t) = p.paintable() {
                        use gdk::prelude::*;
                        t.snapshot(&snap2, tw2 as f64, th2 as f64);
                    }
                    let (tred, tbg, tother, twhy) =
                        match snap2.to_node() {
                            Some(node) => {
                                let tex = renderer.render_texture(
                                    &node,
                                    Some(&gtk::graphene::Rect::new(
                                        0.0, 0.0, tw2 as f32, th2 as f32,
                                    )),
                                );
                                let (tw3, th3) = (tex.width(), tex.height());
                                let mut data =
                                    vec![0u8; (tw3 * th3 * 4).max(4) as usize];
                                if tw3 > 0 && th3 > 0 {
                                    tex.download(&mut data, (tw3 * 4) as usize);
                                }
                                let (mut a, mut b, mut c) = (0, 0, 0);
                                for px in data.chunks_exact(4) {
                                    let (bb, gg, rr, aa) =
                                        (px[0], px[1], px[2], px[3]);
                                    if aa < 16 {
                                        b += 1;
                                    } else if rr > 150 && gg < 100 && bb < 100 {
                                        a += 1;
                                    } else {
                                        c += 1;
                                    }
                                }
                                (a, b, c, format!("tex-{}x{}", tw3, th3))
                            }
                            None => (0, 0, 0, "tex-node-none".to_string()),
                        };
                    eprintln!(
                        "TEMP-PROBE n={} tex-paintable={}x{} red-px={} via={}",
                        n, tw2, th2, tred, twhy
                    );
                    let (red, bg, other, how) = (tred, tbg, tother, twhy);
                    let total = (red + bg + other).max(1) as f64;
                    eprintln!(
                        "TEMP-PROBE n={} cover-paintable={} via={} fill-red={:.1}% bg={:.1}% other={:.1}%",
                        n,
                        has_paint,
                        how,
                        red as f64 * 100.0 / total,
                        bg as f64 * 100.0 / total,
                        other as f64 * 100.0 / total,
                    );
                }
            }
            i += 1;
            if i >= 3 {
                // Print first 3, then count the rest.
                let mut rest = 0;
                let mut c = tile.next_sibling();
                while c.is_some() {
                    rest += 1;
                    c = c.unwrap().next_sibling();
                }
                eprintln!("TEMP-PROBE n={} total-cards={}", n, i + rest);
                break;
            }
            child = tile.next_sibling();
        }
    }
    // Wheel-target check: what answers pick() at a cover center?
    // Scroll events go to the picked widget; a nested scroller would
    // eat them instead of the page scroller.
    if let Some(tile0) = handle.flow.first_child() {
        if let Some(inner0) = tile0.first_child() {
            if let Some(inner_box) = inner0.downcast_ref::<gtk::Box>() {
                if let Some(coverbox) = boxed_child(inner_box, 0) {
                    let ca = coverbox.allocation();
                    if let Some((px, py)) = coverbox.translate_coordinates(
                        &page,
                        ca.width() as f64 / 2.0,
                        ca.height() as f64 / 2.0,
                    ) {
                        let hit = page.pick(px, py, gtk::PickFlags::DEFAULT);
                        let mut chain = Vec::new();
                        let mut cur = hit.clone();
                        while let Some(w) = cur {
                            chain.push(format!(
                                "{}[{}]",
                                w.css_name(),
                                w.css_classes()
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect::<Vec<_>>()
                                    .join(".")
                            ));
                            cur = w.parent();
                            if chain.len() > 6 {
                                break;
                            }
                        }
                        eprintln!("TEMP-PROBE pick-cover-center -> {}", chain.join(" > "));
                    } else {
                        eprintln!("TEMP-PROBE pick-cover-center -> no-coords");
                    }
                }
            }
        }
    }
    // Epic shares the builder: smoke-check 8 cards on its own page.
    let (epic_page, epic_handle) =
        StoresView::store_page(&state, &parent, &sb, &ch, &det, "epic", &deals);
    parent.set_content(Some(&epic_page));
    let epic_games: Vec<StoreGame> = (0..8)
        .map(|i| mk_game(&format!("{}", 2001 + i), "https://example.invalid/x.jpg"))
        .collect();
    epic_handle.render_games(&epic_games);
    pump(400);
    let (etile_w, _, ecover_h) = epic_handle.card_sizes();
    let mut ei = 0;
    let mut echild = epic_handle.flow.first_child();
    while let Some(tile) = echild {
        let tile_alloc = alloc(&tile);
        let einner = tile
            .first_child()
            .map(|w| alloc(&w))
            .unwrap_or((0, 0));
        eprintln!(
            "TEMP-PROBE epic card#{} tile={}x{} inner={}x{} expect={}x{}",
            ei,
            tile_alloc.0,
            tile_alloc.1,
            einner.0,
            einner.1,
            etile_w,
            ecover_h + 138,
        );
        ei += 1;
        echild = tile.next_sibling();
    }
    eprintln!("TEMP-PROBE epic total-cards={}", ei);
    renderer.unrealize();
    parent.hide();
    parent.destroy();
}
