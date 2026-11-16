# fastdl

Trình tải file/video đa kết nối viết bằng Rust, hướng tới tốc độ như IDM. Có CLI và GUI (Tauri).

## Build CLI

```bash
cargo build --release
# binary: target/release/fastdl
```

```bash
fastdl get "https://example.com/big.iso" -d ~/Downloads -c 32
fastdl get URL -H "Cookie: a=b" -H "Referer: https://example.com"
fastdl video "https://www.youtube.com/watch?v=..." -d ~/Movies   # cần: brew install yt-dlp ffmpeg
fastdl video URL -f "bv*[height<=1080]+ba/b"
```

Tải dở bị ngắt: chạy lại đúng lệnh cũ, file `.part` + `.fastdl` sẽ được tiếp tục.

## Chạy GUI

```bash
cd gui
npm install
npx tauri icon src-tauri/icons/icon.png   # chỉ cần khi build bản cài đặt (tạo đủ icon .icns/.ico)
npm run tauri dev                          # chạy thử
npm run tauri build                        # đóng gói app
```

GUI: thêm nhiều link cùng lúc, tự nhận diện file/video, chọn chất lượng, số kết nối, header; dừng/tiếp tục (resume), mở thư mục, xóa (kèm file hoặc không).

## Thiết kế

- `probe`: một request `Range: bytes=0-0` cho biết dung lượng, hỗ trợ range, tên file, ETag.
- `sched`: work-stealing. Worker xong việc sẽ cắt đôi đoạn còn nhiều nhất của worker khác.
- `engine`: bắt đầu 4 kết nối, mỗi 0.5s đo tốc độ, còn tăng >5% thì thêm 2 kết nối (tối đa `-c`).
- `worker`: ghi trực tiếp vào vị trí (pwrite) trên file cấp phát sẵn, thử lại có backoff.
- `state`: lưu các khoảng còn lại ra `.fastdl` để resume.
- `report`: cầu nối cho GUI (tiến độ, tên, đường dẫn, lệnh dừng).
- HTTP/1.1 có chủ đích để mỗi đoạn là một kết nối TCP riêng.
- `video`: yt-dlp lấy URL trực tiếp, engine tải từng luồng rồi ghép bằng ffmpeg;
  HLS/DASH hoặc khi lỗi thì chuyển sang yt-dlp `-N` (không có tiến độ chi tiết trong GUI).

## Đo tốc độ

```bash
cargo build --release
./target/release/fastdl bench bench/urls.txt            # so với curl và aria2c (nếu đã cài)
./target/release/fastdl bench bench/urls.txt -n 5 --with aria2c -o ket-qua.csv
```

Mỗi lần đo in: dung lượng, thời gian, tốc độ TB, tốc độ đỉnh (cửa sổ 1s), TTFB (byte đầu tiên),
lúc đạt 90% tốc độ đỉnh, thời gian của 5% cuối, số kết nối. Cuối cùng in bảng trung vị và tỉ lệ
fastdl / công cụ khác; chi tiết từng lần nằm trong CSV. File tải về bị xóa sau mỗi lần đo.
IDM chỉ có trên Windows: đo tay rồi thêm dòng với `tool = idm` vào CSV.

## Build tự động (GitHub Actions)

`.github/workflows/build.yml` build CLI và app cho macOS (Apple Silicon + Intel), Windows, Linux.

- Push lên `main` hoặc mở PR: tải file cài đặt ở tab **Actions** → lần chạy → mục **Artifacts**.
- Phát hành: `git tag v0.1.0 && git push origin v0.1.0` → tạo bản **Release nháp** kèm file cài đặt.
- Bản macOS được ký ad-hoc (`signingIdentity: "-"`): chạy được trên Apple Silicon, không bị báo "đã hỏng".
  Vì không phải chứng chỉ Apple Developer nên Gatekeeper vẫn hỏi lần đầu: chuột phải → Open,
  hoặc `xattr -dr com.apple.quarantine /Applications/FastDL.app`.
