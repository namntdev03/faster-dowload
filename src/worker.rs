use crate::io::write_at;
use crate::report::Reporter;
use crate::sched::{Scheduler, Seg};
use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, IF_RANGE, RANGE};
use reqwest::{Client, StatusCode};
use std::fs::File;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Dữ liệu dùng chung giữa các worker.
pub struct Shared {
    pub client: Client,
    pub url: String,
    pub headers: HeaderMap,
    pub file: Arc<File>,
    pub sched: Scheduler,
    pub downloaded: AtomicU64,
    pub validator: Option<String>,
    pub retries: u32,
    /// Giới hạn byte cho mỗi request (một đoạn có thể cần nhiều request liên tiếp).
    pub max_req: Option<u64>,
    /// Để hiện lỗi/thử lại lên GUI.
    pub reporter: Option<Arc<Reporter>>,
}

/// Mỗi worker liên tục xin đoạn mới (hoặc cắt đôi đoạn của worker khác) cho tới khi hết việc.
pub async fn worker_loop(sh: Arc<Shared>) -> Result<()> {
    loop {
        match sh.sched.next() {
            Some(seg) => {
                run_seg(&sh, &seg).await?;
                sh.sched.finish(&seg);
            }
            None if sh.sched.is_done() => return Ok(()),
            // Chưa có gì để lấy nhưng file chưa xong: chờ, có thể phải cứu một kết nối chậm.
            None => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

async fn run_seg(sh: &Shared, seg: &Arc<Seg>) -> Result<()> {
    let mut attempt: u32 = 0;
    loop {
        let before = seg.pos();
        match try_seg(sh, seg).await {
            Ok(()) if seg.remaining() == 0 => return Ok(()),
            Ok(()) if seg.pos() > before => {
                attempt = 0; // còn việc (đoạn dài hơn một request): gửi request kế tiếp
                continue;
            }
            other => {
                let e = match other {
                    Err(e) => e,
                    Ok(()) => anyhow!("không nhận thêm được dữ liệu"),
                };
                if seg.remaining() == 0 {
                    return Ok(());
                }
                attempt += 1;
                eprintln!("\n⚠ đoạn {:?}: {e:#} (thử lại {attempt}/{})", seg.range(), sh.retries);
                if let Some(r) = &sh.reporter {
                    r.set_note(&format!("⚠ lỗi, thử lại {attempt}/{}: {e:#}", sh.retries));
                }
                if attempt > sh.retries {
                    return Err(e.context(format!("đoạn {:?} lỗi sau {} lần thử", seg.range(), sh.retries)));
                }
                let wait = (250u64 << attempt.min(5)).min(8000);
                tokio::time::sleep(Duration::from_millis(wait)).await;
            }
        }
    }
}

/// Gửi một request cho phần còn lại của đoạn (tối đa `max_req` byte), bắt đầu từ vị trí đã ghi xong.
async fn try_seg(sh: &Shared, seg: &Arc<Seg>) -> Result<()> {
    let (pos, end) = seg.range();
    if pos >= end {
        return Ok(());
    }
    let req_end = match sh.max_req {
        Some(m) if m > 0 => end.min(pos.saturating_add(m)),
        _ => end,
    };
    let mut req = sh
        .client
        .get(&sh.url)
        .headers(sh.headers.clone())
        .header(RANGE, format!("bytes={}-{}", pos, req_end - 1));
    if let Some(v) = &sh.validator {
        req = req.header(IF_RANGE, v.as_str());
    }
    let resp = req.send().await.context("gửi request")?;
    if resp.status() != StatusCode::PARTIAL_CONTENT {
        bail!("server trả {} thay vì 206 (file có thể đã đổi)", resp.status());
    }

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("đọc dữ liệu")?;
        let mut data: &[u8] = &chunk;
        while !data.is_empty() {
            let n = seg.allow(data.len() as u64);
            if n == 0 {
                return Ok(()); // đoạn đã bị cắt ngắn hoặc đã xong
            }
            let at = seg.pos();
            let part = &data[..n as usize];
            tokio::task::block_in_place(|| write_at(&sh.file, part, at))
                .map_err(|e| anyhow!("ghi file lỗi: {e}"))?;
            let counted = seg.commit(n);
            let total = sh.downloaded.fetch_add(counted, Ordering::Relaxed) + counted;
            if let Some(r) = &sh.reporter {
                r.downloaded.fetch_max(total, Ordering::Relaxed); // số liệu tức thời cho GUI/bench
            }
            data = &data[n as usize..];
        }
    }
    if seg.pos() < req_end && seg.remaining() > 0 {
        bail!("kết nối đóng sớm, còn {} byte", seg.remaining());
    }
    Ok(())
}
