mod bepinex_settings;
pub mod helpers;

pub type RefreshCallback = std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>>>;
pub type CallbackSlot<T> = std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<dyn Fn(T)>>>>;
pub mod add_game;
pub mod center;
pub mod deps_modal;
pub mod details_panel;
pub mod game_card;
#[cfg(test)]
#[path = "policies/input_prompts.rs"]
mod input_prompts_contract;
#[path = "policies/layout.rs"]
pub mod layout;
pub mod remove_modal;
pub mod settings;
pub mod sidebar;
#[path = "policies/svg.rs"]
pub mod svg_assets;
pub use crate::backend::icon_sources as icons;
pub mod apps_settings;
#[path = "policies/dialog.rs"]
pub mod dialog;
pub mod game_settings;
pub mod gamepad;
mod gamepad_controls;
pub mod gamepad_nav;
pub mod import_manager;
pub mod log_modal;
#[path = "policies/markdown.rs"]
pub mod markdown;
pub mod mcx_theme;
mod minecraft_ram;
pub mod minecraft_view;
mod rail_loader;
pub mod stores_view;
pub mod tray;
pub mod update_dialog;
pub mod warnings;
