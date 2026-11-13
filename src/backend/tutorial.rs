//! Pure first-use tutorial progress; interrupted tours count as already seen.

use super::config::ConfigManager;

pub const KEY: &str = "FirstUseTutorialState";
pub const ACTION: &str = "view-tutorial";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeenState {
    Unseen,
    Started,
    Completed,
    Skipped,
}

impl SeenState {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("started") => Self::Started,
            Some("completed") => Self::Completed,
            Some("skipped") => Self::Skipped,
            _ => Self::Unseen,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unseen => "unseen",
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Library,
    Add,
    GameSettings,
    Controller,
    Tray,
    Help,
}

impl Step {
    pub const ALL: [Self; 6] = [
        Self::Library,
        Self::Add,
        Self::GameSettings,
        Self::Controller,
        Self::Tray,
        Self::Help,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Library => "Your library",
            Self::Add => "Add games and stores",
            Self::GameSettings => "Per-game settings",
            Self::Controller => "Controller navigation",
            Self::Tray => "Closing and the tray",
            Self::Help => "Help and this tutorial",
        }
    }
    pub fn body(self) -> &'static str {
        match self {
            Self::Library => "Your library lists added and imported games. Search and filters help you find a title.",
            Self::Add => "Use Add game for local games. Enabled store integrations appear in the sidebar.",
            Self::GameSettings => "Select a game to configure Proton and editable DLL overrides. Supported mods add their own options.",
            Self::Controller => "A controller moves focus between the same controls. Confirm activates; back leaves a dialog or hint.",
            Self::Tray => "Closing hides the window only while a tray host is available. Use Quit to exit; without a tray, closing exits.",
            Self::Help => "Read the project README for setup and troubleshooting. Settings → View tutorial reopens these hints.",
        }
    }
}

pub struct Tour {
    state: SeenState,
    cursor: usize,
    active: bool,
}

impl Tour {
    pub fn new(value: Option<&str>) -> Self {
        Self {
            state: SeenState::parse(value),
            cursor: 0,
            active: false,
        }
    }
    pub fn should_auto_start(&self) -> bool {
        self.state == SeenState::Unseen
    }
    pub fn status(&self) -> SeenState {
        self.state
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn start(&mut self, manual: bool) -> bool {
        if !manual && !self.should_auto_start() {
            return false;
        }
        self.state = SeenState::Started;
        self.cursor = 0;
        self.active = true;
        true
    }
    pub fn current(&mut self, available: &[bool; 6]) -> Option<Step> {
        if !self.active {
            return None;
        }
        while self.cursor < Step::ALL.len() && !available[self.cursor] {
            self.cursor += 1;
        }
        if self.cursor == Step::ALL.len() {
            self.active = false;
            self.state = SeenState::Completed;
            None
        } else {
            Some(Step::ALL[self.cursor])
        }
    }
    pub fn next(&mut self, available: &[bool; 6]) -> Option<Step> {
        if self.active {
            self.cursor = self.cursor.saturating_add(1);
        }
        self.current(available)
    }
    pub fn dismiss(&mut self) {
        self.active = false;
        self.state = SeenState::Skipped;
    }
}

pub fn save_state(config: &ConfigManager, state: SeenState) -> Result<(), String> {
    config.set_launcher_value_in("User Settings", KEY, state.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_completed_and_interrupted_states_do_not_auto_repeat() {
        for value in ["started", "completed", "skipped"] {
            assert!(!Tour::new(Some(value)).should_auto_start());
        }
        let mut tour = Tour::new(None);
        assert!(tour.start(false));
        for _ in 0..6 {
            tour.next(&[true; 6]);
        }
        assert_eq!(tour.status(), SeenState::Completed);
        assert!(!tour.active());
        assert!(!tour.start(false));
    }

    #[test]
    fn tutorial_skip_escape_and_window_hide_are_seen() {
        let mut tour = Tour::new(None);
        assert!(tour.start(false));
        tour.dismiss();
        tour.dismiss();
        assert_eq!(tour.status(), SeenState::Skipped);
        assert!(!tour.active());
        assert!(!Tour::new(Some(tour.status().as_str())).should_auto_start());
    }

    #[test]
    fn tutorial_settings_reopen_restarts_completed_or_skipped_steps() {
        for value in ["started", "completed", "skipped"] {
            let mut tour = Tour::new(Some(value));
            assert!(tour.start(true));
            assert_eq!(tour.current(&[true; 6]), Some(Step::Library));
            assert!(tour.active());
            assert_eq!(tour.status(), SeenState::Started);
        }
    }

    #[test]
    fn tutorial_missing_targets_are_skipped_without_blocking() {
        let mut tour = Tour::new(None);
        tour.start(false);
        assert_eq!(
            tour.current(&[false, true, false, false, false, true]),
            Some(Step::Add)
        );
        assert_eq!(
            tour.next(&[false, true, false, false, false, true]),
            Some(Step::Help)
        );
        assert_eq!(tour.next(&[false; 6]), None);
        assert_eq!(tour.status(), SeenState::Completed);
        let mut empty = Tour::new(None);
        empty.start(false);
        assert_eq!(empty.current(&[false; 6]), None);
        assert!(!empty.active());
    }

    #[test]
    fn tutorial_progress_is_saved_in_an_isolated_configuration() {
        let root = std::env::temp_dir().join(format!("first-use-tutorial-{}", std::process::id()));
        let home = root.join("home");
        let config = root.join("config");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        let _home = crate::test_env::HomeGuard::set(&home);
        let manager = crate::backend::config::ConfigManager::for_test_dir(&config);
        for state in [SeenState::Started, SeenState::Skipped, SeenState::Completed] {
            save_state(&manager, state).unwrap();
            let reloaded = crate::backend::config::ConfigManager::for_test_dir(&config);
            assert_eq!(
                reloaded.launcher_value(KEY).as_deref(),
                Some(state.as_str())
            );
            assert!(!Tour::new(reloaded.launcher_value(KEY).as_deref()).should_auto_start());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
