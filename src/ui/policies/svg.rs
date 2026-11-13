//! Every shipped SVG must declare a viewBox, otherwise GTK scales it from
//! its intrinsic size and the icon is clipped at 48 px.

#[cfg(test)]
mod tests {
    fn svg_has_viewbox(path: &std::path::Path) -> bool {
        let Ok(text) = std::fs::read_to_string(path) else {
            return false;
        };
        let Some(start) = text.find("<svg") else {
            return false;
        };
        let Some(end) = text[start..].find('>') else {
            return false;
        };
        text[start..start + end].contains("viewBox=")
    }

    fn walk_svgs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_svgs(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("svg") {
                out.push(path);
            }
        }
    }

    #[test]
    fn every_svg_declares_a_viewbox() {
        let mut svgs = Vec::new();
        walk_svgs(
            &std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")),
            &mut svgs,
        );
        assert!(!svgs.is_empty(), "no SVGs found");
        for svg in svgs {
            assert!(svg_has_viewbox(&svg), "missing viewBox: {}", svg.display());
        }
    }
}
