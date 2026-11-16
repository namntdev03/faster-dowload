//! Bộ chia việc: mỗi đoạn (Seg) là một khoảng byte [pos, end). Worker xong việc sẽ
//! "ăn cắp" nửa sau của đoạn còn nhiều nhất (dynamic segmentation, giống IDM nhưng liên tục).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Mảnh nhỏ nhất được cắt ra ở giai đoạn cuối (endgame).
const TAIL_MIN: u64 = 64 * 1024;
/// Chỉ can thiệp khi đoạn chậm nhất còn cần hơn bấy nhiêu giây.
const ENDGAME_ETA: f64 = 0.6;

struct Span {
    pos: u64,
    end: u64,
    /// Vị trí lúc bắt đầu và thời điểm bắt đầu, để đo tốc độ của đoạn.
    base: u64,
    start: Instant,
}

/// Một đoạn đang tải. `pos` là vị trí đã GHI XONG (committed).
pub struct Seg {
    inner: Mutex<Span>,
}

impl Seg {
    pub fn new(start: u64, end: u64) -> Arc<Seg> {
        Arc::new(Seg { inner: Mutex::new(Span { pos: start, end, base: start, start: Instant::now() }) })
    }

    pub fn range(&self) -> (u64, u64) {
        let g = self.inner.lock().unwrap();
        (g.pos, g.end)
    }

    pub fn pos(&self) -> u64 {
        self.inner.lock().unwrap().pos
    }

    pub fn remaining(&self) -> u64 {
        let g = self.inner.lock().unwrap();
        g.end.saturating_sub(g.pos)
    }

    /// Số byte tối đa được phép ghi ngay lúc này (tối đa `want`).
    pub fn allow(&self, want: u64) -> u64 {
        let g = self.inner.lock().unwrap();
        g.end.saturating_sub(g.pos).min(want)
    }

    /// Xác nhận đã ghi `n` byte. Trả về số byte thực sự được tính (không vượt `end` nếu bị cắt bớt).
    pub fn commit(&self, n: u64) -> u64 {
        let mut g = self.inner.lock().unwrap();
        let new_pos = (g.pos + n).min(g.end);
        let counted = new_pos - g.pos;
        g.pos = new_pos;
        counted
    }

    /// (còn lại, tốc độ byte/s, số giây đã chạy)
    fn stats(&self) -> (u64, f64, f64) {
        let g = self.inner.lock().unwrap();
        let el = g.start.elapsed().as_secs_f64();
        (g.end.saturating_sub(g.pos), (g.pos - g.base) as f64 / el.max(1e-3), el)
    }

    /// Lấy toàn bộ phần còn lại (dùng khi kết nối giữ đoạn này gần như đứng yên).
    fn take_all(&self) -> Option<(u64, u64)> {
        let mut g = self.inner.lock().unwrap();
        if g.pos >= g.end {
            return None;
        }
        let r = (g.pos, g.end);
        g.end = g.pos;
        Some(r)
    }

    /// Cắt nửa sau của đoạn. Trả về (mid, old_end) nếu đủ lớn để chia.
    fn split(&self, min_split: u64) -> Option<(u64, u64)> {
        let mut g = self.inner.lock().unwrap();
        let rem = g.end.saturating_sub(g.pos);
        if rem < min_split.saturating_mul(2) {
            return None;
        }
        let mid = g.pos + rem / 2;
        let old_end = g.end;
        g.end = mid;
        Some((mid, old_end))
    }
}

struct Inner {
    pending: VecDeque<Arc<Seg>>,
    active: Vec<Arc<Seg>>,
}

pub struct Scheduler {
    inner: Mutex<Inner>,
    min_split: u64,
}

impl Scheduler {
    pub fn new(ranges: Vec<(u64, u64)>, min_split: u64) -> Self {
        let pending = ranges
            .into_iter()
            .filter(|(a, b)| a < b)
            .map(|(a, b)| Seg::new(a, b))
            .collect();
        Self { inner: Mutex::new(Inner { pending, active: Vec::new() }), min_split }
    }

    /// Lấy việc tiếp theo: ưu tiên đoạn chưa ai nhận, nếu hết thì cắt đôi đoạn còn nhiều nhất.
    pub fn next(&self) -> Option<Arc<Seg>> {
        let mut g = self.inner.lock().unwrap();
        if let Some(s) = g.pending.pop_front() {
            g.active.push(s.clone());
            return Some(s);
        }
        let victim = g.active.iter().max_by_key(|s| s.remaining())?.clone();
        let (from, to) = match victim.split(self.min_split) {
            Some(r) => r,
            None => Self::endgame(&g.active)?,
        };
        let seg = Seg::new(from, to);
        g.active.push(seg.clone());
        Some(seg)
    }

    /// Giai đoạn cuối: không còn đoạn nào đủ lớn để chia bình thường, nhưng một kết nối chậm
    /// có thể giữ cả file lại. Chọn đoạn cần lâu nhất để xong:
    /// - kết nối gần như đứng (chậm hơn 1/4 trung vị sau 2s) → giao toàn bộ phần còn lại cho kết nối mới;
    /// - còn lại → cắt đôi tiếp (xuống tới 64 KB) để nhiều kết nối cùng làm phần đuôi.
    fn endgame(active: &[Arc<Seg>]) -> Option<(u64, u64)> {
        let stats: Vec<(&Arc<Seg>, u64, f64, f64)> = active
            .iter()
            .map(|s| {
                let (rem, sp, el) = s.stats();
                (s, rem, sp, el)
            })
            .filter(|x| x.1 > 0)
            .collect();
        let mut known: Vec<f64> = stats.iter().filter(|x| x.3 >= 0.5).map(|x| x.2).collect();
        known.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = *known.get(known.len() / 2)?;
        let eta = |x: &(&Arc<Seg>, u64, f64, f64)| {
            let speed = if x.3 >= 0.5 { x.2 } else { median };
            x.1 as f64 / speed.max(1024.0)
        };
        let worst = stats.iter().max_by(|a, b| eta(*a).partial_cmp(&eta(*b)).unwrap_or(std::cmp::Ordering::Equal))?;
        if eta(worst) < ENDGAME_ETA {
            return None;
        }
        if worst.3 >= 2.0 && worst.2 < median * 0.25 {
            return worst.0.take_all();
        }
        worst.0.split(TAIL_MIN)
    }

    pub fn finish(&self, seg: &Arc<Seg>) {
        let mut g = self.inner.lock().unwrap();
        g.active.retain(|s| !Arc::ptr_eq(s, seg));
    }

    /// Xong khi mọi byte đã ghi (kể cả khi vài worker chậm vẫn chưa nhận ra đoạn của mình đã bị lấy).
    pub fn is_done(&self) -> bool {
        let g = self.inner.lock().unwrap();
        g.pending.is_empty() && g.active.iter().all(|s| s.remaining() == 0)
    }

    /// Các khoảng byte chưa tải xong (để lưu trạng thái resume).
    pub fn snapshot(&self) -> Vec<(u64, u64)> {
        let g = self.inner.lock().unwrap();
        let mut v: Vec<(u64, u64)> = g
            .pending
            .iter()
            .chain(g.active.iter())
            .map(|s| s.range())
            .filter(|(a, b)| a < b)
            .collect();
        v.sort_unstable();
        v
    }
}
