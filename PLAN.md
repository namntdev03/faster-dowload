# FastDL – kế hoạch code

Mỗi bước ghi rõ: làm gì, sửa file nào, thế nào là xong. Làm theo thứ tự; mỗi bước build + chạy thử
(`cd gui && npm run tauri dev`) rồi mới sang bước sau. Giữ mọi file ≤ 300 dòng.

## Đã xong (16/11)

- GUI mới: sidebar lọc + đếm, thẻ tải, sáng/tối, biểu đồ tốc độ, hộp thêm link (nhận diện file/video,
  mức kết nối 4/8/16/32, nhớ cài đặt), xác nhận khi xóa, toast, phím tắt ⌘N / ⌘F / dán link.
- `src/ytrun.rs` (mới): yt-dlp dự phòng giờ **dừng được** (kill tiến trình → trạng thái "Đã dừng", bấm
  Tiếp tục thì yt-dlp tự nối file .part), **có tiến độ + tốc độ** trên GUI, báo **lỗi thật** của yt-dlp
  (3 dòng ERROR/WARNING cuối) thay vì "yt-dlp thất bại". CLI vẫn in như cũ. Thêm feature tokio `io-util`.
- `src/video.rs`: ghi chú "Luồng 1/2 (hình)", "Luồng 2/2 (tiếng)"; sau khi ghép thì trỏ đường dẫn
  về file cuối (nút mở thư mục và dung lượng đúng).
- Frontend: không bắn lại toast "Đã tải xong" khi reload/HMR; dán link hoặc ⌘N khi đang mở hộp thoại
  không mở chồng hộp thoại; số kết nối lưu cũ được làm tròn về mức có nút chọn.

- `fastdl bench` (mục 2.0): `src/bench.rs`, `src/bench_report.rs`, `bench/urls.txt`; engine báo số byte
  tức thời cho reporter (`worker.rs`, `single.rs`) để đo TTFB chính xác.

- Kết quả bench đầu tiên (16/11, đường truyền đỉnh ~4 MiB/s): fastdl ≥ curl/aria2c với file lớn
  (×1.1–2.3 so với curl, ×0.93–1.2 so với aria2c) nhưng chậm hơn curl ×0.54 với file 2 MB, 5% cuối mất
  2–7s, và lỗi TLS với fsn1-speed.hetzner.com.
- A4 endgame (`sched.rs`, `worker.rs`, `engine.rs`): cắt tiếp đoạn cuối xuống 64 KB, giao lại đoạn của kết nối
  gần như đứng, worker rảnh chờ thay vì thoát, xong ngay khi đủ byte. TLS dự phòng: rustls lỗi thì thử TLS
  hệ điều hành (reqwest `native-tls`).

## Bước 0 – Đưa dự án vào git

Thư mục chưa phải git repo. `git init`, commit trạng thái hiện tại bằng tài khoản namntdev03, để mỗi
bước sau là một commit, hỏng thì quay lại được. Xóa `gui/_to_delete/` trước khi commit.

## Giai đoạn 1 – Ổn định và đồng bộ UI ↔ engine

### 1.1 Lưu danh sách tải qua các lần mở app
- Mới `gui/src-tauri/src/store.rs`: đọc/ghi `jobs.json` trong `app_data_dir()` (ghi file tạm rồi rename).
- `jobs.rs`: struct `JobRecord` (Serialize/Deserialize) gồm url, dir, kind, format, connections, headers,
  name, path, status, error, downloaded, total. `Jobs` có cờ `dirty`; một task nền mỗi 2s ghi nếu dirty.
- `main.rs` (`setup`): nạp `jobs.json`; mục đang "running" lúc tắt → "paused" (engine tự resume nhờ `.fastdl`).
- Xong khi: tải dở, tắt app, mở lại → mục vẫn còn, bấm Tiếp tục chạy tiếp đúng phần trăm.

### 1.2 Hàng đợi + giới hạn số lượt tải cùng lúc
- Trạng thái mới `queued` (Rust `Status::Queued`, `api.ts` `Status`, chip "Đang chờ", lọc chung nhóm "Đang tải").
- `Jobs::pump()`: chạy tối đa N mục (mặc định 3), mục xong/dừng/lỗi thì gọi `pump()` lấy mục chờ kế tiếp.
- Lệnh mới `set_max_active(n)`; GUI đặt N trong Cài đặt (bước 3.3).
- Xong khi: thêm 10 link → chỉ 3 chạy, 7 "Đang chờ", tự chạy dần.

### 1.3 Tiến độ video gộp thành một thanh
- `ytdlp.rs`: `Part.size` lấy từ `filesize` / `filesize_approx` của format.
- `report.rs`: thêm `offset: AtomicU64` và `grand_total: AtomicU64`; `JobView.downloaded = offset + downloaded`,
  `total = grand_total` nếu > 0.
