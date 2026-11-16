use crate::config::Options;
use crate::io::write_at;
use crate::progress;
use crate::report::Cancelled;
use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use reqwest::header::HeaderMap;
use reqwest::Client;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

/// Tải bằng một kết nối (khi server không hỗ trợ range hoặc không biết dung lượng).
/// Không resume được: dừng giữa chừng thì lần sau tải lại từ đầu.
pub async fn download_single(
    client: &Client,
    url: &str,
    headers: &HeaderMap,
    part: &Path,
    total: Option<u64>,
    opts: &Options,
) -> Result<()> {
    let resp = client.get(url).headers(headers.clone()).send().await.context("gửi request")?;
    if !resp.status().is_success() {
        bail!("server trả về HTTP {}", resp.status());
    }
    let file = File::create(part).with_context(|| format!("không tạo được {}", part.display()))?;
    let mut stream = resp.bytes_stream();
    let mut off: u64 = 0;
    let mut last_print = Instant::now();
    let mut last_off: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("đọc dữ liệu")?;
        tokio::task::block_in_place(|| write_at(&file, &chunk, off))
            .map_err(|e| anyhow!("ghi file lỗi: {e}"))?;
        off += chunk.len() as u64;
        if let Some(r) = &opts.reporter {
            r.downloaded.fetch_max(off, std::sync::atomic::Ordering::Relaxed);
        }
        let dt = last_print.elapsed();
        if dt >= Duration::from_millis(500) {
            let speed = (off - last_off) as f64 / dt.as_secs_f64();
            if !opts.quiet {
                progress::render(off, total.unwrap_or(0), speed, 1);
            }
            if let Some(r) = &opts.reporter {
                r.update(off, total.unwrap_or(0), speed as u64, 1);
                if r.cancelled() {
                    return Err(Cancelled.into());
                }
            }
            last_print = Instant::now();
            last_off = off;
        }
    }
    if let Some(t) = total {
        if off != t {
            bail!("tải thiếu: {off}/{t} byte");
        }
    }
    Ok(())
}
