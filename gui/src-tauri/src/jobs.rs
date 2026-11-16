//! Danh sách lượt tải: hàng đợi (giới hạn số mục chạy cùng lúc) và lưu vào jobs.json.

use crate::job::{Job, JobRecord, JobView, NewJob, Status};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Hub {
    map: Mutex<BTreeMap<u64, Arc<Job>>>,
    next: AtomicU64,
    max_active: AtomicUsize,
    dirty: AtomicBool,
    store: Mutex<Option<PathBuf>>,
    pump_lock: Mutex<()>,
}

#[derive(Clone)]
pub struct Jobs(pub(crate) Arc<Hub>);

impl Jobs {
    pub fn new() -> Self {
        Jobs(Arc::new(Hub {
            map: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(0),
            max_active: AtomicUsize::new(3),
            dirty: AtomicBool::new(false),
            store: Mutex::new(None),
            pump_lock: Mutex::new(()),
        }))
    }

    pub fn add(&self, n: NewJob) -> Arc<Job> {
        let id = self.0.next.fetch_add(1, Ordering::Relaxed) + 1;
        let job = Job::new(id, n, Arc::downgrade(&self.0));
        self.0.map.lock().unwrap().insert(id, job.clone());
        job
    }

    pub fn get(&self, id: u64) -> Option<Arc<Job>> {
        self.0.map.lock().unwrap().get(&id).cloned()
    }

    pub fn remove(&self, id: u64) -> Option<Arc<Job>> {
        let j = self.0.map.lock().unwrap().remove(&id);
        self.changed();
        j
    }

    pub fn views(&self) -> Vec<JobView> {
        self.all().iter().map(|j| j.view()).collect()
    }

    fn all(&self) -> Vec<Arc<Job>> {
        self.0.map.lock().unwrap().values().cloned().collect()
    }

    pub fn set_max_active(&self, n: usize) {
        self.0.max_active.store(n.clamp(1, 10), Ordering::Relaxed);
        self.pump();
    }

    /// Có gì thay đổi: đánh dấu cần lưu và cho mục chờ chạy nếu còn chỗ.
    pub fn changed(&self) {
        self.0.dirty.store(true, Ordering::Relaxed);
        self.pump();
    }

    /// Chạy các mục đang chờ (theo thứ tự thêm vào) cho tới khi đủ số mục chạy cùng lúc.
    pub fn pump(&self) {
        let _g = self.0.pump_lock.lock().unwrap();
        let jobs = self.all();
        let max = self.0.max_active.load(Ordering::Relaxed);
        let mut running = jobs.iter().filter(|j| j.status() == Status::Running).count();
        for j in jobs.iter().filter(|j| j.status() == Status::Queued) {
            if running >= max {
                break;
            }
            j.start();
            running += 1;
        }
    }

    /// Nạp jobs.json lúc mở app, rồi chạy luồng nền lưu lại mỗi 2 giây khi có thay đổi.
    pub fn open(&self, path: PathBuf) {
        let records: Vec<JobRecord> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        {
            let mut map = self.0.map.lock().unwrap();
            for r in records {
                let id = r.id;
                self.0.next.fetch_max(id, Ordering::Relaxed);
                map.insert(id, Job::from_record(r, Arc::downgrade(&self.0)));
            }
        }
        *self.0.store.lock().unwrap() = Some(path);
        let me = self.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            let running = me.all().iter().any(|j| j.status() == Status::Running);
            if me.0.dirty.swap(false, Ordering::Relaxed) || running {
                me.save();
            }
        });
    }

    pub fn save(&self) {
        let Some(path) = self.0.store.lock().unwrap().clone() else { return };
        let records: Vec<JobRecord> = self.all().iter().map(|j| j.record()).collect();
        if let Err(e) = write_atomic(&path, &records) {
            eprintln!("không lưu được danh sách tải: {e:#}");
        }
    }
}

fn write_atomic(path: &Path, records: &[JobRecord]) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(records)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
