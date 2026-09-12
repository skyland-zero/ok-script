//! Brand marks, About-page link resolution and HTML-to-text conversion.
//!
//! The web frontend draws its brand marks as inline SVGs inside `App.tsx`
//! (`GithubMark`, `DiscordMark`, `ChatMark`, `HeartMark`, `ShareMark`); the same
//! paths are vendored under `assets/icons/ok` so the native UI shows identical
//! glyphs. The `about` text is rich HTML in the browser, which GPUI cannot
//! render, so it is converted to plain text with links preserved.

use gpui::{px, AnyElement, Hsla, IntoElement, Styled};
use serde_json::Value;

use crate::icons::OkIcon;

/// Order used by the web frontend's `aboutLinkOrder`.
pub const ABOUT_LINK_ORDER: [(&str, &str, OkIcon); 8] = [
    ("github", "GitHub", OkIcon::Github),
    ("download", "Download", OkIcon::ArrowDownload),
    ("discord", "Discord", OkIcon::Discord),
    ("qq_group", "QQ群", OkIcon::Chat),
    ("qq_channel", "QQ频道", OkIcon::Chat),
    ("faq", "FAQ", OkIcon::QuestionCircle),
    ("share", "Share", OkIcon::ShareMark),
    ("sponsor", "Sponsor", OkIcon::HeartMark),
];

/// Brand glyph for a link key, sized and coloured like the web buttons.
pub fn brand_mark(icon: OkIcon, size: f32, color: Hsla) -> AnyElement {
    icon.icon().size(px(size)).text_color(color).into_any_element()
}

/// Mirror of the web frontend's `localizedLink`/`configuredAboutLink`.
pub fn link_url(links: &Value, key: &str, locale: &str) -> Option<String> {
    let value = links.get(key)?;
    resolve_link(value, locale)
}

fn resolve_link(value: &Value, locale: &str) -> Option<String> {
    if let Some(text) = value.as_str() {
        if text.is_empty() {
            return None;
        }
        return Some(text.to_owned());
    }
    let object = value.as_object()?;
    let base = locale.split('_').next().unwrap_or(locale);
    for candidate in [locale, base, "default", "en_US", "en"] {
        if let Some(text) = object.get(candidate).and_then(Value::as_str) {
            if !text.is_empty() {
                return Some(text.to_owned());
            }
        }
    }
    object.values().find_map(|value| value.as_str().map(str::to_owned))
}

/// Convert the backend's `about` HTML into readable text.
///
/// Block elements become line breaks, `<a href>` keeps its target in
/// parentheses, `<li>` becomes a bullet, and remaining tags are dropped — the
/// equivalent of the web frontend's whitelist sanitiser plus its renderer.
pub fn html_to_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        text.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start + 1..start + end];
        let name = tag
            .trim_start_matches('/')
            .split([' ', '\t', '\n', '/'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !tag.starts_with('/') {
            match name.as_str() {
                "br" => text.push('\n'),
                "p" | "div" | "blockquote" | "pre" | "h1" | "h2" | "h3" | "h4" | "hr" | "ul"
                | "ol" => text.push('\n'),
                "li" => {
                    text.push('\n');
                    text.push_str("• ");
                }
                "a" => {
                    if let Some(href) = attribute(tag, "href") {
                        if href.starts_with("http://") || href.starts_with("https://") {
                            text.push('\u{1}');
                            text.push_str(&href);
                            text.push('\u{2}');
                        }
                    }
                }
                _ => {}
            }
        } else if matches!(name.as_str(), "p" | "div" | "h1" | "h2" | "h3" | "h4" | "li") {
            text.push('\n');
        }
        rest = &rest[start + end + 1..];
    }
    text.push_str(rest);

    // Re-insert link targets after their label.
    let mut with_links = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\u{1}' => {
                let mut url = String::new();
                for next in chars.by_ref() {
                    if next == '\u{2}' {
                        break;
                    }
                    url.push(next);
                }
                with_links.push_str(&format!(" ({url})"));
            }
            other => with_links.push(other),
        }
    }

    let decoded = decode_entities(&with_links);
    let mut lines: Vec<&str> = decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines.dedup();
    lines.join("\n")
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(index) = tag.find(&needle) {
            let rest = &tag[index + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_owned());
            }
        }
    }
    None
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}
