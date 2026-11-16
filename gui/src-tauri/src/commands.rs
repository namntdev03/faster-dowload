use crate::jobs::{Jobs, JobView, Kind, NewJob};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use tauri::{Manager, State};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddReq {
    pub urls: Vec<String>,
    pub dir: String,
    pub kind: Kind,
    pub format: String,
    pub connections: usize,
    pub headers: Vec<String>,
}

#[tauri::command]
pub fn default_dir(app: tauri::AppHandle) -> String {
    app.path()
        .download_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[tauri::command]
pub fn add_download(state: State<'_, Jobs>, req: AddReq) -> Result<Vec<u64>, String> {
    if req.dir.trim().is_empty() {
        return Err("Chưa chọn thư mục lưu".into());
    }
    let mut ids = Vec::new();
    for url in req.urls.iter().map(|u| u.trim()).filter(|u| !u.is_empty()) {
        let job = state.add(NewJob {
            url: url.to_string(),
            dir: PathBuf::from(&req.dir),
            kind: req.kind,
            format: req.format.clone(),
            connections: req.connections,
            headers: req.headers.clone(),
        });
        job.start();
        ids.push(job.id);
    }
    Ok(ids)
}

#[tauri::command]
pub fn list_jobs(state: State<'_, Jobs>) -> Vec<JobView> {
    state.views()
}

#[tauri::command]
pub fn pause_job(state: State<'_, Jobs>, id: u64) {
    if let Some(j) = state.get(id) {
        j.pause();
    }
}

#[tauri::command]
pub fn resume_job(state: State<'_, Jobs>, id: u64) {
    if let Some(j) = state.get(id) {
        j.start();
    }
}

#[tauri::command]
pub fn remove_job(state: State<'_, Jobs>, id: u64, delete_file: bool) {
    if let Some(j) = state.remove(id) {
        j.cleanup(delete_file);
    }
}

/// Mở thư mục chứa file (hoặc chọn sẵn file nếu hệ điều hành hỗ trợ).
#[tauri::command]
pub fn reveal_job(state: State<'_, Jobs>, id: u64) -> Result<(), String> {
    let job = state.get(id).ok_or("Không tìm thấy mục tải")?;
    let file = job.rep.path().filter(|p| p.exists());
    let dir = job.dir.clone();
    let res = if cfg!(target_os = "macos") {
        match &file {
            Some(f) => Command::new("open").arg("-R").arg(f).spawn(),
            None => Command::new("open").arg(&dir).spawn(),
        }
    } else if cfg!(target_os = "windows") {
        match &file {
            Some(f) => Command::new("explorer").arg(format!("/select,{}", f.display())).spawn(),
            None => Command::new("explorer").arg(&dir).spawn(),
        }
    } else {
        Command::new("xdg-open").arg(&dir).spawn()
    };
    res.map(|_| ()).map_err(|e| e.to_string())
}
