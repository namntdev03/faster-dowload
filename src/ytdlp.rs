use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::ffi::OsString;
use std::path::PathBuf;
use tokio::process::Command;

/// Một luồng (video hoặc audio) mà yt-dlp đã giải mã ra URL trực tiếp.
#[derive(Debug, Clone)]
pub struct Part {
    pub url: String,
    pub ext: String,
    pub has_video: bool,
    pub headers: Vec<(String, String)>,
    /// Kích thước mỗi request mà yt-dlp khuyến nghị (YouTube).
    pub chunk: Option<u64>,
}

#[derive(Debug)]
pub struct Info {
    pub title: String,
    pub id: String,
    pub parts: Vec<Part>,
    /// true nếu mọi luồng là HTTP(S) thường (không phải HLS/DASH) nên engine của ta tải được.
    pub native: bool,
}

/// Các thư mục cần tìm công cụ: PATH hiện tại + nơi Homebrew/pip hay cài.
/// App GUI trên macOS không kế thừa PATH của shell nên phải tự thêm.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for d in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin", "/usr/bin"] {
        dirs.push(PathBuf::from(d));
    }
    if let Some(h) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(h).join(".local/bin"));
    }
    dirs
}

pub fn locate(tool: &str) -> PathBuf {
    for d in search_dirs() {
        for name in [tool.to_string(), format!("{tool}.exe")] {
            let c = d.join(&name);
            if c.is_file() {
                return c;
            }
        }
    }
    PathBuf::from(tool)
}

/// Tạo lệnh chạy công cụ ngoài, với PATH đã được bổ sung (để yt-dlp tìm thấy ffmpeg).
pub fn cmd(tool: &str) -> Command {
    let mut c = Command::new(locate(tool));
    if let Ok(p) = std::env::join_paths(search_dirs()) {
        c.env("PATH", OsString::from(p));
    }
    c
}

pub async fn have(tool: &str, arg: &str) -> bool {
    cmd(tool)
        .arg(arg)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Header yt-dlp trả về mà ta không được truyền thẳng (engine tự quản lý).
fn skip_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "accept-encoding" | "range" | "host" | "content-length" | "connection"
    )
}

fn parse_part(f: &Value) -> Option<Part> {
    let proto = f["protocol"].as_str().unwrap_or("");
    if !matches!(proto, "https" | "http") {
        return None;
    }
    let url = f["url"].as_str()?.to_string();
    let mut headers: Vec<(String, String)> = f["http_headers"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(k, _)| !skip_header(k))
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    if let Some(c) = f["cookies"].as_str() {
        if !c.is_empty() {
            headers.push(("Cookie".into(), c.to_string()));
        }
    }
    Some(Part {
        url,
        ext: f["ext"].as_str().unwrap_or("bin").to_string(),
        has_video: f["vcodec"].as_str().map(|v| v != "none").unwrap_or(true),
        headers,
        chunk: f["downloader_options"]["http_chunk_size"].as_u64(),
    })
}

/// Hỏi yt-dlp thông tin video và URL trực tiếp của các luồng đã chọn theo `format`.
pub async fn fetch_info(url: &str, format: &str) -> Result<Info> {
    let out = cmd("yt-dlp")
        .args(["--dump-single-json", "--no-warnings", "--no-playlist", "-4", "-f", format, url])
        .output()
        .await
        .context("không chạy được yt-dlp")?;
    if !out.status.success() {
        bail!("yt-dlp lỗi: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let v: Value = serde_json::from_slice(&out.stdout).context("đọc JSON của yt-dlp")?;
    let formats: Vec<&Value> = match v["requested_formats"].as_array() {
        Some(a) if !a.is_empty() => a.iter().collect(),
        _ => vec![&v],
    };
    let mut parts: Vec<Part> = formats.iter().filter_map(|f| parse_part(f)).collect();
    let native = parts.len() == formats.len() && !parts.is_empty();
    parts.sort_by_key(|p| !p.has_video); // video trước, audio sau
    Ok(Info {
        title: v["title"].as_str().unwrap_or("video").to_string(),
        id: v["id"].as_str().unwrap_or("").to_string(),
        parts,
        native,
    })
}
