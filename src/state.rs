use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Trạng thái để resume: những khoảng byte còn lại cần tải.
#[derive(Serialize, Deserialize, Debug)]
pub struct State {
    pub url: String,
    pub size: u64,
    pub validator: Option<String>,
    pub remaining: Vec<(u64, u64)>,
}

/// Đường dẫn file trạng thái cho file `.part`.
pub fn path_for(part: &Path) -> PathBuf {
    let mut s = part.as_os_str().to_owned();
    s.push(".fastdl");
    PathBuf::from(s)
}

pub fn load(p: &Path) -> Option<State> {
    let bytes = std::fs::read(p).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Ghi qua file tạm rồi đổi tên để không bao giờ để lại file hỏng nửa chừng.
pub fn save(p: &Path, st: &State) -> Result<()> {
    let tmp = p.with_extension("fastdl.tmp");
    std::fs::write(&tmp, serde_json::to_vec(st)?)?;
    std::fs::rename(tmp, p)?;
    Ok(())
}
