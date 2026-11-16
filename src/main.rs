use anyhow::Result;
use clap::{Parser, Subcommand};
use fastdl::config::{parse_header, Options};
use fastdl::bench::{self, BenchOpts};
use fastdl::{engine, video};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "fastdl", version, about = "Tải file/video đa kết nối tốc độ cao")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Tải một file qua HTTP(S)
    Get {
        url: String,
        /// Tên file đầu ra (mặc định lấy từ server)
        #[arg(short, long)]
        output: Option<String>,
        /// Thư mục lưu
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        /// Số kết nối tối đa
        #[arg(short = 'c', long, default_value_t = 32)]
        connections: usize,
        /// Header thêm vào, ví dụ -H "Cookie: a=b" (lặp lại được)
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
        #[arg(long, default_value_t = 8)]
        retries: u32,
    },
    /// Tải video từ trang web (cần yt-dlp, ffmpeg)
    Video {
        url: String,
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
        /// Định dạng yt-dlp, ví dụ "bv*[height<=1080]+ba/b"
        #[arg(short, long, default_value = "bv*+ba/b")]
        format: String,
        #[arg(short = 'c', long, default_value_t = 32)]
        connections: usize,
        /// Số phân mảnh song song khi dùng yt-dlp cho HLS/DASH
        #[arg(short = 'N', long, default_value_t = 16)]
        fragments: usize,
        #[arg(short = 'H', long = "header")]
        headers: Vec<String>,
    },
    /// Đo tốc độ trên một danh sách URL, so với curl / aria2c, xuất CSV
    Bench {
        /// File danh sách URL, mỗi dòng một link (dòng bắt đầu bằng # là chú thích)
        list: PathBuf,
        /// Số lần đo mỗi URL cho mỗi công cụ
        #[arg(short = 'n', long, default_value_t = 3)]
        runs: usize,
        #[arg(short = 'c', long, default_value_t = 32)]
        connections: usize,
        /// Công cụ để so (phân cách bằng dấu phẩy; --with "" để chỉ đo fastdl)
        #[arg(long, value_delimiter = ',', default_value = "curl,aria2c")]
        with: Vec<String>,
        /// File CSV kết quả
        #[arg(short, long, default_value = "bench.csv")]
        out: PathBuf,
        /// Thư mục tạm để tải (mặc định thư mục tạm của hệ thống)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Giới hạn mỗi lần đo (giây); quá thì dừng và tính tốc độ trên phần đã tải
        #[arg(short, long, default_value_t = 90)]
        timeout: u64,
    },
}

fn build_opts(connections: usize, headers: &[String], retries: u32) -> Result<Options> {
    let mut o = Options::default();
    o.max_conns = connections.max(1);
    o.init_conns = o.init_conns.min(o.max_conns);
    o.retries = retries;
    for h in headers {
        o.headers.push(parse_header(h)?);
    }
    Ok(o)
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Lỗi: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Get { url, output, dir, connections, headers, retries } => {
            let opts = build_opts(connections, &headers, retries)?;
            engine::download(&url, &dir, output.as_deref(), &opts).await?;
        }
        Cmd::Video { url, dir, format, connections, fragments, headers } => {
            let opts = build_opts(connections, &headers, 8)?;
            video::run(&url, &dir, &format, fragments, &opts).await?;
        }
        Cmd::Bench { list, runs, connections, with, out, dir, timeout } => {
            let dir = dir.unwrap_or_else(std::env::temp_dir);
            let timeout = std::time::Duration::from_secs(timeout.max(5));
            let o = BenchOpts { runs: runs.max(1), conns: connections, with, out, dir, timeout };
            bench::run(&list, &o).await?;
        }
    }
    Ok(())
}
