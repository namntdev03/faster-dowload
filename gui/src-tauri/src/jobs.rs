use fastdl::config::{parse_header, Options};
use fastdl::report::{Cancelled, Reporter};
use fastdl::{engine, video};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Video,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Paused,
    Done,
    Error,
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
}

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

impl Job {
    pub fn view(&self) -> JobView {
        let st = *self.status.lock().unwrap();
        let status = match st {
            Status::Running if self.rep.cancelled() => "pausing",
            Status::Running => "running",
            Status::Paused => "paused",
            Status::Done => "done",
            Status::Error => "error",
        };
        JobView {
            id: self.id,
            url: self.url.clone(),
            name: self.rep.name(),
            kind: self.kind,
            status,
            downloaded: self.rep.downloaded.load(Ordering::Relaxed),
            total: self.rep.total.load(Ordering::Relaxed),
            speed: self.rep.speed.load(Ordering::Relaxed),
            conns: self.rep.conns.load(Ordering::Relaxed),
            error: self.error.lock().unwrap().clone(),
            note: self.rep.note(),
            dir: self.dir.to_string_lossy().into_owned(),
        }
    }

    pub fn is_running(&self) -> bool {
        *self.status.lock().unwrap() == Status::Running
    }

    pub fn pause(&self) {
        self.rep.cancel.store(true, Ordering::Relaxed);
    }

    /// Bắt đầu hoặc tiếp tục (engine tự resume nhờ file .fastdl).
    pub fn start(self: &Arc<Self>) {
        {
            let mut st = self.status.lock().unwrap();
            if *st == Status::Running || *st == Status::Done {
                return;
            }
            *st = Status::Running;
        }
        self.error.lock().unwrap().clear();
        self.rep.reset();
        let job = self.clone();
        tauri::async_runtime::spawn(async move {
            let res = job.run().await;
            let mut st = job.status.lock().unwrap();
            match res {
                Ok(()) => *st = Status::Done,
                Err(e) if e.downcast_ref::<Cancelled>().is_some() => *st = Status::Paused,
                Err(e) => {
                    *job.error.lock().unwrap() = format!("{e:#}");
                    *st = Status::Error;
                }
            }
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
                if !self.is_running() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let Some(path) = self.rep.path() else { return };
            let mut part = path.as_os_str().to_owned();
            part.push(".part");
            let part = PathBuf::from(part);
            let mut state = part.as_os_str().to_owned();
            state.push(".fastdl");
            let _ = std::fs::remove_file(&part);
            let _ = std::fs::remove_file(PathBuf::from(state));
            if delete_done {
                let _ = std::fs::remove_file(&path);
            }
        });
    }
}

#[derive(Default)]
pub struct Jobs {
    map: Mutex<BTreeMap<u64, Arc<Job>>>,
    next: AtomicU64,
}

pub struct NewJob {
    pub url: String,
    pub dir: PathBuf,
    pub kind: Kind,
    pub format: String,
    pub connections: usize,
    pub headers: Vec<String>,
}

impl Jobs {
    pub fn add(&self, n: NewJob) -> Arc<Job> {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let job = Arc::new(Job {
            id,
            url: n.url,
            dir: n.dir,
            kind: n.kind,
            format: n.format,
            connections: n.connections,
            headers: n.headers,
            rep: Reporter::new(),
            status: Mutex::new(Status::Paused),
            error: Mutex::new(String::new()),
        });
        self.map.lock().unwrap().insert(id, job.clone());
        job
    }

    pub fn get(&self, id: u64) -> Option<Arc<Job>> {
        self.map.lock().unwrap().get(&id).cloned()
    }

    pub fn remove(&self, id: u64) -> Option<Arc<Job>> {
        self.map.lock().unwrap().remove(&id)
    }

    pub fn views(&self) -> Vec<JobView> {
        self.map.lock().unwrap().values().map(|j| j.view()).collect()
    }
}
