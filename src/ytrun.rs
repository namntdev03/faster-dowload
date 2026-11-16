//! Chạy yt-dlp như tiến trình con: đọc tiến độ cho GUI, dừng được khi bấm Dừng,
//! và trả về lỗi thật của yt-dlp thay vì một câu chung chung.

use crate::report::{Cancelled, Reporter};
use crate::ytdlp;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

const PROGRESS: &str = "FDL|";
const FILEPATH: &str = "FDL_PATH|";

pub async fn run(url: &str, dir: &Path, format: &str, frags: usize, aria: bool, rep: Option<&Arc<Reporter>>) -> Result<()> {
    let mut c = ytdlp::cmd("yt-dlp");
    c.args(["-f", format, "-N", &frags.to_string(), "--no-playlist"]);
    if aria {
        c.args(["--downloader", "aria2c", "--downloader-args", "aria2c:-x16 -s16 -k1M"]);
    }
    c.args(["--merge-output-format", "mp4", "-P"]).arg(dir);

    // CLI: để yt-dlp in thẳng ra terminal như bình thường.
    let Some(rep) = rep else {
        let status = c.arg(url).status().await.context("không chạy được yt-dlp")?;
        if !status.success() {
            bail!("yt-dlp thất bại");
        }
        return Ok(());
    };

    c.args([
        "--newline",
        "--progress",
        "--progress-template",
        "download:FDL|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s",
        "--print",
        "after_move:FDL_PATH|%(filepath)s",
    ]);
    rep.clear_grand();
    let mut child = c
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("không chạy được yt-dlp")?;

    let path = Arc::new(Mutex::new(None::<PathBuf>));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let out = tokio::spawn(read_lines(child.stdout.take().unwrap(), rep.clone(), path.clone(), errors.clone()));
    let err = tokio::spawn(read_lines(child.stderr.take().unwrap(), rep.clone(), path.clone(), errors.clone()));

    let status = loop {
        tokio::select! {
            st = child.wait() => break st.context("yt-dlp dừng bất thường")?,
            _ = tokio::time::sleep(Duration::from_millis(300)) => {
                if rep.cancelled() {
                    let _ = child.kill().await;
                    return Err(Cancelled.into());
                }
            }
        }
    };
    let _ = out.await;
    let _ = err.await;
    rep.speed.store(0, std::sync::atomic::Ordering::Relaxed);

    if !status.success() {
        let msg = errors.lock().unwrap().join("\n");
        bail!("yt-dlp thất bại{}", if msg.is_empty() { String::new() } else { format!(": {msg}") });
    }
    if let Some(p) = path.lock().unwrap().clone() {
        let n = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        rep.set_path(p);
        rep.update(n, n, 0, 0);
    }
    rep.set_note("");
    Ok(())
}

/// Đọc từng dòng: dòng tiến độ thì cập nhật reporter, dòng đường dẫn thì ghi lại,
/// dòng ERROR/WARNING giữ lại vài dòng cuối để báo lỗi.
async fn read_lines<R: AsyncRead + Unpin>(
    src: R,
    rep: Arc<Reporter>,
    path: Arc<Mutex<Option<PathBuf>>>,
    errors: Arc<Mutex<Vec<String>>>,
) {
    let mut lines = BufReader::new(src).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if let Some(p) = line.strip_prefix(FILEPATH) {
            *path.lock().unwrap() = Some(PathBuf::from(p));
        } else if let Some(rest) = line.strip_prefix(PROGRESS) {
            let v: Vec<f64> = rest.split('|').map(|x| x.parse().unwrap_or(0.0)).collect();
            if v.len() == 4 {
                let total = if v[1] > 0.0 { v[1] } else { v[2] };
                rep.update(v[0] as u64, total as u64, v[3] as u64, 0);
            }
        } else if line.starts_with("ERROR") || line.starts_with("WARNING") {
            let mut e = errors.lock().unwrap();
            if line.starts_with("ERROR") {
                e.retain(|l| l.starts_with("ERROR"));
            }
            e.push(line.to_string());
            let n = e.len();
            if n > 3 {
                e.drain(..n - 3);
            }
        }
    }
}
