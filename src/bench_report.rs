//! Ghi CSV và in bảng tổng kết cho `fastdl bench`.

use crate::bench::Sample;
use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::path::Path;

const MIB: f64 = 1024.0 * 1024.0;

pub fn write_csv(path: &Path, rows: &[Sample]) -> Result<()> {
    let mut out = String::from("url,tool,run,size_bytes,total_s,avg_mibps,peak_mibps,ttfb_s,t90_s,tail5_s,conns,timed_out,error\n");
    let num = |v: Option<f64>| v.map(|x| format!("{x:.3}")).unwrap_or_default();
    for s in rows {
        let _ = writeln!(
            out,
            "{},{},{},{},{:.3},{:.3},{},{},{},{},{},{},{}",
            csv(&s.url),
            s.tool,
            s.run,
            s.size,
            s.total,
            s.avg() / MIB,
            num(s.peak.map(|v| v / MIB)),
            num(s.ttfb),
            num(s.t90),
            num(s.tail5),
            s.conns,
            s.timed_out,
            csv(&s.error)
        );
    }
    std::fs::write(path, out).with_context(|| format!("không ghi được {}", path.display()))
}

fn csv(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[v.len() / 2])
}

/// Bảng trung vị theo từng URL × công cụ, kèm so sánh fastdl với công cụ khác.
pub fn print_summary(urls: &[String], tools: &[String], rows: &[Sample]) {
    eprintln!("\n=== Tổng kết (trung vị; lần hết giờ tính theo phần đã tải) ===");
    for url in urls {
        eprintln!("\n{url}");
        eprintln!("  {:<8} {:>10} {:>10} {:>9} {:>9} {:>12}", "công cụ", "TB MiB/s", "thời gian", "TTFB", "5% cuối", "lỗi/hết giờ");
        let mut avgs = Vec::new();
        for tool in tools {
            let ok: Vec<&Sample> = rows.iter().filter(|s| &s.url == url && &s.tool == tool && s.error.is_empty()).collect();
            let fails = rows.iter().filter(|s| &s.url == url && &s.tool == tool && !s.error.is_empty()).count();
            let slow = ok.iter().filter(|s| s.timed_out).count();
            let avg = median(ok.iter().map(|s| s.avg()).collect());
            let total = median(ok.iter().map(|s| s.total).collect());
            let ttfb = median(ok.iter().filter_map(|s| s.ttfb).collect());
            let tail = median(ok.iter().filter_map(|s| s.tail5).collect());
            let f = |v: Option<f64>, d: usize, unit: &str| v.map(|x| format!("{x:.d$}{unit}")).unwrap_or_else(|| "—".into());
            eprintln!(
                "  {:<8} {:>10} {:>10} {:>9} {:>9} {:>12}",
                tool,
                f(avg.map(|v| v / MIB), 2, ""),
                f(total, 1, "s"),
                f(ttfb, 2, "s"),
                f(tail, 1, "s"),
                format!("{fails}/{slow}")
            );
            avgs.push((tool.as_str(), avg));
        }
        if let Some((_, Some(ours))) = avgs.iter().find(|(t, _)| *t == "fastdl") {
            for (tool, other) in avgs.iter().filter(|(t, _)| *t != "fastdl") {
                if let Some(other) = other.filter(|v| *v > 0.0) {
                    eprintln!("  → fastdl / {tool}: ×{:.2}", ours / other);
                }
            }
        }
    }
}
