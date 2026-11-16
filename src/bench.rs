//! `fastdl bench`: đo tốc độ có số liệu, so engine của ta với curl / aria2c.
//! IDM chỉ chạy trên Windows: đo tay rồi thêm dòng vào CSV với tool = "idm".

use crate::config::Options;
use crate::engine;
use crate::report::Reporter;
use crate::ytdlp;
use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MIB: f64 = 1024.0 * 1024.0;

pub struct BenchOpts {
    pub runs: usize,
    pub conns: usize,
    pub with: Vec<String>,
    pub out: PathBuf,
    pub dir: PathBuf,
    /// Mỗi lần đo tối đa bấy nhiêu; quá thì dừng và tính tốc độ trên phần đã tải.
    pub timeout: Duration,
}

/// Kết quả một lần tải. Thời gian tính bằng giây từ lúc bắt đầu lệnh.
#[derive(Default, Clone)]
pub struct Sample {
    pub tool: String,
    pub url: String,
    pub run: usize,
    pub size: u64,
    pub ttfb: Option<f64>,
    pub t90: Option<f64>,
    pub total: f64,
    pub peak: Option<f64>,
    pub tail5: Option<f64>,
    pub conns: usize,
    /// Hết giờ: size là phần đã tải được, tốc độ vẫn hợp lệ.
    pub timed_out: bool,
    pub error: String,
}

impl Sample {
    pub fn avg(&self) -> f64 {
        if self.total > 0.0 { self.size as f64 / self.total } else { 0.0 }
    }
}

pub async fn run(list: &Path, o: &BenchOpts) -> Result<()> {
    let urls = read_urls(list)?;
    let mut tools = vec!["fastdl".to_string()];
    for t in o.with.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if !matches!(t, "curl" | "aria2c") {
            bail!("không hỗ trợ so với '{t}' (chỉ có curl, aria2c)");
        }
        if ytdlp::have(t, "--version").await {
            tools.push(t.to_string());
        } else {
            eprintln!("⚠ bỏ qua {t}: chưa cài (brew install {t})");
        }
    }
    eprintln!(
        "Đo {} URL × {} công cụ × {} lần, tối đa {}s mỗi lần. Mỗi lần tải xong file bị xóa.",
        urls.len(),
        tools.len(),
        o.runs,
        o.timeout.as_secs()
    );

    let mut all = Vec::new();
    for url in &urls {
        eprintln!("\n▶ {url}");
        for tool in &tools {
            for run in 1..=o.runs {
                let res = match tool.as_str() {
                    "fastdl" => run_fastdl(url, o).await,
                    "curl" => run_curl(url, o).await,
                    _ => run_aria2(url, o).await,
                };
                let mut s = res.unwrap_or_else(|e| Sample { error: format!("{e:#}"), ..Default::default() });
                s.tool = tool.clone();
                s.url = url.clone();
                s.run = run;
                eprintln!("  {}", describe(&s));
                all.push(s);
                crate::bench_report::write_csv(&o.out, &all)?; // ghi dần: Ctrl+C giữa chừng vẫn còn số liệu
                let _ = std::fs::remove_dir_all(work_dir(o));
            }
        }
    }
    crate::bench_report::write_csv(&o.out, &all)?;
    crate::bench_report::print_summary(&urls, &tools, &all);
    eprintln!("\nĐã ghi số liệu chi tiết vào {}", o.out.display());
    Ok(())
}

/// Mỗi dòng một URL; bỏ dòng trống và dòng bắt đầu bằng #; chữ sau URL là chú thích.
fn read_urls(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("không đọc được {}", path.display()))?;
    let urls: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect();
    if urls.is_empty() {
        bail!("{} không có URL nào", path.display());
    }
    Ok(urls)
}

/// Thư mục tải riêng của bench (chỉ thư mục này bị xóa sau mỗi lần đo).
fn work_dir(o: &BenchOpts) -> PathBuf {
    o.dir.join("fastdl-bench-tmp")
}

async fn run_fastdl(url: &str, o: &BenchOpts) -> Result<Sample> {
    let rep = Reporter::new();
    let mut opts = Options::default();
    opts.max_conns = o.conns.max(1);
    opts.init_conns = opts.init_conns.min(opts.max_conns);
    opts.quiet = true;
    opts.reporter = Some(rep.clone());

    let start = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = tokio::spawn(sample_loop(rep.clone(), start, stop.clone()));
    let res = tokio::time::timeout(o.timeout, engine::download(url, &work_dir(o), None, &opts)).await;
    let total = start.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let (trace, conns) = sampler.await.unwrap_or_default();

    let (size, timed_out) = match res {
        Ok(r) => (std::fs::metadata(r?).map(|m| m.len()).unwrap_or(0), false),
        Err(_) => (rep.downloaded.load(Ordering::Relaxed), true),
    };
    Ok(Sample { conns, timed_out, ..analyze(&trace, size, total) })
}

