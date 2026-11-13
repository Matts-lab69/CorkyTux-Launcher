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
