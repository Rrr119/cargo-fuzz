//! A small HTTP-router-shaped parser used by the coverage comparison.

use std::hint::black_box;

pub fn check(data: &[u8]) -> u32 {
    black_box(score(data))
}

pub fn score(data: &[u8]) -> u32 {
    let Ok(text) = std::str::from_utf8(data) else {
        return 0;
    };

    let mut fields = text.split_whitespace();
    let method = fields.next().unwrap_or("");
    let target = fields.next().unwrap_or("");
    let version = fields.next().unwrap_or("");

    let mut score = 1;
    score += method_score(method);
    score += version_score(version);
    score += target_score(target);
    score
}

fn method_score(method: &str) -> u32 {
    match method {
        "GET" => 2,
        "POST" => 3,
        "PUT" => 4,
        "DELETE" => 5,
        "PATCH" => 4,
        "HEAD" => 2,
        "OPTIONS" => 2,
        _ if method.starts_with('G') => 1,
        _ if method.starts_with('P') => 1,
        _ => 0,
    }
}

fn version_score(version: &str) -> u32 {
    match version {
        "HTTP/1.1" => 5,
        "HTTP/1.0" => 3,
        "HTTP/2" => 4,
        _ if version.starts_with("HTTP/") => 1,
        _ => 0,
    }
}

fn target_score(target: &str) -> u32 {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut score = query_score(query);

    match segments.first().copied() {
        Some("health") => score += health_score(&segments),
        Some("users") => score += users_score(&segments),
        Some("orders") => score += orders_score(&segments),
        Some("items") => score += items_score(&segments),
        Some("admin") => score += admin_score(&segments),
        Some(other) if other.starts_with('v') => score += 2,
        Some(_) => score += 1,
        None => {}
    }

    score
}

fn health_score(segments: &[&str]) -> u32 {
    let mut score = 4;
    if segments.get(1) == Some(&"ready") {
        score += 3;
    }
    if segments.get(1) == Some(&"live") {
        score += 2;
    }
    score
}

fn users_score(segments: &[&str]) -> u32 {
    let mut score = 5;
    match segments.get(1).copied() {
        Some("active") => score += 5,
        Some("archive") => score += 4,
        Some("me") => score += 3,
        Some(id) if id.bytes().all(|byte| byte.is_ascii_digit()) => {
            score += 6;
            if segments.get(2) == Some(&"details") {
                score += 4;
            }
            if segments.get(2) == Some(&"reports") {
                score += 5;
            }
        }
        Some(_) => score += 1,
        None => {}
    }
    score
}

fn orders_score(segments: &[&str]) -> u32 {
    let mut score = 5;
    if let Some(id) = segments.get(1) {
        if id.bytes().all(|byte| byte.is_ascii_digit()) {
            score += 5;
        }
    }
    if segments.get(2) == Some(&"reports") {
        score += 6;
    }
    if segments.get(2) == Some(&"items") {
        score += 4;
    }
    score
}

fn items_score(segments: &[&str]) -> u32 {
    let mut score = 4;
    match segments.get(1).copied() {
        Some("archive") => score += 4,
        Some("search") => score += 3,
        Some(sku) if sku.starts_with("SKU-") => score += 6,
        Some(_) => score += 1,
        None => {}
    }
    score
}

fn admin_score(segments: &[&str]) -> u32 {
    let mut score = 6;
    if segments.get(1) == Some(&"reports") {
        score += 7;
    }
    if segments.get(1) == Some(&"settings") {
        score += 5;
    }
    if segments.get(2) == Some(&"export") {
        score += 4;
    }
    score
}

fn query_score(query: &str) -> u32 {
    if query.is_empty() {
        return 0;
    }

    let mut score = 1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "page" if value.bytes().all(|byte| byte.is_ascii_digit()) => score += 3,
            "limit" if value.bytes().all(|byte| byte.is_ascii_digit()) => score += 3,
            "sort" => score += 2,
            "format" if value == "json" => score += 4,
            "format" if value == "csv" => score += 3,
            "q" if !value.is_empty() => score += 2,
            _ => {}
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_route_scores_highly() {
        assert!(score(b"GET /users/42/details?format=json HTTP/1.1") > 20);
        assert_eq!(score(b"not utf8 \xff"), 0);
    }
}
