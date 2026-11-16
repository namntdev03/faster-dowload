use crate::config::Options;
use crate::engine::download;
use crate::probe::sanitize;
use crate::report::Cancelled;
use crate::ytdlp::{self, Info};
use crate::ytrun;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Tải video: ưu tiên engine đa kết nối của ta trên URL trực tiếp, rồi ghép bằng ffmpeg.
/// Nếu là HLS/DASH hoặc tải trực tiếp thất bại thì giao cho yt-dlp (tải phân mảnh song song).
pub async fn run(url: &str, dir: &Path, format: &str, frags: usize, opts: &Options) -> Result<()> {
    if !ytdlp::have("yt-dlp", "--version").await {
        bail!("chưa tìm thấy yt-dlp. Mở Terminal chạy: brew install yt-dlp ffmpeg");
    }
    std::fs::create_dir_all(dir)?;
    if let Some(r) = &opts.reporter {
        r.set_note("Đang lấy thông tin video...");
    }
    let info = ytdlp::fetch_info(url, format).await?;
    eprintln!("Video: {}", info.title);
    if let Some(r) = &opts.reporter {
        r.set_name(&info.title);
        r.set_note("");
    }

    if info.native && (info.parts.len() == 1 || ytdlp::have("ffmpeg", "-version").await) {
        match native(&info, dir, opts).await {
            Ok(path) => {
                eprintln!("Xong: {}", path.display());
                return Ok(());
            }
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => return Err(e),
            Err(e) => eprintln!("Tải trực tiếp thất bại ({e:#}), chuyển sang yt-dlp..."),
        }
    } else if !info.native {
        eprintln!("Luồng HLS/DASH: dùng yt-dlp tải phân mảnh song song");
    }
    if let Some(r) = &opts.reporter {
        r.set_note("Đang tải bằng yt-dlp...");
    }
    fallback(url, dir, format, frags, opts).await
}

async fn native(info: &Info, dir: &Path, opts: &Options) -> Result<PathBuf> {
    let base = sanitize(&info.title);
    let mut files = Vec::new();
    for (i, part) in info.parts.iter().enumerate() {
        if opts.reporter.as_ref().map(|r| r.cancelled()).unwrap_or(false) {
            return Err(Cancelled.into());
        }
        let name = format!("{base}.f{i}.{}", part.ext);
        let target = dir.join(&name);
        if target.exists() {
            files.push(target); // luồng này đã tải xong ở lần trước
            continue;
        }
        let mut o = opts.clone();
        o.headers = part.headers.clone();
        o.max_request = part.chunk;
        o.force_ipv4 = true; // khớp với yt-dlp -4 lúc lấy link
        eprintln!("Luồng {}/{}", i + 1, info.parts.len());
        if let Some(r) = &opts.reporter {
            let what = if part.has_video { "hình" } else { "tiếng" };
            r.set_note(&format!("Luồng {}/{} ({what})", i + 1, info.parts.len()));
        }
        files.push(download(&part.url, dir, Some(&name), &o).await?);
    }
    if files.len() == 1 {
        let final_path = dir.join(format!("{base}.{}", info.parts[0].ext));
        std::fs::rename(&files[0], &final_path)?;
        finish(opts, &final_path);
        return Ok(final_path);
    }
    if let Some(r) = &opts.reporter {
        r.set_note("Đang ghép video + audio bằng ffmpeg...");
    }
    let container = match (info.parts[0].ext.as_str(), info.parts[1].ext.as_str()) {
        ("mp4", "m4a") => "mp4",
        ("webm", "webm") => "webm",
        _ => "mkv",
    };
    let out = dir.join(format!("{base}.{container}"));
    let status = ytdlp::cmd("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(&files[0])
        .arg("-i")
        .arg(&files[1])
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c", "copy"])
        .args(if container == "mp4" { vec!["-movflags", "+faststart"] } else { vec![] })
        .arg(&out)
        .status()
        .await
        .context("không chạy được ffmpeg (brew install ffmpeg)")?;
    if !status.success() {
        bail!("ffmpeg ghép file thất bại");
    }
    for f in &files {
        let _ = std::fs::remove_file(f);
    }
    finish(opts, &out);
    Ok(out)
}

/// Cho GUI biết file cuối cùng (để mở thư mục, hiện đúng dung lượng).
fn finish(opts: &Options, path: &Path) {
    if let Some(r) = &opts.reporter {
        let n = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        r.set_path(path.to_path_buf());
        r.update(n, n, 0, 0);
        r.set_note("");
    }
}

async fn fallback(url: &str, dir: &Path, format: &str, frags: usize, opts: &Options) -> Result<()> {
    // aria2c bị YouTube trả 403 (link gắn với cách yt-dlp gửi request), nên chỉ dùng cho trang khác.
    let is_youtube = ::url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .map(|h| h.contains("youtube.com") || h.contains("youtu.be"))
        .unwrap_or(false);
    let rep = opts.reporter.as_ref();
    let use_aria = !is_youtube && ytdlp::have("aria2c", "--version").await;
    if use_aria {
        match ytrun::run(url, dir, format, frags, true, rep).await {
            Ok(()) => return Ok(()),
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => return Err(e),
            Err(e) => eprintln!("aria2c thất bại ({e:#}), thử lại bằng bộ tải mặc định của yt-dlp..."),
        }
    }
    ytrun::run(url, dir, format, frags, false, rep).await
}