/// Lấy mẫu (thời điểm, số byte) mỗi 100ms.
async fn sample_loop(rep: Arc<Reporter>, start: Instant, stop: Arc<AtomicBool>) -> (Vec<(f64, u64)>, usize) {
    let mut trace = Vec::new();
    let mut conns = 0;
    while !stop.load(Ordering::Relaxed) {
        trace.push((start.elapsed().as_secs_f64(), rep.downloaded.load(Ordering::Relaxed)));
        conns = conns.max(rep.conns.load(Ordering::Relaxed));
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    (trace, conns)
}

/// Từ chuỗi mẫu tính: byte đầu tiên, tốc độ đỉnh (cửa sổ 1s), lúc đạt 90% đỉnh, thời gian 5% cuối.
pub fn analyze(trace: &[(f64, u64)], size: u64, total: f64) -> Sample {
    let ttfb = trace.iter().find(|(_, b)| *b > 0).map(|(t, _)| *t);
    let mut speeds = Vec::new(); // (thời điểm, tốc độ 1s gần nhất)
    let mut j = 0;
    for i in 0..trace.len() {
        while j + 1 < i && trace[i].0 - trace[j + 1].0 >= 1.0 {
            j += 1;
        }
        let dt = trace[i].0 - trace[j].0;
        if dt >= 0.9 {
            speeds.push((trace[i].0, trace[i].1.saturating_sub(trace[j].1) as f64 / dt));
        }
    }
    let peak = speeds.iter().map(|(_, v)| *v).fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
    let t90 = peak.and_then(|p| speeds.iter().find(|(_, v)| *v >= 0.9 * p).map(|(t, _)| *t));
    let mark = (size as f64 * 0.95) as u64;
    let tail5 = trace.iter().find(|(_, b)| *b >= mark && size > 0).map(|(t, _)| total - t);
    Sample { size, ttfb, t90, total, peak, tail5, ..Default::default() }
}

async fn run_curl(url: &str, o: &BenchOpts) -> Result<Sample> {
    let dir = work_dir(o);
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("curl.bin");
    let mut c = ytdlp::cmd("curl");
    c.args(["-L", "-s", "-S", "--fail", "-o"])
        .arg(&file)
        .args(["-w", "%{time_starttransfer} %{size_download}"])
        .arg(url)
        .kill_on_drop(true);
    let start = Instant::now();
    let res = tokio::time::timeout(o.timeout, c.output()).await;
    let total = start.elapsed().as_secs_f64();
    let Ok(out) = res else {
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        return Ok(Sample { size, total, conns: 1, timed_out: true, ..Default::default() });
    };
    let out = out.context("không chạy được curl")?;
    if !out.status.success() {
        bail!("curl lỗi: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut it = text.split_whitespace();
    let ttfb = it.next().and_then(|x| x.parse().ok());
    let size = it.next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    Ok(Sample { size, ttfb, total, conns: 1, ..Default::default() })
}

async fn run_aria2(url: &str, o: &BenchOpts) -> Result<Sample> {
    let dir = work_dir(o);
    std::fs::create_dir_all(&dir)?;
    let x = o.conns.clamp(1, 16).to_string(); // aria2c cho tối đa 16 kết nối/server
    let mut c = ytdlp::cmd("aria2c");
    c.args(["-x", &x, "-s", &x, "-k", "1M", "--file-allocation=none", "--allow-overwrite=true"])
        .args(["--console-log-level=error", "--summary-interval=0", "--download-result=hide", "-d"])
        .arg(&dir)
        .args(["-o", "aria.bin", url])
        .kill_on_drop(true);
    let start = Instant::now();
    let res = tokio::time::timeout(o.timeout, c.output()).await;
    let total = start.elapsed().as_secs_f64();
    let conns = o.conns.clamp(1, 16);
    let Ok(out) = res else {
        // aria2c ghi rải rác nên độ dài file chỉ là ước lượng phần đã tải.
        let size = std::fs::metadata(dir.join("aria.bin")).map(|m| m.len()).unwrap_or(0);
        return Ok(Sample { size, total, conns, timed_out: true, ..Default::default() });
    };
    let out = out.context("không chạy được aria2c")?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("aria2c lỗi: {}", msg.trim()));
    }
    let size = std::fs::metadata(dir.join("aria.bin")).map(|m| m.len()).unwrap_or(0);
    Ok(Sample { size, total, conns, ..Default::default() })
}

fn describe(s: &Sample) -> String {
    if !s.error.is_empty() {
        return format!("{} #{}: LỖI {}", s.tool, s.run, s.error);
    }
    let opt = |v: Option<f64>, f: &dyn Fn(f64) -> String| v.map(f).unwrap_or_else(|| "—".into());
    let timeout = if s.timed_out { " · ⏱ HẾT GIỜ (tính trên phần đã tải)" } else { "" };
    format!(
        "{} #{}: {:.1} MiB trong {:.1}s · TB {:.2} MiB/s · đỉnh {} · TTFB {} · 90% đỉnh lúc {} · 5% cuối {} · {} kết nối{}",
        s.tool,
        s.run,
        s.size as f64 / MIB,
        s.total,
        s.avg() / MIB,
        opt(s.peak, &|v| format!("{:.2} MiB/s", v / MIB)),
        opt(s.ttfb, &|v| format!("{v:.2}s")),
        opt(s.t90, &|v| format!("{v:.1}s")),
        opt(s.tail5, &|v| format!("{v:.1}s")),
        s.conns,
        timeout
    )
}
