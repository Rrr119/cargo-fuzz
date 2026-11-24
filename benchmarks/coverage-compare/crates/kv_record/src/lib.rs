//! A small INI/key-value parser used by the coverage comparison.

use std::hint::black_box;

pub fn check(data: &[u8]) -> u32 {
    black_box(score(data))
}

pub fn score(data: &[u8]) -> u32 {
    let Ok(text) = std::str::from_utf8(data) else {
        return 0;
    };

    let mut total = 1;
    let mut section = "";
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            total += section_score(section);
            continue;
        }

        for record in line.split(';') {
            let record = record.trim();
            if record.is_empty() {
                continue;
            }
            if let Some((key, value)) = record.split_once('=') {
                total += pair_score(section, key.trim(), value.trim());
            } else {
                total += 1;
            }
        }
    }
    total
}

fn section_score(section: &str) -> u32 {
    match section {
        "database" => 5,
        "server" => 4,
        "cache" => 4,
        "tls" => 5,
        "logging" => 3,
        _ if section.starts_with("db:") => 2,
        _ => 1,
    }
}

fn pair_score(section: &str, key: &str, value: &str) -> u32 {
    let generic = generic_key_score(key, value);
    let contextual = match section {
        "database" => database_score(key, value),
        "server" => server_score(key, value),
        "cache" => cache_score(key, value),
        "tls" => tls_score(key, value),
        _ => 0,
    };
    generic + contextual
}

fn generic_key_score(key: &str, value: &str) -> u32 {
    match key {
        "host" if !value.is_empty() => 2,
        "port" if value.bytes().all(|b| b.is_ascii_digit()) => 3,
        "user" if !value.is_empty() => 2,
        "password" if value.len() >= 6 => 4,
        "name" if !value.is_empty() => 1,
        "timeout" if value.parse::<u32>().is_ok_and(|n| n > 0) => 3,
        "retries" if value.parse::<u8>().is_ok_and(|n| n <= 10) => 3,
        "debug" | "enabled" if value == "true" || value == "false" => 2,
        _ => 0,
    }
}

fn database_score(key: &str, value: &str) -> u32 {
    match key {
        "driver" if value == "postgres" => 6,
        "driver" if value == "mysql" => 5,
        "host" if value == "localhost" => 3,
        "port" if value == "5432" => 5,
        "port" if value == "3306" => 4,
        "name" if value == "app" => 3,
        "sslmode" if value == "require" => 6,
        "pool" if value.parse::<u8>().is_ok_and(|n| (1..=64).contains(&n)) => 4,
        _ => 1,
    }
}

fn server_score(key: &str, value: &str) -> u32 {
    match key {
        "bind" if value.contains(':') => 4,
        "port" if value == "8080" || value == "443" => 5,
        "workers" if value.parse::<u8>().is_ok_and(|n| n >= 1) => 3,
        "debug" if value == "false" => 2,
        "shutdown_timeout" if value.parse::<u32>().is_ok() => 4,
        _ => 1,
    }
}

fn cache_score(key: &str, value: &str) -> u32 {
    match key {
        "driver" if value == "redis" => 6,
        "ttl" if value.parse::<u32>().is_ok_and(|n| n >= 30) => 4,
        "max_entries" if value.parse::<u32>().is_ok() => 3,
        "enabled" if value == "true" => 2,
        "namespace" if value.starts_with("app:") => 5,
        _ => 1,
    }
}

fn tls_score(key: &str, value: &str) -> u32 {
    match key {
        "enabled" if value == "true" => 4,
        "cert" if value.ends_with(".pem") || value.ends_with(".crt") => 5,
        "key" if value.ends_with(".key") => 5,
        "min_version" if value == "1.3" => 7,
        "cipher_suites" if value.contains("TLS_AES_256_GCM") => 6,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_record_scores() {
        let input = b"[database]\ndriver=postgres;port=5432;sslmode=require;\n";
        assert!(score(input) > 20);
        assert_eq!(score(&[0xff]), 0);
    }
}
