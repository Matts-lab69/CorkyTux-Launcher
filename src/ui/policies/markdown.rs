//! The safe markdown subset used for plugin and mod descriptions: images and
//! links, bold and italic, dashes as bullets. Everything is escaped first, so
//! the input cannot inject Pango markup.

/// Minimal, safe Markdown for plugin descriptions: bold, italic, list items,
/// line breaks and http(s) links as Pango markup. Everything else is escaped,
/// images are dropped, and non-http schemes are never turned into links.
pub fn markdown_to_pango(input: &str) -> String {
    use std::sync::OnceLock;
    static IMG: OnceLock<regex::Regex> = OnceLock::new();
    static LINK: OnceLock<regex::Regex> = OnceLock::new();
    static BOLD: OnceLock<regex::Regex> = OnceLock::new();
    static ITALIC: OnceLock<regex::Regex> = OnceLock::new();
    let img = IMG.get_or_init(|| regex::Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap());
    let link = LINK.get_or_init(|| regex::Regex::new(r"\[([^\]]+)\]\((https?://[^)\s]+)\)").unwrap());
    let bold = BOLD.get_or_init(|| regex::Regex::new(r"\*\*([^*]+)\*\*").unwrap());
    let italic = ITALIC.get_or_init(|| regex::Regex::new(r"\*([^*]+)\*").unwrap());
    let mut out = String::new();
    for (i, line) in input.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            out.push_str("• ");
            out.push_str(&markdown_inline(rest, img, link, bold, italic));
        } else {
            out.push_str(&markdown_inline(line, img, link, bold, italic));
        }
    }
    out
}

fn markdown_inline(
    text: &str,
    img: &regex::Regex,
    link: &regex::Regex,
    bold: &regex::Regex,
    italic: &regex::Regex,
) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let without_images = img.replace_all(&escaped, "");
    let with_links = link.replace_all(&without_images, "<a href=\"$2\">$1</a>");
    let with_bold = bold.replace_all(&with_links, "<b>$1</b>");
    italic.replace_all(&with_bold, "<i>$1</i>").to_string()
}

#[cfg(test)]
mod tests {
    use super::markdown_to_pango;

    #[test]
    fn markdown_subset_is_safe() {
        use super::markdown_to_pango;
        assert_eq!(markdown_to_pango("<script>alert(1)</script>"),
                   "&lt;script&gt;alert(1)&lt;/script&gt;");
        assert_eq!(markdown_to_pango("**hola**"), "<b>hola</b>");
        assert_eq!(markdown_to_pango("*cursiva*"), "<i>cursiva</i>");
        assert_eq!(markdown_to_pango("- uno\n- dos"), "• uno\n• dos");
        assert_eq!(
            markdown_to_pango("[web](https://example.com/a)"),
            "<a href=\"https://example.com/a\">web</a>"
        );
        let js = markdown_to_pango("[bad](javascript:alert(1))");
        assert!(!js.contains("<a"), "{}", js);
        assert_eq!(markdown_to_pango("![x](https://e/x.png) text"), " text");
    }
}
