//! A tiny HTML-like tag/attribute scanner for coverage comparisons.

use std::hint::black_box;

pub fn check(data: &[u8]) -> u32 {
    black_box(score(data))
}

pub fn score(data: &[u8]) -> u32 {
    let Ok(text) = std::str::from_utf8(data) else {
        return 0;
    };

    let mut total = 1;
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        total += 1;
        let after_lt = &rest[start + 1..];
        let Some(end) = after_lt.find('>') else {
            total += 1;
            break;
        };
        let token = &after_lt[..end];
        total += tag_score(token);
        rest = &after_lt[end + 1..];
    }
    total
}

fn tag_score(token: &str) -> u32 {
    let token = token.trim();
    if token.is_empty() {
        return 0;
    }

    let closing = token.starts_with('/');
    let self_closing = token.ends_with('/');
    let body = token.trim_start_matches('/').trim_end_matches('/').trim();
    let mut parts = body.split_whitespace();
    let name = parts.next().unwrap_or("");
    let attribute_text = body[name.len()..].trim();

    let mut score = match name {
        "html" => 5,
        "head" => 4,
        "title" => 4,
        "body" => 5,
        "header" => 4,
        "main" => 4,
        "section" => 4,
        "article" => 4,
        "div" => 2,
        "span" => 2,
        "p" => 2,
        "a" => 3,
        "img" => 4,
        "table" => 5,
        "thead" | "tbody" | "tr" => 3,
        "td" | "th" => 3,
        "ul" | "ol" | "li" => 3,
        _ if name.contains('-') => 2,
        _ => 1,
    };

    if closing {
        score += 1;
    }
    if self_closing {
        score += 2;
    }
    score += attributes_score(name, attribute_text);
    score
}

fn attributes_score(tag: &str, attributes: &str) -> u32 {
    let mut score = 0;
    for attribute in attributes.split_whitespace() {
        let (name, raw_value) = attribute.split_once('=').unwrap_or((attribute, ""));
        let value = raw_value.trim_matches('"').trim_matches('\'');
        score += match name {
            "id" if value == "top" || value == "main" => 4,
            "id" if !value.is_empty() => 1,
            "class" if value.split_whitespace().any(|class| class == "container") => 5,
            "class" if value.split_whitespace().any(|class| class == "active") => 4,
            "class" if !value.is_empty() => 1,
            "href" if value.starts_with("https://") => 5,
            "href" if value.starts_with('/') => 3,
            "href" if value.starts_with('#') => 2,
            "src" if value.ends_with(".png") => 4,
            "src" if value.starts_with("data:") => 6,
            "alt" if !value.is_empty() => 2,
            "type" if value == "text/html" => 3,
            _ => 0,
        };
    }

    match tag {
        "a" if attributes.contains("href=") => score += 2,
        "img" if attributes.contains("alt=") && attributes.contains("src=") => score += 3,
        _ => {}
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_markup_scores() {
        let input = include_str!("../schema.toml");
        assert!(score(input.as_bytes()) > 10);
        assert_eq!(score(&[0xff]), 0);
    }
}
