use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

const PLUGIN: &str = "heroic-store";
const METADATA_DIR: &str = "~/.config/legendary/metadata";

fn expand(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~") {
        PathBuf::from(std::env::var("HOME").unwrap()).join(rest.trim_start_matches('/'))
    } else {
        PathBuf::from(s)
    }
}

fn plugin_exe() -> PathBuf {
    let home = std::env::var("HOME").unwrap();
    PathBuf::from(home).join(".local/share/CorkyTux/plugins").join(PLUGIN).join(PLUGIN)
}

fn cache_inventory() -> Vec<(String, String, usize)> {
    let dir = expand(METADATA_DIR);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(txt) = std::fs::read_to_string(&p) else { continue };
            let Ok(d) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
            let app = d.get("app_name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let title = d.get("app_title").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let m = d.get("metadata").cloned().unwrap_or_default();
            let desc = |k: &str| m.get(k).and_then(|x| x.as_str()).unwrap_or("").len();
            out.push((app, title, desc("description") + desc("shortDescription") + desc("longDescription")));
        }
    }
    out.sort();
    out
}

fn run_game_info(app_id: &str) -> Result<String, String> {
    let out = Command::new(plugin_exe())
        .args(["game-info", "--store", "epic", "--app-id", app_id])
        .output()
        .map_err(|e| format!("no se pudo ejecutar plugin: {e}"))?;
    let body = String::from_utf8_lossy(&out.stdout);
    let mut final_doc: Option<serde_json::Value> = None;
    let mut last: Option<serde_json::Value> = None;
    for line in body.lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if v.is_object() {
                if v.get("ok").is_some() || v.get("type").is_some() {
                    final_doc = Some(v);
                } else {
                    last = Some(v);
                }
            }
        }
    }
    match final_doc.or(last) {
        Some(doc) => {
            if doc.get("type").and_then(|t| t.as_str()) == Some("error")
                || doc.get("ok").and_then(|o| o.as_bool()) == Some(false)
            {
                Err(doc.get("message").and_then(|m| m.as_str()).unwrap_or("err").to_string())
            } else {
                Ok(doc.to_string())
            }
        }
        None => Err(format!("stdout sin JSON: {}", body.trim().chars().take(80).collect::<String>())),
    }
}

fn main() {
    let inv = cache_inventory();
    println!("app_id                              title                         cache_len  info_desc_len  final_empty  head");
    for (app, title, cache_len) in inv.iter() {
        match run_game_info(app) {
            Ok(doc) => {
                let v: serde_json::Value = serde_json::from_str(&doc).unwrap_or_default();
                let info = v.get("info").cloned().unwrap_or_default();
                let t = info.get("title").and_then(|x| x.as_str()).unwrap_or(title).to_string();
                let desc = info.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let head = desc.chars().take(34).collect::<String>();
                println!(
                    "{:<38} {:<30} {:<9} {:<15} {:<11} {}",
                    app, t, cache_len, desc.len(), desc.is_empty(), head
                );
            }
            Err(e) => println!("{:<38} {:<30} {:<9} ERR: {}", app, title, cache_len, e),
        }
    }
}