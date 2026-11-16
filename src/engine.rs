use crate::config::Options;
use crate::probe::{probe, sanitize, Probe};
use crate::progress;
use crate::report::Cancelled;
use crate::sched::Scheduler;
use crate::single::download_single;
use crate::state::{self, State};
use crate::worker::{worker_loop, Shared};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use std::fs::OpenOptions;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

/// HTTP/1.1 có chủ đích: mỗi request là một kết nối TCP riêng (như IDM), tránh việc
/// HTTP/2 gộp tất cả vào một kết nối và dính giới hạn tốc độ theo kết nối của server.
pub fn build_client(opts: &Options) -> Result<Client> {
    client_with(opts, false)
}

/// `native = true`: dùng TLS của hệ điều hành (như curl/aria2c trên macOS) thay cho rustls.
/// Một số server cắt kết nối với rustls nhưng chạy bình thường với TLS hệ thống.
fn client_with(opts: &Options, native: bool) -> Result<Client> {
    let mut b = Client::builder();
    b = if native { b.use_native_tls() } else { b.use_rustls_tls() };
    let mut b = b
        .http1_only()
        .user_agent(opts.user_agent.clone())
        .pool_max_idle_per_host(opts.max_conns)
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(10));
    if opts.force_ipv4 {
        b = b.local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    }
    b.build().context("không tạo được HTTP client")
}

/// Tải một URL về `dir`. Trả về đường dẫn file cuối cùng.
pub async fn download(url: &str, dir: &Path, name: Option<&str>, opts: &Options) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let headers = opts.header_map()?;
    let client = build_client(opts)?;
    let (client, p) = match probe(&client, url, &headers).await {
        Ok(p) => (client, p),
        Err(e) => {
            // Thử lại bằng TLS của hệ điều hành trước khi bỏ cuộc.
            let native = client_with(opts, true)?;
            match probe(&native, url, &headers).await {
                Ok(p) => (native, p),
                Err(_) => return Err(e.context("không kết nối được tới server")),
            }
        }
    };

    let fname = name.map(sanitize).unwrap_or_else(|| p.filename.clone());
    let out = unique_path(dir.join(&fname));
    if let Some(r) = &opts.reporter {
        if r.name().is_empty() {
            r.set_name(&fname);
        }
        r.set_path(out.clone());
        r.update(0, p.size.unwrap_or(0), 0, 0);
    }
    let mut part_name = out.as_os_str().to_owned();
    part_name.push(".part");
    let part = PathBuf::from(part_name);

    let started = Instant::now();
    match (p.size, p.ranges) {
        (Some(size), true) if size > 0 => {
            if !opts.quiet {
                eprintln!("{} | {} | range: có", fname, progress::fmt_bytes(size as f64));
            }
            run_ranged(&client, headers, url, &p, size, &part, opts).await?;
        }
        _ => {
            if !opts.quiet {
                eprintln!("{fname} | server không hỗ trợ range, dùng 1 kết nối");
            }
            download_single(&client, &p.final_url, &headers, &part, p.size, opts).await?;
            progress::finish_line();
        }
    }
    std::fs::rename(&part, &out).context("không đổi tên file .part")?;
    if let Some(r) = &opts.reporter {
        let n = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        r.update(n, n, 0, 0);
    }
    if !opts.quiet {
        let total = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) as f64;
        let secs = started.elapsed().as_secs_f64().max(0.001);
        eprintln!("Xong: {} ({} trong {:.1}s, TB {}/s)", out.display(), progress::fmt_bytes(total), secs, progress::fmt_bytes(total / secs));
    }
    Ok(out)
}

/// Nếu file đã tồn tại thì thêm " (1)", " (2)"... trước phần mở rộng.
fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    (1..)
        .map(|i| parent.join(format!("{stem} ({i}){ext}")))
        .find(|c| !c.exists())
        .unwrap()
}

/// Cắt các khoảng còn lại thành những đoạn ban đầu cho `pieces` worker đầu tiên.
fn initial_ranges(remaining: &[(u64, u64)], pieces: usize, min_split: u64) -> Vec<(u64, u64)> {
    let total: u64 = remaining.iter().map(|(a, b)| b - a).sum();
    let chunk = (total / pieces.max(1) as u64).max(min_split).max(1);
    let mut out = Vec::new();
    for &(a, b) in remaining {
        let mut s = a;
        while s < b {
            let e = (s + chunk).min(b);
            out.push((s, e));
            s = e;
        }
    }
    out
}

