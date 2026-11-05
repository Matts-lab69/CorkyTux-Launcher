//! Strip secrets from anything the launcher writes to a log or shows in a
//! message.
//!
//! The patterns are the same ones the CorkyTux-Plugins Minecraft plugin
//! uses: the launcher and the plugins handle the same secrets (Minecraft
//! access tokens, the Ely.by password, API keys, the Epic exchange code).
//! The launcher adds the `-auth_password` family, because it builds the
//! Epic Online Services launch arguments itself.

use regex::Regex;
use std::cmp::Reverse;
use std::sync::{OnceLock, RwLock};

pub const REDACTED: &str = "<REDACTED>";

/// Secrets shorter than this are ignored, to keep normal text safe from
/// overzealous masking.
const MIN_SECRET_LEN: usize = 8;

/// (pattern, replacement). The replacement uses the crate syntax
/// (`${1}` for a group).
const RULES: &[(&str, &str)] = &[
    // --accessToken <value> (also --access_token)
    (
        r#"(?i)(--access[_-]?token)([= ]+)("[^"]*"|'[^']*'|[^\s"']+)"#,
        "${1}${2}<REDACTED>",
    ),
    // --password <value>
    (
        r#"(?i)(--password)([= ]+)("[^"]*"|'[^']*'|[^\s"']+)"#,
        "${1}${2}<REDACTED>",
    ),
    // --api-key <value> (also --api_key)
    (
        r#"(?i)(--api[_-]?key)([= ]+)("[^"]*"|'[^']*'|[^\s"']+)"#,
        "${1}${2}<REDACTED>",
    ),
    // JSON token fields
    (
        r#"(?i)("(?:access[_-]?token|client[_-]?token|refresh[_-]?token|id[_-]?token|identitytoken)"\s*:\s*)"(?:[^"\\]|\\.)*""#,
        "${1}\"<REDACTED>\"",
    ),
    // JSON secret fields: passwords, API keys, auth codes
    (
        r#"(?i)("(?:password|api[_-]?key|auth[_-]?code|authorization[_-]?code|device[_-]?code|user[_-]?code)"\s*:\s*)"(?:[^"\\]|\\.)*""#,
        "${1}\"<REDACTED>\"",
    ),
    // Authorization: Bearer <token>
    (r#"(?i)(authorization\s*:\s*bearer\s+)(\S+)"#, "${1}<REDACTED>"),
    // x-api-key: <key>
    (r#"(?i)(x-api-key\s*:\s*)(\S+)"#, "${1}<REDACTED>"),
    // Xbox RpsTicket (d=<microsoft token>)
    (r#"(?i)(rpsTicket"?\s*[:=]\s*"?d=)(\S+)"#, "${1}<REDACTED>"),
    // Xbox XBL3.0 identity token
    (r#"(XBL3\.0\s+x=[^;\s"]+;)(\S+)"#, "${1}<REDACTED>"),
    // Epic Online Services: -auth_password=<exchange code>
    (
        r#"(?i)(-auth[_-]?password[= ]+)("[^"]*"|'[^']*'|[^\s"']+)"#,
        "${1}<REDACTED>",
    ),
    // Epic Online Services: -auth_login=<login>
    (
        r#"(?i)(-auth[_-]?login[= ]+)("[^"]*"|'[^']*'|[^\s"']+)"#,
        "${1}<REDACTED>",
    ),
    // Epic Online Services: -auth_username=<user>
    (
        r#"(?i)(-auth[_-]?username[= ]+)("[^"]*"|'[^']*'|[^\s"']+)"#,
        "${1}<REDACTED>",
    ),
    // CorkyTux / SteamGridDB integration key, JSON form ("NAME": "value").
    // The value class excludes newlines so an empty value can never reach
    // the next line.
    (
        r#"(?i)("(?:CORKYTUX_STEAMGRIDDB_API_KEY|CORKYTUX_SGDB_BUILTIN_KEY|IntegrationApiKey)"\s*:\s*)"[^"\n]*""#,
        "${1}\"<REDACTED>\"",
    ),
    // CorkyTux / SteamGridDB integration key, labeled assignments:
    // NAME=value, NAME = value, NAME: value, NAME='value', NAME="value".
    // The unquoted value stops at whitespace, `;` and `,` so separators
    // (even inside an error message) survive, and it never crosses a line.
    // Quote pairs stay intact; the branch groups are empty strings in the
    // replacements for the other alternatives.
    (
        r#"(?i)(CORKYTUX_STEAMGRIDDB_API_KEY|CORKYTUX_SGDB_BUILTIN_KEY|IntegrationApiKey)([ \t]*[=:][ \t]*)(?:(')([^'\n]*)(')|(")([^"\n]*)(")|([^;,\s"'\n]+))"#,
        "${1}${2}${3}${6}<REDACTED>${5}${8}",
    ),
];

fn compiled() -> &'static Vec<(Regex, &'static str)> {
    static RULES_COMPILED: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    RULES_COMPILED.get_or_init(|| {
        RULES
            .iter()
            .map(|(pattern, replacement)| {
                (
                    Regex::new(pattern).expect("the redaction patterns must compile"),
                    *replacement,
                )
            })
            .collect()
    })
}

/// Registered runtime secrets (the SGDB key and any other value the
/// launcher learns at runtime). Guarded by an `RwLock` behind an
/// `OnceLock`; no external dependency.
fn registered_secrets() -> &'static RwLock<Vec<String>> {
    static REGISTRY: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// The built-in SteamGridDB key baked in at compile time, if any. Cached
/// in an `OnceLock`; no IO beyond the compile-time environment the
/// compiler sees.
fn builtin_secret() -> Option<&'static str> {
    static BUILTIN: OnceLock<Option<String>> = OnceLock::new();
    BUILTIN
        .get_or_init(|| {
            let value = option_env!("CORKYTUX_SGDB_BUILTIN_KEY")?;
            let trimmed = value.trim();
            if trimmed.chars().count() >= MIN_SECRET_LEN {
                Some(trimmed.to_string())
            } else {
                None
            }
        })
        .as_deref()
}

/// Register one secret value to mask everywhere bare (without any
/// label), as inside error messages. Idempotent and thread-safe. Values
/// that are empty, blank or shorter than [`MIN_SECRET_LEN`] characters
/// are ignored so normal text is never over-masked.
pub fn register_secret(secret: &str) {
    let trimmed = secret.trim();
    if trimmed.chars().count() < MIN_SECRET_LEN {
        return;
    }
    let registry = registered_secrets();
    let mut list = match registry.write() {
        Ok(list) => list,
        Err(poisoned) => poisoned.into_inner(),
    };
    if !list.iter().any(|known| known == trimmed) {
        list.push(trimmed.to_string());
    }
}

/// Remove the secrets from a free text: a log line, an error message, the
/// content of a saved log.
pub fn redact_secrets(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut out = text.to_string();
    for (pattern, replacement) in compiled() {
        out = pattern.replace_all(&out, *replacement).into_owned();
    }
    let secrets: Vec<String> = {
        let registry = registered_secrets();
        let list = match registry.read() {
            Ok(list) => list,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut list = list.clone();
        if let Some(builtin) = builtin_secret() {
            let builtin = builtin.to_string();
            if !list.iter().any(|known| *known == builtin) {
                list.push(builtin);
            }
        }
        // Longest first, so prefixes are masked in one pass.
        list.sort_by_key(|secret| Reverse(secret.chars().count()));
        list
    };
    for secret in &secrets {
        if let Ok(escaped) = Regex::new(&regex::escape(secret)) {
            out = escaped.replace_all(&out, REDACTED.to_string()).into_owned();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red(text: &str) -> String {
        redact_secrets(text)
    }

    #[test]
    fn cli_access_token_is_redacted() {
        let out = red("java -jar game.jar --accessToken ACCESS_TOKEN_VALUE");
        assert!(!out.contains("ACCESS_TOKEN_VALUE"), "redaction failed");
        assert!(out.contains("--accessToken <REDACTED>"), "redaction failed");
    }

    #[test]
    fn cli_password_is_redacted() {
        let out = red("minecraft-launcher ely-auth --password PASSWORD_VALUE");
        assert!(!out.contains("PASSWORD_VALUE"), "redaction failed");
        assert!(out.contains("--password <REDACTED>"), "redaction failed");
    }

    #[test]
    fn cli_api_key_is_redacted() {
        let out = red("curse-auth-config --api-key API_KEY_VALUE");
        assert!(!out.contains("API_KEY_VALUE"), "redaction failed");
        assert!(out.contains("--api-key <REDACTED>"), "redaction failed");
    }

    #[test]
    fn json_token_fields_are_redacted() {
        let out = red(r#"{"accessToken": "ACCESS_TOKEN_VALUE", "clientToken": "CLIENT_TOKEN_VALUE", "refresh_token": "REFRESH_TOKEN_VALUE"}"#);
        for secret in ["ACCESS_TOKEN_VALUE", "CLIENT_TOKEN_VALUE", "REFRESH_TOKEN_VALUE"] {
            assert!(!out.contains(secret), "redaction failed");
        }
        assert!(out.matches(REDACTED).count() == 3, "redaction failed");
    }

    #[test]
    fn json_secret_fields_are_redacted() {
        let out = red(r#"{"password": "PASSWORD_VALUE", "api_key": "API_KEY_VALUE", "user_code": "USER_CODE_VALUE"}"#);
        for secret in ["PASSWORD_VALUE", "API_KEY_VALUE", "USER_CODE_VALUE"] {
            assert!(!out.contains(secret), "redaction failed");
        }
        assert!(out.matches(REDACTED).count() == 3, "redaction failed");
    }

    #[test]
    fn authorization_bearer_is_redacted() {
        let out = red("Authorization: Bearer BEARER_TOKEN_VALUE");
        assert!(!out.contains("BEARER_TOKEN_VALUE"), "redaction failed");
        assert!(out.contains("Bearer <REDACTED>"), "redaction failed");
    }

    #[test]
    fn x_api_key_header_is_redacted() {
        let out = red("x-api-key: API_KEY_VALUE");
        assert!(!out.contains("API_KEY_VALUE"), "redaction failed");
        assert!(out.contains("x-api-key: <REDACTED>"), "redaction failed");
    }

    #[test]
    fn xbox_tokens_are_redacted() {
        let rps = red(r#"{"RpsTicket": "d=MICROSOFT_TOKEN_VALUE"}"#);
        assert!(!rps.contains("MICROSOFT_TOKEN_VALUE"), "redaction failed");
        let xbl = red(r#"{"identityToken": "XBL3.0 x=USER_HASH_VALUE;XSTS_TOKEN_VALUE"}"#);
        assert!(!xbl.contains("XSTS_TOKEN_VALUE"), "redaction failed");
        assert!(!xbl.contains("USER_HASH_VALUE"), "redaction failed");
    }

    #[test]
    fn eos_exchange_code_is_redacted() {
        let out = red(" -auth_login=LOGIN_VALUE -auth_password=EXCHANGE_CODE_VALUE -auth_type=exchangecode");
        assert!(!out.contains("EXCHANGE_CODE_VALUE"), "redaction failed");
        assert!(!out.contains("LOGIN_VALUE"), "redaction failed");
        assert!(out.contains("-auth_password=<REDACTED>"), "redaction failed");
        assert!(out.contains("-auth_login=<REDACTED>"), "redaction failed");
        // The rest of the line stays readable.
        assert!(out.contains("-auth_type=exchangecode"), "redaction failed");
    }

    #[test]
    fn eos_username_is_redacted() {
        let out = red("-auth_username=USERNAME_VALUE -epicuserid=USER_ID_VALUE");
        assert!(!out.contains("USERNAME_VALUE"), "redaction failed");
        assert!(out.contains("-auth_username=<REDACTED>"), "redaction failed");
    }

    #[test]
    fn a_plain_line_is_untouched() {
        let out = red("[CorkyTux] Exit with return code 0");
        assert!(out == "[CorkyTux] Exit with return code 0", "redaction failed");
    }

    #[test]
    fn sgdb_named_values_are_redacted_without_losing_labels() {
        for name in ["CORKYTUX_STEAMGRIDDB_API_KEY", "CORKYTUX_SGDB_BUILTIN_KEY", "IntegrationApiKey"] {
            for label in [name.to_string(), name.to_lowercase()] {
                for (input, expected) in [
                    (format!("{label}=SGDB_VALUE"), format!("{label}={REDACTED}")),
                    (format!("{label} = 'SGDB_VALUE'"), format!("{label} = '{REDACTED}'")),
                    (format!(r#"{{"{label}": "SGDB_VALUE"}}"#), format!(r#"{{"{label}": "{REDACTED}"}}"#)),
                    (format!("error: {label}=SGDB_VALUE; request failed"), format!("error: {label}={REDACTED}; request failed")),
                ] {
                    assert!(red(&input) == expected, "named redaction failed");
                }
            }
        }
    }

    #[test]
    fn empty_named_values_do_not_consume_the_next_line() {
        for name in ["CORKYTUX_STEAMGRIDDB_API_KEY", "CORKYTUX_SGDB_BUILTIN_KEY", "IntegrationApiKey"] {
            for input in [format!("{name}= \nnext_line"), format!("{name}:\r\nnext_line"), format!("{name}=\"\"\nnext_line")] {
                assert!(red(&input).ends_with("\nnext_line"), "line boundary changed");
            }
        }
    }

    #[test]
    fn builtin_absence_does_not_break_redaction() {
        // With no baked-in key the helper must be a no-op, not a panic.
        let out = red("[CorkyTux] SteamGridDB lookup started");
        assert!(out == "[CorkyTux] SteamGridDB lookup started", "redaction failed");
    }

    #[test]
    fn registered_secret_is_redacted_when_naked() {
        register_secret("RUNTIME_SECRET_VALUE");
        let out = red("loaded integration token RUNTIME_SECRET_VALUE ready");
        assert!(!out.contains("RUNTIME_SECRET_VALUE"), "redaction failed");
        assert!(out.contains(REDACTED), "redaction failed");
    }

    #[test]
    fn registered_secret_inside_request_error_is_redacted() {
        register_secret("REQERR_SECRET_VALUE");
        let out = red("error sending request for url (https://api.example): REQERR_SECRET_VALUE now retrying");
        assert!(!out.contains("REQERR_SECRET_VALUE"), "redaction failed");
        assert!(out.contains("error sending request for url (https://api.example): <REDACTED> now retrying"), "redaction failed");
    }

    #[test]
    fn registered_secret_with_metacharacters_is_redacted_literally() {
        register_secret("A.B*(C)[D]_x");
        let out = red("weird value A.B*(C)[D]_x kept");
        assert!(!out.contains("A.B*(C)[D]_x"), "redaction failed");
        assert!(out.contains("weird value <REDACTED> kept"), "redaction failed");
    }

    #[test]
    fn short_or_blank_registration_does_not_change_output() {
        register_secret("");
        register_secret("   ");
        register_secret("SHORT");
        let out = red("plain line SHORT values stay visible");
        assert!(out == "plain line SHORT values stay visible", "redaction failed");
    }
}
