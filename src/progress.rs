use std::io::Write;

pub fn fmt_bytes(n: f64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}

fn fmt_eta(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "--:--".into();
    }
    let s = secs as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// Vẽ một dòng tiến độ trên stderr. `total == 0` nghĩa là chưa biết dung lượng.
pub fn render(done: u64, total: u64, speed: f64, conns: usize) {
    let mut err = std::io::stderr();
    if total == 0 {
        let _ = write!(err, "\r{} | {}/s | {} kn   ", fmt_bytes(done as f64), fmt_bytes(speed), conns);
    } else {
        let pct = done as f64 * 100.0 / total as f64;
        let filled = (pct / 100.0 * 24.0) as usize;
        let bar: String = (0..24).map(|i| if i < filled { '#' } else { '-' }).collect();
        let eta = if speed > 1.0 { (total - done.min(total)) as f64 / speed } else { f64::NAN };
        let _ = write!(
            err,
            "\r[{bar}] {pct:5.1}% {}/{} | {}/s | ETA {} | {} kn   ",
            fmt_bytes(done as f64),
            fmt_bytes(total as f64),
            fmt_bytes(speed),
            fmt_eta(eta),
            conns
        );
    }
    let _ = err.flush();
}

pub fn finish_line() {
    eprintln!();
}
