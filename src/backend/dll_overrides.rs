use std::collections::HashMap;
use std::process::Command;

use super::config::IniFile;

pub const KEY: &str = "custom_dll_overrides";
pub const LEGACY_KEY: &str = "Overrides";

/// Copy legacy values once; retain old and unrelated keys for compatibility.
pub fn migrate(ini: &mut IniFile) -> bool {
    let mut changed = false;
    for game in ini.section_names() {
        if ini.get_value(&game, KEY).is_none() {
            if let Some(value) = ini.get_value(&game, LEGACY_KEY) {
                ini.set_value(&game, KEY, &value);
                changed = true;
            }
        }
    }
    changed
}

/// Validate Wine's semicolon-separated DLL lists and native/builtin load orders.
pub fn validate(value: &str) -> Result<(), String> {
    if value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err("DLL overrides must be a single line of at most 16384 bytes.".into());
    }
    for clause in value
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (libraries, order) = clause.split_once('=').ok_or_else(|| {
            "Use DLL names followed by a load order, for example winhttp=n,b.".to_string()
        })?;
        if libraries.split(',').any(|name| {
            let name = name.trim();
            name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '*'))
                || name == "."
                || name == ".."
        }) {
            return Err("DLL names must not contain paths or empty list items.".into());
        }
        if !order.trim().is_empty()
            && order.split(',').any(|mode| {
                !matches!(
                    mode.trim().to_ascii_lowercase().as_str(),
                    "n" | "b" | "native" | "builtin"
                )
            })
        {
            return Err(
                "Use n (native), b (builtin), or an empty load order to disable a DLL.".into(),
            );
        }
    }
    Ok(())
}

/// Set an explicit value without invoking a shell; an empty new value wins too.
pub fn apply(command: &mut Command, game: &HashMap<String, String>) -> Result<(), String> {
    if let Some(value) = game.get(KEY).or_else(|| game.get("overrides")) {
        validate(value)?;
        command.env("WINEDLLOVERRIDES", value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::config::IniFile;
    use std::collections::HashMap;
    use std::process::Command;

    #[test]
    fn custom_dll_overrides_test_constructor_migrates_only_explicit_directory() {
        let root =
            std::env::temp_dir().join(format!("custom-dll-migration-{}", std::process::id()));
        let home = root.join("home");
        let config = home.join(".config/CorkyTux");
        let explicit = root.join("explicit");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&explicit).unwrap();
        let original = "[Game]\nOverrides=winhttp=n,b\nUnrelated=keep\n";
        std::fs::write(config.join("Games.ini"), original).unwrap();
        std::fs::write(explicit.join("Games.ini"), original).unwrap();
        let _home = crate::test_env::HomeGuard::set(&home);
        let manager = crate::backend::config::ConfigManager::for_test_dir(&explicit);
        assert_eq!(
            std::fs::read_to_string(config.join("Games.ini")).unwrap(),
            original
        );
        assert_eq!(
            manager.game_value("Game", KEY).as_deref(),
            Some("winhttp=n,b")
        );
        let saved = IniFile::parse(&std::fs::read_to_string(explicit.join("Games.ini")).unwrap());
        assert_eq!(saved.get_value("Game", KEY).as_deref(), Some("winhttp=n,b"));
        assert_eq!(
            saved.get_value("Game", "Unrelated").as_deref(),
            Some("keep")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn custom_dll_overrides_migrate_legacy_values_without_loss() {
        let mut ini = IniFile::parse("[Legacy]\nOverrides=winhttp=n,b;dxgi=b\nUnknown=keep\n[New]\ncustom_dll_overrides=dinput8=n\nOverrides=version=b\n[Empty]\ncustom_dll_overrides=\nOverrides=winhttp=n,b\n");
        assert!(migrate(&mut ini));
        assert_eq!(
            ini.get_value("Legacy", KEY).as_deref(),
            Some("winhttp=n,b;dxgi=b")
        );
        assert_eq!(ini.get_value("Legacy", "Unknown").as_deref(), Some("keep"));
        assert_eq!(ini.get_value("New", KEY).as_deref(), Some("dinput8=n"));
        assert_eq!(
            ini.get_value("New", LEGACY_KEY).as_deref(),
            Some("version=b")
        );
        assert_eq!(ini.get_value("Empty", KEY).as_deref(), Some(""));
        assert!(!migrate(&mut ini));
        let reloaded = IniFile::parse(&ini.to_string());
        assert_eq!(
            reloaded.get_value("Legacy", KEY),
            ini.get_value("Legacy", KEY)
        );
    }

    #[test]
    fn custom_dll_overrides_migrate_case_variants_and_preserve_invalid_legacy() {
        let mut ini = IniFile::parse("[Game]\noVeRrIdEs=legacy unparsed value\n");
        assert!(migrate(&mut ini));
        assert_eq!(
            ini.get_value("Game", KEY).as_deref(),
            Some("legacy unparsed value")
        );
    }

    #[test]
    fn custom_dll_overrides_validate_wine_load_orders() {
        for value in [
            "",
            "winhttp=n,b",
            "d3d11,dxgi=n,b;version=b",
            "*=builtin;foo=",
            " winhttp = native, builtin ;",
        ] {
            assert!(validate(value).is_ok(), "valid value rejected: {value}");
        }
        for value in [
            "winhttp",
            "=n,b",
            "winhttp=x",
            "winhttp=n,,b",
            "winhttp=n=b",
            "../winhttp=n",
            "winhttp=n\nPATH=x",
            "foo,,bar=n",
        ] {
            assert!(validate(value).is_err(), "invalid value accepted: {value}");
        }
    }

    #[test]
    fn custom_dll_overrides_apply_without_executing_payloads() {
        let mut game = HashMap::from([(LEGACY_KEY.to_lowercase(), "version=b".to_string())]);
        let mut cmd = Command::new("unused-test-command");
        apply(&mut cmd, &game).unwrap();
        assert_eq!(
            cmd.get_envs()
                .find(|(k, _)| *k == "WINEDLLOVERRIDES")
                .unwrap()
                .1
                .unwrap(),
            "version=b"
        );
        game.insert(KEY.to_string(), "winhttp=n,b".to_string());
        apply(&mut cmd, &game).unwrap();
        assert_eq!(
            cmd.get_envs()
                .find(|(k, _)| *k == "WINEDLLOVERRIDES")
                .unwrap()
                .1
                .unwrap(),
            "winhttp=n,b"
        );
        game.insert(KEY.to_string(), String::new());
        apply(&mut cmd, &game).unwrap();
        assert_eq!(
            cmd.get_envs()
                .find(|(k, _)| *k == "WINEDLLOVERRIDES")
                .unwrap()
                .1
                .unwrap(),
            ""
        );
        game.insert(KEY.to_string(), "winhttp=invalid".to_string());
        assert!(apply(&mut cmd, &game).is_err());
    }
}
