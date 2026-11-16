//! Cầu nối giữa engine và giao diện: GUI đọc tiến độ và gửi lệnh dừng qua đây.

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Lỗi đặc biệt: người dùng chủ động dừng. Trạng thái resume vẫn được lưu.
#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "đã dừng")
    }
}

impl std::error::Error for Cancelled {}

#[derive(Debug, Default)]
pub struct Reporter {
    pub downloaded: AtomicU64,
    pub total: AtomicU64,
    /// byte/giây (đã làm mượt)
    pub speed: AtomicU64,
    pub conns: AtomicUsize,
    pub cancel: AtomicBool,
    /// Tải nhiều luồng nối tiếp (video): byte của các luồng đã xong và tổng mọi luồng.
    /// grand_total = 0 nghĩa là không gộp, hiển thị thẳng downloaded/total.
    pub offset: AtomicU64,
    pub grand_total: AtomicU64,
    name: Mutex<String>,
    /// File tạm cần dọn nếu người dùng xóa lượt tải (luồng hình/tiếng chưa ghép).
    temps: Mutex<Vec<PathBuf>>,
    path: Mutex<Option<PathBuf>>,
    note: Mutex<String>,
}

impl Reporter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn update(&self, done: u64, total: u64, speed: u64, conns: usize) {
        self.downloaded.store(done, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.speed.store(speed, Ordering::Relaxed);
        self.conns.store(conns, Ordering::Relaxed);
    }

    /// (đã tải, tổng) để hiển thị: gộp mọi luồng nếu có grand_total.
    pub fn shown(&self) -> (u64, u64) {
        let g = self.grand_total.load(Ordering::Relaxed);
        let d = self.downloaded.load(Ordering::Relaxed);
        if g > 0 {
            ((self.offset.load(Ordering::Relaxed) + d).min(g), g)
        } else {
            (d, self.total.load(Ordering::Relaxed))
        }
    }

    /// Bỏ chế độ gộp (khi đã có file cuối).
    pub fn clear_grand(&self) {
        self.offset.store(0, Ordering::Relaxed);
        self.grand_total.store(0, Ordering::Relaxed);
    }

    pub fn add_temp(&self, p: PathBuf) {
        let mut t = self.temps.lock().unwrap();
        if !t.contains(&p) {
            t.push(p);
        }
    }

    pub fn temps(&self) -> Vec<PathBuf> {
        self.temps.lock().unwrap().clone()
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Chuẩn bị cho lần chạy (lại) mới.
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::Relaxed);
        self.speed.store(0, Ordering::Relaxed);
        self.conns.store(0, Ordering::Relaxed);
        self.set_note("");
    }

    pub fn name(&self) -> String {
        self.name.lock().unwrap().clone()
    }

    pub fn set_name(&self, s: &str) {
        *self.name.lock().unwrap() = s.to_string();
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.path.lock().unwrap().clone()
    }

    pub fn set_path(&self, p: PathBuf) {
        *self.path.lock().unwrap() = Some(p);
    }

    pub fn note(&self) -> String {
        self.note.lock().unwrap().clone()
    }

    pub fn set_note(&self, s: &str) {
        *self.note.lock().unwrap() = s.to_string();
    }
}
