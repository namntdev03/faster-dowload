use std::fs::File;
use std::io;

/// Ghi `buf` tại vị trí `off` mà không đụng tới con trỏ file (an toàn khi nhiều worker dùng chung).
#[cfg(unix)]
pub fn write_at(f: &File, buf: &[u8], off: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    f.write_all_at(buf, off)
}

#[cfg(windows)]
pub fn write_at(f: &File, buf: &[u8], off: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    let mut done = 0usize;
    while done < buf.len() {
        let n = f.seek_write(&buf[done..], off + done as u64)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "ghi được 0 byte"));
        }
        done += n;
    }
    Ok(())
}
