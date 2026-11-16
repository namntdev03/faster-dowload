//! Một lượt tải: trạng thái, chạy/dừng, chuyển sang dạng hiển thị (JobView) và dạng lưu (JobRecord).

use crate::jobs::{Hub, Jobs};
use fastdl::config::{parse_header, Options};
use fastdl::report::{Cancelled, Reporter};
use fastdl::{engine, video};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Video,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Running,
    Paused,
    Done,
    Error,
}

pub struct NewJob {
    pub url: String,
    pub dir: PathBuf,
    pub kind: Kind,
    pub format: String,
    pub connections: usize,
    pub headers: Vec<String>,
}

/// Dạng ghi vào jobs.json.
#[derive(Serialize, Deserialize)]
pub struct JobRecord {
    pub id: u64,
    pub url: String,
    pub dir: String,
    pub kind: Kind,
    pub format: String,
    pub connections: usize,
    pub headers: Vec<String>,
    pub name: String,
    pub path: Option<String>,
    pub status: Status,
    pub error: String,
    pub downloaded: u64,
    pub total: u64,
}

/// Dạng gửi lên giao diện.
#[derive(Serialize)]
pub struct JobView {
    pub id: u64,
    pub url: String,
    pub name: String,
    pub kind: Kind,
    pub status: &'static str,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    pub conns: usize,
    pub error: String,
    pub note: String,
    pub dir: String,
}

pub struct Job {
    pub id: u64,
    pub url: String,
    pub dir: PathBuf,
    pub kind: Kind,
    pub format: String,
    pub connections: usize,
    pub headers: Vec<String>,
    pub rep: Arc<Reporter>,
    status: Mutex<Status>,
    error: Mutex<String>,
    hub: Weak<Hub>,
}

impl Job {
    pub fn new(id: u64, n: NewJob, hub: Weak<Hub>) -> Arc<Self> {
        Arc::new(Job {
            id,
            url: n.url,
            dir: n.dir,
            kind: n.kind,
            format: n.format,
            connections: n.connections,
            headers: n.headers,
            rep: Reporter::new(),
            status: Mutex::new(Status::Queued),
            error: Mutex::new(String::new()),
            hub,
        })
    }

    /// Khôi phục từ jobs.json. Mục đang chạy/chờ lúc tắt app trở thành "Đã dừng".
    pub fn from_record(r: JobRecord, hub: Weak<Hub>) -> Arc<Self> {
        let status = match r.status {
            Status::Running | Status::Queued => Status::Paused,
            s => s,
        };
        let rep = Reporter::new();
        rep.set_name(&r.name);
        if let Some(p) = &r.path {
            rep.set_path(PathBuf::from(p));
        }
        rep.update(r.downloaded, r.total, 0, 0);
        Arc::new(Job {
            id: r.id,
            url: r.url,
            dir: PathBuf::from(r.dir),
            kind: r.kind,
            format: r.format,
            connections: r.connections,
            headers: r.headers,
            rep,
            status: Mutex::new(status),
            error: Mutex::new(r.error),
            hub,
        })
    }

    pub fn record(&self) -> JobRecord {
        JobRecord {
            id: self.id,
            url: self.url.clone(),
            dir: self.dir.to_string_lossy().into_owned(),
            kind: self.kind,
            format: self.format.clone(),
            connections: self.connections,
            headers: self.headers.clone(),
            name: self.rep.name(),
            path: self.rep.path().map(|p| p.to_string_lossy().into_owned()),
            status: self.status(),
            error: self.error.lock().unwrap().clone(),
            downloaded: self.rep.shown().0,
            total: self.rep.shown().1,
        }
    }

    pub fn status(&self) -> Status {
        *self.status.lock().unwrap()
    }