- `video.rs::native`: trước mỗi luồng đặt `offset` = tổng các luồng đã xong.
- Xong khi: video YouTube 2 luồng chạy 0→100% một lần, ETA hợp lý.

### 1.4 Xóa mục video dọn sạch file tạm
- `report.rs`: `temps: Mutex<Vec<PathBuf>>`; `video.rs` ghi tên các file luồng `.fN.ext` vào đó.
- `jobs.rs::cleanup`: xóa mỗi file tạm cùng `.part`, `.part.fastdl`; với yt-dlp dự phòng xóa
  `<tên>*.part` / `*.ytdl` trong thư mục đích.
- Xong khi: thêm video → dừng giữa chừng → Xóa → thư mục không còn rác.

### 1.5 Clipboard và thông báo hệ thống
- Thêm `tauri-plugin-clipboard-manager` (nút "Dán từ clipboard" dùng plugin thay `navigator.clipboard`,
  vì WKWebView trên macOS có thể từ chối hoặc hiện popup xin quyền).
- Thêm `tauri-plugin-notification`: tải xong/lỗi khi cửa sổ không được focus → thông báo macOS.
- Cập nhật `capabilities/default.json` với quyền của hai plugin.

## Giai đoạn 2 – Nhanh hơn IDM

Nguyên tắc: không phần mềm nào vượt được băng thông đường truyền. Nhanh hơn IDM chỉ có ở 5 chỗ:
(1) server giới hạn theo từng kết nối, (2) tận dụng nhiều nguồn (nhiều IP, nhiều mirror),
(3) dùng nhiều đường mạng cùng lúc, (4) khởi động nhanh và không chậm ở đuôi, (5) video.
Mọi kỹ thuật dưới đây phải đo trước/sau bằng 2.0, không đoán.

### 2.0 Bộ đo tốc độ — ĐÃ LÀM
- Lệnh `fastdl bench <file-urls.txt>`: mỗi URL tải 3 lần, in TTFB (byte đầu tiên), thời gian đạt 90% tốc độ
  đỉnh, tốc độ TB, thời gian 5% cuối, số kết nối. Xuất CSV để so với IDM / aria2c `-x16` / curl.
- Bộ URL cố định: file trên CDN (node, python), GitHub release, server giới hạn/kết nối, video YouTube.
- Mới `src/bench.rs`; `main.rs` thêm subcommand.

### Nhóm A – Ít công, lợi rõ

**A1. Không bỏ phí request thăm dò.** Hiện `probe` gửi `Range: bytes=0-0` rồi mới mở worker (mất 1 RTT +
1 TLS handshake). Đổi thành gửi `Range: bytes=0-` và giữ luôn response đó làm worker đầu tiên đang tải
đoạn 0 (scheduler sẽ cắt đoạn này khi worker khác vào). `probe.rs`, `engine.rs`, `worker.rs`.

**A2. Mở nhanh đủ kết nối, nhớ theo host.** Thay "bắt đầu 4, +2 mỗi 0.5s" bằng:
- lưu `hosts.json` (host → số kết nối tốt nhất, tốc độ đạt được, có giới hạn/kết nối không);
- lần sau bắt đầu thẳng ở mức đã biết; host mới thì nhân đôi 4→8→16 thay vì +2;
- file nhỏ (< 4 MB) dùng 1–2 kết nối (mở thêm chỉ tốn handshake). `engine.rs::pool`, mới `src/hosts.rs`.

**A3. Phát hiện và thay kết nối chậm.** Đo tốc độ từng worker; worker < 25% trung vị trong 3s → đóng,
mở lại kết nối mới (thường rơi vào đường/edge khác), trả đoạn về scheduler. Thay cho chờ timeout 10s.
`worker.rs` (đếm byte theo worker), `engine.rs::pool`.

**A4. Đuôi tải (endgame)** — ĐÃ LÀM, chờ đo lại. – như mục cũ: khi đoạn còn lại ít hơn worker rảnh, tải trùng nửa sau của đoạn
chậm nhất, ai xong trước thắng. `sched.rs`, `worker.rs`.

**A5. Ghi đĩa gom khối.** Mỗi worker gom dữ liệu thành khối 1–4 MB rồi mới `pwrite` (ít syscall, ít
`block_in_place`); cấp phát trước file bằng `F_PREALLOCATE` (macOS) / `fallocate` (Linux) thay vì set_len
(tránh file thưa, ghi tuần tự nhanh hơn). `io.rs`, `worker.rs`.

### Nhóm B – Vừa công, lợi lớn ở nhiều trường hợp

**B1. Rải kết nối ra mọi IP của server.** Phân giải DNS lấy tất cả bản ghi A/AAAA (CDN thường trả 2–8 IP)
và chia kết nối đều cho các IP (giữ SNI/Host đúng tên miền). Nếu từng IP bị giới hạn riêng thì tổng tăng
theo số IP. IDM không làm việc này. Dùng `reqwest::ClientBuilder::resolve_to_addrs` cho mỗi nhóm worker.
Mới `src/dns.rs`, `engine.rs`.

