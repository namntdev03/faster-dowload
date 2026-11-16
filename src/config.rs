use anyhow::{anyhow, Result};
use crate::report::Reporter;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING};
use std::sync::Arc;

/// Tuỳ chọn cho một lần tải.
#[derive(Clone, Debug)]
pub struct Options {
    /// Số kết nối tối đa.
    pub max_conns: usize,
    /// Số kết nối ban đầu (sẽ tự tăng dần nếu tốc độ còn tăng).
    pub init_conns: usize,
    /// Đoạn nhỏ nhất (byte) còn được phép chia đôi khi "ăn cắp" việc.
    pub min_split: u64,
    /// Số lần thử lại cho mỗi đoạn.
    pub retries: u32,
    /// Kích thước tối đa mỗi request Range (YouTube cần ~10MiB). None = không giới hạn.
    pub max_request: Option<u64>,
    /// Header thêm vào mọi request (Cookie, Referer, ...).
    pub headers: Vec<(String, String)>,
    pub user_agent: String,
    pub quiet: bool,
    /// Ép kết nối bằng IPv4 (link YouTube bị khóa theo IP của người lấy link).
    pub force_ipv4: bool,
    /// GUI dùng để nhận tiến độ và ra lệnh dừng.
    pub reporter: Option<Arc<Reporter>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_conns: 32,
            init_conns: 4,
            min_split: 1 << 20,
            retries: 8,
            max_request: None,
            headers: Vec::new(),
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                         (KHTML, like Gecko) Chrome/130.0 Safari/537.36"
                .to_string(),
            quiet: false,
            force_ipv4: false,
            reporter: None,
        }
    }
}

impl Options {
    /// Dựng HeaderMap. Luôn yêu cầu `identity` để byte-range khớp với file gốc.
    pub fn header_map(&self) -> Result<HeaderMap> {
        let mut map = HeaderMap::new();
        map.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        for (k, v) in &self.headers {
            let name = HeaderName::from_bytes(k.trim().as_bytes())
                .map_err(|e| anyhow!("tên header sai '{k}': {e}"))?;
            let val = HeaderValue::from_str(v.trim())
                .map_err(|e| anyhow!("giá trị header '{k}' sai: {e}"))?;
            map.insert(name, val);
        }
        Ok(map)
    }
}

/// Tách "Tên: giá trị" thành cặp.
pub fn parse_header(s: &str) -> Result<(String, String)> {
    let (k, v) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("header phải có dạng 'Tên: giá trị', nhận được: {s}"))?;
    Ok((k.trim().to_string(), v.trim().to_string()))
}
