use anyhow::{bail, Result};
use reqwest::header::{
    HeaderMap, ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, ETAG,
    LAST_MODIFIED, RANGE,
};
use reqwest::{Client, StatusCode};

/// Thông tin về file trên server.
#[derive(Debug, Clone)]
pub struct Probe {
    /// URL cuối cùng sau khi theo redirect.
    pub final_url: String,
    pub size: Option<u64>,
    /// Server có hỗ trợ tải theo khoảng byte (206) hay không.
    pub ranges: bool,
    pub filename: String,
    /// ETag (không phải weak) hoặc Last-Modified, dùng cho If-Range và kiểm tra resume.
    pub validator: Option<String>,
}

/// Thăm dò lần lượt: Range 0-0, Range 0-1MiB, rồi không Range. Một số CDN từ chối range quá nhỏ.
pub async fn probe(client: &Client, url: &str, headers: &HeaderMap) -> Result<Probe> {
    let mut errs = Vec::new();
    for range in [Some("bytes=0-0"), Some("bytes=0-1048575"), None] {
        match probe_once(client, url, headers, range).await {
            Ok(p) => return Ok(p),
            Err(e) => errs.push(format!("{}: {e:#}", range.unwrap_or("không Range"))),
        }
    }
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default();
    bail!("server từ chối mọi cách thăm dò (host {host}) [{}]", errs.join(" | "))
}

async fn probe_once(
    client: &Client,
    url: &str,
    headers: &HeaderMap,
    range: Option<&str>,
) -> Result<Probe> {
    let mut req = client.get(url).headers(headers.clone());
    if let Some(r) = range {
        req = req.header(RANGE, r);
    }
    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        bail!("HTTP {status}");
    }
    let h = resp.headers();
    let parse_u64 = |s: &str| s.trim().parse::<u64>().ok();
    let accepts = h
        .get(ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("bytes"))
        .unwrap_or(false);

    let (size, ranges) = if status == StatusCode::PARTIAL_CONTENT {
        let total = h
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .and_then(parse_u64);
        (total, total.is_some())
    } else {
        let len = h.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(parse_u64);
        (len, accepts && len.is_some())
    };

    let validator = h
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.starts_with("W/"))
        .or_else(|| h.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()))
        .map(|s| s.to_string());

    let final_url = resp.url().to_string();
    let filename = h
        .get(CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(from_disposition)
        .map(|n| sanitize(&n))
        .unwrap_or_else(|| from_url(&final_url));

    Ok(Probe { final_url, size, ranges, filename, validator })
}

fn from_disposition(v: &str) -> Option<String> {
    for part in v.split(';') {
        let p = part.trim();
        if p.to_ascii_lowercase().starts_with("filename*=") {
            let val = p[10..].trim().trim_matches('"');
            let enc = val.splitn(3, '\'').nth(2).unwrap_or(val);
            return Some(percent_decode(enc));
        }
    }
    for part in v.split(';') {
        let p = part.trim();
        if p.to_ascii_lowercase().starts_with("filename=") {
            return Some(p[9..].trim().trim_matches('"').to_string());
        }
    }
    None
}

fn from_url(u: &str) -> String {
    let name = url::Url::parse(u)
        .ok()
        .and_then(|p| p.path_segments().and_then(|s| s.last().map(|x| x.to_string())))
        .map(|s| percent_decode(&s))
        .unwrap_or_default();
    sanitize(&name)
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Làm sạch tên file cho an toàn trên mọi hệ điều hành.
pub fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s: String = s.trim().trim_start_matches('.').chars().take(200).collect();
    if s.is_empty() {
        "download.bin".to_string()
    } else {
        s
    }
}
