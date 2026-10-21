//! Strip secrets from anything the launcher writes to a log or shows in a
//! message.
//!
//! The patterns are the same ones the CorkyTux-Plugins Minecraft plugin
//! uses: the launcher and the plugins handle the same secrets (Minecraft
//! access tokens, the Ely.by password, API keys, the Epic exchange code).
//! The launcher adds the `-auth_password` family, because it builds the
//! Epic Online Services launch arguments itself.

use regex::Regex;
use std::sync::OnceLock;

pub const REDACTED: &str = "<REDACTED>";

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
        let out = red("java -jar game.jar --accessToken eyJhbGciOi.superSecret");
        assert!(!out.contains("eyJhbGciOi.superSecret"), "{}", out);
        assert!(out.contains("--accessToken <REDACTED>"), "{}", out);
    }

    #[test]
    fn cli_password_is_redacted() {
        let out = red("minecraft-launcher ely-auth --password hunter2");
        assert!(!out.contains("hunter2"), "{}", out);
        assert!(out.contains("--password <REDACTED>"), "{}", out);
    }

    #[test]
    fn cli_api_key_is_redacted() {
        let out = red("curse-auth-config --api-key $2a$10$abc");
        assert!(!out.contains("$2a$10$abc"), "{}", out);
        assert!(out.contains("--api-key <REDACTED>"), "{}", out);
    }

    #[test]
    fn json_token_fields_are_redacted() {
        let out = red(r#"{"accessToken": "abc123", "clientToken": "def456", "refresh_token": "ghi789"}"#);
        for secret in ["abc123", "def456", "ghi789"] {
            assert!(!out.contains(secret), "{}", out);
        }
        assert_eq!(out.matches(REDACTED).count(), 3, "{}", out);
    }

    #[test]
    fn json_secret_fields_are_redacted() {
        let out = red(r#"{"password": "hunter2", "api_key": "key-123", "user_code": "ABCD-EFGH"}"#);
        for secret in ["hunter2", "key-123", "ABCD-EFGH"] {
            assert!(!out.contains(secret), "{}", out);
        }
        assert_eq!(out.matches(REDACTED).count(), 3, "{}", out);
    }

    #[test]
    fn authorization_bearer_is_redacted() {
        let out = red("Authorization: Bearer eyJraWQiOiJzZWNyZXQ");
        assert!(!out.contains("eyJraWQiOiJzZWNyZXQ"), "{}", out);
        assert!(out.contains("Bearer <REDACTED>"), "{}", out);
    }

    #[test]
    fn x_api_key_header_is_redacted() {
        let out = red("x-api-key: $2a$10$abcdef");
        assert!(!out.contains("$2a$10$abcdef"), "{}", out);
        assert!(out.contains("x-api-key: <REDACTED>"), "{}", out);
    }

    #[test]
    fn xbox_tokens_are_redacted() {
        let rps = red(r#"{"RpsTicket": "d=microsoft_access_token"}"#);
        assert!(!rps.contains("microsoft_access_token"), "{}", rps);
        let xbl = red(r#"{"identityToken": "XBL3.0 x=uhs_value;xsts_token"}"#);
        assert!(!xbl.contains("xsts_token"), "{}", xbl);
        assert!(!xbl.contains("uhs_value"), "{}", xbl);
    }

    #[test]
    fn eos_exchange_code_is_redacted() {
        let out = red(" -auth_login=unused -auth_password=abc123exchange -auth_type=exchangecode");
        assert!(!out.contains("abc123exchange"), "{}", out);
        assert!(!out.contains("unused"), "{}", out);
        assert!(out.contains("-auth_password=<REDACTED>"), "{}", out);
        assert!(out.contains("-auth_login=<REDACTED>"), "{}", out);
        // The rest of the line stays readable.
        assert!(out.contains("-auth_type=exchangecode"), "{}", out);
    }

    #[test]
    fn eos_username_is_redacted() {
        let out = red("-auth_username=PlayerOne -epicuserid=1234");
        assert!(!out.contains("PlayerOne"), "{}", out);
        assert!(out.contains("-auth_username=<REDACTED>"), "{}", out);
    }

    #[test]
    fn a_plain_line_is_untouched() {
        let out = red("[CorkyTux] Exit with return code 0");
        assert_eq!(out, "[CorkyTux] Exit with return code 0");
    }
}
