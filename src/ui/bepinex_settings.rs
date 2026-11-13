use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;

use crate::backend::bepinex;
use crate::AppState;

#[derive(Clone)]
struct Binding {
    state: AppState,
    game: String,
    path: PathBuf,
    raw_directory: String,
    executable: PathBuf,
    steam_managed: bool,
    switch: gtk::Switch,
    status: gtk::Label,
    busy: Rc<Cell<bool>>,
    enabled: Rc<Cell<bool>>,
    installation_available: Rc<Cell<bool>>,
    entry: Rc<RefCell<Option<gtk::Entry>>>,
}

pub(super) fn key(state: &AppState, game: &str) -> &'static str {
    // Prefer the canonical setting when present; retain legacy compatibility.
    bepinex::override_key(&state.config.game_section(game))
}

impl Binding {
    fn restore_controls(&self, active: bool) {
        self.switch.set_active(active);
        self.enabled.set(active);
        self.switch
            .set_sensitive(active || self.installation_available.get());
        if let Some(entry) = self.entry.borrow().as_ref() {
            entry.set_sensitive(true);
        }
        self.busy.set(false);
    }

    fn start(&self, enable: bool, authorization: Option<bepinex::Authorization>) {
        self.busy.set(true);
        self.switch.set_sensitive(false);
        if let Some(entry) = self.entry.borrow().as_ref() {
            entry.set_sensitive(false);
        }
        self.status.set_text(if enable {
            "Preparing BepInEx… No downloaded files will be executed."
        } else {
            "Removing unchanged, managed BepInEx files…"
        });
        let current = self
            .state
            .config
            .game_value(&self.game, key(&self.state, &self.game))
            .unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel();
        let path = self.path.clone();
        let executable = self.executable.clone();
        let initial = current.clone();
        std::thread::spawn(move || {
            let result = if enable {
                authorization
                    .ok_or_else(|| {
                        "BepInEx package detection is missing; reopen settings".to_string()
                    })
                    .and_then(|authorization| {
                        bepinex::enable(&path, &executable, &initial, authorization)
                    })
            } else {
                bepinex::disable(&path, &initial)
            };
            let _ = tx.send(result);
        });
        let binding = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(result) => {
                let result = result.and_then(|mut change| {
                    if !binding.state.config.game_names().contains(&binding.game) {
                        return Err("The game was removed; its settings were not recreated.".into());
                    }
                    if binding.state.config.game_value(&binding.game, "MainPath").unwrap_or_default() != binding.raw_directory {
                        return Err("The game folder changed during the operation; activation settings were not overwritten.".into());
                    }
                    let latest = binding
                        .state
                        .config
                        .game_value(&binding.game, key(&binding.state, &binding.game))
                        .unwrap_or_default();
                    if enable {
                        change.overrides = bepinex::rebase_overrides(&binding.path, &latest)?;
                    } else if latest != current {
                        change.overrides = latest;
                        change.warning =
                            Some("DLL overrides changed during removal and were preserved.".into());
                    }
                    Ok(change)
                });
                match result {
                    Ok(change) => {
                        binding.state.config.set_game_value(
                            &binding.game,
                            key(&binding.state, &binding.game),
                            &change.overrides,
                        );
                        binding.state.config.set_game_value(
                            &binding.game,
                            "BepInExEnabled",
                            if enable { "true" } else { "false" },
                        );
                        if let Some(entry) = binding.entry.borrow().as_ref() {
                            if entry.widget_name().as_str() == key(&binding.state, &binding.game) {
                                entry.set_text(&change.overrides);
                            }
                        }
                        let message = change.warning.as_deref().unwrap_or(if enable {
                            "BepInEx enabled. User mods are managed separately."
                        } else {
                            "BepInEx disabled. User mods, plugins and configuration were kept."
                        });
                        if enable && binding.steam_managed {
                            binding.status.set_text(&format!("{message} Steam requires WINEDLLOVERRIDES=\"winhttp=n,b\" %command% in its own launch options; Steam configuration was not changed."));
                        } else {
                            binding.status.set_text(message);
                        }
                        binding.restore_controls(enable);
                    }
                    Err(error) => {
                        binding.status.set_text(&format!("BepInEx: {error}"));
                        binding.restore_controls(binding.enabled.get());
                    }
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                binding
                    .status
                    .set_text("BepInEx operation stopped; inspect the game before retrying.");
                binding.restore_controls(binding.enabled.get());
                glib::ControlFlow::Break
            }
        });
    }
}

pub fn append(
    container: &gtk::Box,
    state: &AppState,
    game: &str,
    _parent: &adw::ApplicationWindow,
    entry: Rc<RefCell<Option<gtk::Entry>>>,
) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::new(Some("Mods (BepInEx)"));
    title.set_halign(gtk::Align::Start);
    title.set_hexpand(true);
    title.add_css_class("info-value");
    let switch = gtk::Switch::new();
    switch.set_valign(gtk::Align::Center);
    switch.set_sensitive(false);
    row.append(&title);
    row.append(&switch);
    container.append(&row);
    let status = gtk::Label::new(Some(
        "Checking Windows Unity runtime and executable architecture…",
    ));
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("time-label");
    container.append(&status);
    let raw = state
        .config
        .game_value(game, "MainPath")
        .unwrap_or_default();
    let path = if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(rest)
    } else {
        PathBuf::from(&raw)
    };
    let executable = PathBuf::from(
        state
            .config
            .game_value(game, "Executable")
            .unwrap_or_default(),
    );
    let binding = Binding {
        state: state.clone(),
        game: game.into(),
        path,
        raw_directory: raw,
        executable,
        steam_managed: crate::backend::game_model::is_steam_game(
            &state.config.game_value(game, "Source").unwrap_or_default(),
            &state.config.game_value(game, "SteamID").unwrap_or_default(),
        ),
        switch: switch.clone(),
        status,
        busy: Rc::new(Cell::new(true)),
        enabled: Rc::new(Cell::new(false)),
        installation_available: Rc::new(Cell::new(false)),
        entry,
    };
    let target = Rc::new(Cell::new(None::<bepinex::Target>));
    let b = binding.clone();
    let t = target.clone();
    switch.connect_active_notify(move |switch| {
        if b.busy.get() {
            return;
        }
        let enable = switch.is_active();
        b.start(
            enable,
            t.get().map(|target| bepinex::Authorization {
                target,
                allow_unverified: false,
            }),
        );
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let path = binding.path.clone();
    let executable = binding.executable.clone();
    std::thread::spawn(move || {
        let _ = tx.send(bepinex::inspect(&path, &executable));
    });
    glib::idle_add_local(move || match rx.try_recv() {
        Ok(Ok(inspection)) => {
            target.set(Some(inspection.target));
            binding
                .installation_available
                .set(inspection.target.installation_available());
            let prefix = if inspection.external {
                "Existing unmanaged BepInEx detected; its files will be preserved. "
            } else {
                ""
            };
            binding
                .status
                .set_text(&format!("{prefix}{}{}", inspection.target.description(), if binding.steam_managed { " Steam needs its own winhttp=n,b launch option; Steam configuration will not be changed." } else { "" }));
            binding.restore_controls(inspection.managed);
            glib::ControlFlow::Break
        }
        Ok(Err(error)) => {
            binding
                .status
                .set_text(&format!("BepInEx unavailable: {error}"));
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => {
            binding
                .status
                .set_text("BepInEx runtime detection stopped.");
            glib::ControlFlow::Break
        }
    });
}