    pub fn view(&self) -> JobView {
        let st = self.status();
        let running = st == Status::Running;
        let status = match st {
            Status::Running if self.rep.cancelled() => "pausing",
            Status::Running => "running",
            Status::Queued => "queued",
            Status::Paused => "paused",
            Status::Done => "done",
            Status::Error => "error",
        };
        let (downloaded, total) = self.rep.shown();
        JobView {
            id: self.id,
            url: self.url.clone(),
            name: self.rep.name(),
            kind: self.kind,
            status,
            downloaded,
            total,
            speed: if running { self.rep.speed.load(Ordering::Relaxed) } else { 0 },
            conns: if running { self.rep.conns.load(Ordering::Relaxed) } else { 0 },
            error: self.error.lock().unwrap().clone(),
            note: self.rep.note(),
            dir: self.dir.to_string_lossy().into_owned(),
        }
    }

    fn notify(&self) {
        if let Some(h) = self.hub.upgrade() {
            Jobs(h).changed();
        }
    }

    /// Dừng: mục đang chờ thì chuyển ngay sang "Đã dừng", mục đang chạy thì báo engine dừng.
    pub fn pause(&self) {
        let mut st = self.status.lock().unwrap();
        let cur = *st;
        match cur {
            Status::Queued => {
                *st = Status::Paused;
                drop(st);
                self.notify();
            }
            Status::Running => self.rep.cancel.store(true, Ordering::Relaxed),
            _ => {}
        }
    }

    /// Đưa mục đã dừng/lỗi vào hàng đợi. Trả về true nếu có thay đổi.
    pub fn enqueue(&self) -> bool {
        let mut st = self.status.lock().unwrap();
        if matches!(*st, Status::Paused | Status::Error) {
            *st = Status::Queued;
            return true;
        }
        false
    }

    /// Chỉ hàng đợi (Jobs::pump) gọi hàm này: chạy một mục đang chờ.
    pub(crate) fn start(self: &Arc<Self>) {
        {
            let mut st = self.status.lock().unwrap();
            if *st != Status::Queued {
                return;
            }
            *st = Status::Running;
        }
        self.error.lock().unwrap().clear();
        self.rep.reset();
        let job = self.clone();
        tauri::async_runtime::spawn(async move {
            let res = job.run().await;
            {
                let mut st = job.status.lock().unwrap();
                match res {
                    Ok(()) => *st = Status::Done,
                    Err(e) if e.downcast_ref::<Cancelled>().is_some() => *st = Status::Paused,
                    Err(e) => {
                        *job.error.lock().unwrap() = format!("{e:#}");
                        *st = Status::Error;
                    }
                }
            }
            job.rep.speed.store(0, Ordering::Relaxed);
            job.notify(); // lưu lại và cho mục chờ kế tiếp chạy
        });
    }

    async fn run(&self) -> anyhow::Result<()> {
        let mut o = Options::default();
        o.max_conns = self.connections.clamp(1, 64);
        o.init_conns = o.init_conns.min(o.max_conns);
        o.quiet = true;
        o.reporter = Some(self.rep.clone());
        for h in &self.headers {
            o.headers.push(parse_header(h)?);
        }
        match self.kind {
            Kind::File => {
                engine::download(&self.url, &self.dir, None, &o).await?;
            }
            Kind::Video => video::run(&self.url, &self.dir, &self.format, 16, &o).await?,
        }
        Ok(())
    }

    /// Xóa file tạm (và file đã xong nếu `delete_done`). Chờ job dừng hẳn rồi mới xóa.
    pub fn cleanup(self: Arc<Self>, delete_done: bool) {
        self.pause();
        std::thread::spawn(move || {
            for _ in 0..100 {
                if self.status() != Status::Running {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let mut targets = self.rep.temps();
            if let Some(p) = self.rep.path() {
                targets.push(p);
            }
            for path in &targets {
                let mut part = path.as_os_str().to_owned();
                part.push(".part");
                let mut state = part.clone();
                state.push(".fastdl");
                let _ = std::fs::remove_file(PathBuf::from(part));
                let _ = std::fs::remove_file(PathBuf::from(state));
            }
            for t in self.rep.temps() {
                let _ = std::fs::remove_file(t); // luồng hình/tiếng chưa ghép
            }
            if delete_done && self.status() == Status::Done {
                if let Some(p) = self.rep.path() {
                    let _ = std::fs::remove_file(p);
                }
            }
        });
    }
}
