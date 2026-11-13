#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::config::IniFile;
    use std::collections::HashMap;
    use std::process::Command;

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