async fn run_ranged(
    client: &Client,
    headers: reqwest::header::HeaderMap,
    url: &str,
    p: &Probe,
    size: u64,
    part: &Path,
    opts: &Options,
) -> Result<()> {
    let spath = state::path_for(part);
    let resumable = state::load(&spath).filter(|st| {
        st.url == url
            && st.size == size
            && st.validator == p.validator
            && std::fs::metadata(part).map(|m| m.len() == size).unwrap_or(false)
    });
    let (remaining, fresh) = match resumable {
        Some(st) => {
            if !opts.quiet {
                eprintln!("Tiếp tục tải dở ({} khoảng còn lại)", st.remaining.len());
            }
            (st.remaining, false)
        }
        None => (vec![(0, size)], true),
    };

    let file = OpenOptions::new().read(true).write(true).create(true).open(part)?;
    if fresh {
        file.set_len(size)?;
    }
    let left: u64 = remaining.iter().map(|(a, b)| b - a).sum();
    let init = opts.init_conns.clamp(1, opts.max_conns.max(1));
    let sh = Arc::new(Shared {
        client: client.clone(),
        url: p.final_url.clone(),
        headers,
        file: Arc::new(file),
        sched: Scheduler::new(initial_ranges(&remaining, init, opts.min_split), opts.min_split),
        downloaded: AtomicU64::new(size - left),
        validator: p.validator.clone(),
        retries: opts.retries,
        max_req: opts.max_request,
        reporter: opts.reporter.clone(),
    });

    let save = |sh: &Shared| {
        let st = State {
            url: url.to_string(),
            size,
            validator: p.validator.clone(),
            remaining: sh.sched.snapshot(),
        };
        let _ = state::save(&spath, &st);
    };

    let result = pool(&sh, init, size, opts, &save).await;
    if !opts.quiet {
        progress::finish_line();
    }
    if let Err(e) = result {
        save(sh.as_ref());
        return Err(e);
    }
    if sh.downloaded.load(Ordering::Relaxed) != size || !sh.sched.is_done() {
        save(sh.as_ref());
        bail!("tải chưa đủ dữ liệu, chạy lại lệnh để tiếp tục");
    }
    let _ = std::fs::remove_file(&spath);
    Ok(())
}

/// Vòng điều khiển: chạy worker, mỗi 0.5s đo tốc độ, còn tăng thì thêm kết nối.
async fn pool(
    sh: &Arc<Shared>,
    init: usize,
    size: u64,
    opts: &Options,
    save: &(dyn Fn(&Shared) + Sync),
) -> Result<()> {
    let mut set: JoinSet<Result<()>> = JoinSet::new();
    for _ in 0..init {
        set.spawn(worker_loop(sh.clone()));
    }
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.tick().await; // bỏ lần tick tức thì đầu tiên
    let mut last_bytes = sh.downloaded.load(Ordering::Relaxed);
    let (mut best, mut ema, mut stalls, mut ticks) = (0f64, 0f64, 0u32, 0u32);
    let mut growing = true;

    loop {
        tokio::select! {
            joined = set.join_next() => match joined {
                None => return Ok(()),
                Some(Err(e)) => return Err(e.into()),
                Some(Ok(Err(e))) => return Err(e),
                // Đủ byte thì xong ngay, không chờ worker chậm (JoinSet bị drop sẽ hủy chúng).
                Some(Ok(Ok(()))) if sh.sched.is_done() => return Ok(()),
                Some(Ok(Ok(()))) => {}
            },
            _ = tick.tick() => {
                ticks += 1;
                let now = sh.downloaded.load(Ordering::Relaxed);
                let speed = (now - last_bytes) as f64 / 0.5;
                last_bytes = now;
                ema = if ema == 0.0 { speed } else { ema * 0.7 + speed * 0.3 };
                if growing && set.len() < opts.max_conns && !sh.sched.is_done() {
                    if speed > best * 1.05 {
                        best = speed;
                        stalls = 0;
                        let add = 2usize.min(opts.max_conns - set.len());
                        for _ in 0..add {
                            set.spawn(worker_loop(sh.clone()));
                        }
                    } else if best > 0.0 {
                        stalls += 1;
                        if stalls >= 3 {
                            growing = false;
                        }
                    }
                }
                if !opts.quiet {
                    progress::render(now, size, ema, set.len());
                }
                if let Some(r) = &opts.reporter {
                    r.update(now, size, ema as u64, set.len());
                    if speed > 0.0 && r.note().starts_with('⚠') {
                        r.set_note("");
                    }
                    if r.cancelled() {
                        return Err(Cancelled.into());
                    }
                }
                if ticks % 4 == 0 {
                    save(sh.as_ref());
                }
            }
        }
    }
}