**B2. Nhiều nguồn (mirror) cho cùng một file.** Tải các đoạn khác nhau của cùng file từ nhiều URL:
- người dùng dán nhiều link cùng file (GUI: "Thêm nguồn phụ");
- tự nhận mirror cho các nguồn quen (python.org, nodejs.org, SourceForge, mirror Linux), hoặc file Metalink;
- kiểm tra cùng file bằng kích thước + ETag/Last-Modified + so 64 KB đầu; nguồn nào nhanh được giao đoạn lớn hơn.
`Shared.url` thành danh sách nguồn có trọng số; `worker.rs` chọn nguồn theo tốc độ đo được.

**B3. Tải song song hình + tiếng của video.** Hiện tải luồng hình xong mới tải tiếng. Chạy cả hai cùng lúc
(chia ngân sách kết nối theo kích thước) → video ngắn nhanh hơn rõ. `video.rs::native`.

**B4. Sửa YouTube 403 rồi tải YouTube đa kết nối.** (giữ nguyên các giả thuyết ở trên: tham số `&range=`,
header/cookie thiếu, User-Agent khớp client). yt-dlp tải YouTube tuần tự từng khúc 10 MB; nếu ta tải
song song nhiều khúc `&range=` thì đây là chỗ nhanh hơn IDM rõ nhất với video.

**B5. HLS/DASH bằng engine của ta.** Đọc `.m3u8`/`.mpd`, tải song song nhiều segment (16–32), giữ thứ tự,
giải mã AES-128 nếu có, nối rồi ghép ffmpeg. Mới `src/hls.rs`.

### Nhóm C – Công nhiều, dành cho trường hợp đặc biệt

**C1. Gộp nhiều đường mạng (Wi-Fi + Ethernet + điện thoại tethering).** Liệt kê interface đang có
internet, gắn mỗi nhóm kết nối vào một interface (bind local address / `IP_BOUND_IF` trên macOS),
scheduler chia đoạn theo tốc độ từng đường. Đây là cách duy nhất vượt băng thông một đường truyền.
GUI: bật/tắt từng mạng. Mới `src/netif.rs`.

**C2. HTTP/3 (QUIC) và HTTP/2 nhiều kết nối.** Server hỗ trợ HTTP/3 (Cloudflare, Google) → khởi động
nhanh hơn (0-RTT), chịu mất gói tốt hơn trên Wi-Fi. Thử `reqwest` feature `http3` (còn thử nghiệm) hoặc
`quinn`. Với server chỉ ưu tiên HTTP/2 thì mở nhiều kết nối HTTP/2 riêng thay vì ép HTTP/1.1.
Bật theo host nếu bench cho thấy nhanh hơn.

**C3. Chia băng thông giữa các lượt tải.** Khi nhiều lượt chạy cùng lúc, ưu tiên mục người dùng chọn
và mục sắp xong (tổng thời gian chờ ngắn hơn), giới hạn tổng kết nối toàn app để không nghẽn router.

**C4. Tinh chỉnh socket.** Tăng `SO_RCVBUF` cho đường dài/độ trễ cao, TLS session resumption giữa các
kết nối cùng host (giảm handshake khi mở thêm kết nối). Chỉ giữ nếu bench chứng minh có lợi.

### Thứ tự đề xuất
2.0 → A1 → A2 → A3 → A4 → B4 (YouTube) → B3 → A5 → B1 → B2 → B5 → C1 → C2/C3/C4.

## Giai đoạn 3 – Tiện ích

- **3.1 Cài đặt**: trang/hộp thoại riêng: thư mục mặc định, số lượt tải cùng lúc, kết nối mặc định,
  chất lượng video mặc định, giao diện. Gom các lựa chọn đang rải ở sidebar/hộp thêm link.
- **3.2 Kéo thả link** vào cửa sổ → mở hộp thêm link.
- **3.3 Giới hạn băng thông** (token bucket trong `worker.rs`) và hẹn giờ tải.
- **3.4 Tiện ích trình duyệt** (Chrome MV3): app mở server `127.0.0.1:<port>` có token; extension gửi
  link + cookie + referer, có menu chuột phải "Tải bằng FastDL" và bắt link video trên trang.

## Giai đoạn 4 – Đóng gói

- `npx tauri icon src-tauri/icons/icon.png` rồi `npm run tauri build`.
- Quyết định có kèm yt-dlp/ffmpeg trong bản build không (yt-dlp: Unlicense; ffmpeg: bản LGPL nếu
  muốn tránh GPL). Nếu không kèm, hiện hướng dẫn cài khi chưa có.
