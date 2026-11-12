//! Real loopback HTTP coverage for cache validators and redirect statuses.

use super::*;
use std::io::{Read, Write};
use std::sync::mpsc;

fn replies(responses: Vec<Vec<u8>>) -> (String, mpsc::Receiver<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            // Bound the fixture lifetime even if a rejected redirect is not followed.
            listener.set_nonblocking(true).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            let _ = tx.send(requests);
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(error) => panic!("fixture accept failed: {error}"),
                }
            };
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 16 * 1024, "fixture request too large");
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            requests.push(String::from_utf8(request).unwrap());
            stream.write_all(&response).unwrap();
        }
        let _ = tx.send(requests);
    });
    (format!("http://{address}/latest"), rx)
}

fn status_response(status: u16, location: Option<&str>) -> Vec<u8> {
    let location = location.map(|value| format!("Location: {value}\r\n")).unwrap_or_default();
    format!("HTTP/1.1 {status} Test\r\n{location}Content-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}

#[test]
fn not_modified_without_location_reaches_cached_release_decision() {
    let _environment = crate::test_env::lock();
    let home = std::env::temp_dir().join(format!("corky-http304-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let cache = home.join("release-cache");
    let cached_json = r#"{"tag_name":"v3.2.1","draft":false,"prerelease":false,"body":"cached","assets":[]}"#;
    store_cached_latest(&cache, cached_json).unwrap();
    let before = std::fs::read(cache.join(CACHED_LATEST_FILE)).unwrap();
    let (url, requests) = replies(vec![status_response(304, None)]);
    let _api = ApiUrlGuard::set(url);

    let outcome = api_get_latest(Some("\"cached-release\""));
    println!("actual API outcome: {outcome:?}; expected: Ok(NotModified), no Location required");
    assert!(matches!(outcome, Ok(ApiOutcome::NotModified)), "304 must reach cache handling: {outcome:?}");
    let info = read_cached_latest(&cache).expect("304 must retain usable cached metadata");
    assert_eq!(info.tag, "v3.2.1");
    assert_eq!(decide_update(&semver::Version::parse("3.2.0").unwrap(), &info, ""), UpdateDecision::Available);
    assert_eq!(decide_update(&semver::Version::parse("3.2.1").unwrap(), &info, ""), UpdateDecision::UpToDate);
    assert_eq!(decide_update(&semver::Version::parse("3.2.0").unwrap(), &info, "v3.2.1"), UpdateDecision::UpToDate);
    assert_eq!(std::fs::read(cache.join(CACHED_LATEST_FILE)).unwrap(), before);
    let captured = requests.recv_timeout(std::time::Duration::from_secs(6)).unwrap();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].to_ascii_lowercase().contains("if-none-match: \"cached-release\""));
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn not_modified_ignores_location_and_preserves_missing_cache_state() {
    let _environment = crate::test_env::lock();
    let home = std::env::temp_dir().join(format!("corky-http304-empty-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let (url, requests) = replies(vec![status_response(304, Some("https://evil.example/not-a-redirect"))]);
    let _api = ApiUrlGuard::set(url);
    assert!(matches!(api_get_latest(None), Ok(ApiOutcome::NotModified)));
    assert!(read_cached_latest(&home.join("no-cache")).is_none());
    assert_eq!(requests.recv_timeout(std::time::Duration::from_secs(6)).unwrap().len(), 1);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn supported_redirect_statuses_require_location() {
    for status in [301, 302, 303, 307, 308] {
        let (url, requests) = replies(vec![status_response(status, None)]);
        let error = get_guarded(&http_client().unwrap(), &url, None, API_HOSTS, MAX_REDIRECTS).unwrap_err();
        assert!(error.contains(&format!("HTTP {status}")), "{error}");
        assert!(error.contains("Location"), "{error}");
        assert_eq!(requests.recv_timeout(std::time::Duration::from_secs(6)).unwrap().len(), 1);
    }
}

#[test]
fn supported_redirect_statuses_follow_location() {
    for status in [301, 302, 303, 307, 308] {
        let (url, requests) = replies(vec![status_response(status, Some("/metadata")), status_response(200, None)]);
        let response = get_guarded(&http_client().unwrap(), &url, None, API_HOSTS, MAX_REDIRECTS).unwrap();
        assert_eq!(response.status().as_u16(), 200, "redirect status {status}");
        assert_eq!(requests.recv_timeout(std::time::Duration::from_secs(6)).unwrap().len(), 2);
    }
}

#[test]
fn unsupported_3xx_statuses_are_errors_even_with_location() {
    for status in [300, 305, 306] {
        for location in [None, Some("/must-not-follow")] {
            let (url, requests) = replies(vec![status_response(status, location)]);
            let error = get_guarded(&http_client().unwrap(), &url, None, API_HOSTS, MAX_REDIRECTS).unwrap_err();
            assert!(error.contains(&format!("HTTP {status}")), "{error}");
            assert!(error.contains("unsupported HTTP redirect status"), "{error}");
            assert_eq!(requests.recv_timeout(std::time::Duration::from_secs(6)).unwrap().len(), 1);
        }
    }
}
