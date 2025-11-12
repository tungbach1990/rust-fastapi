# Dự án Rust/Axum

## Giới thiệu
Dự án này xây dựng một API server tối ưu dựa trên Rust kết hợp với Axum, nhằm cung cấp giải pháp linh hoạt, mở rộng dễ dàng cho các ứng dụng web phức tạp. Mục tiêu của dự án là tối ưu hiệu suất, quản lý tác vụ tự động và mở rộng chức năng dựa trên các module riêng biệt.

## Kiến trúc tổng thể
Dự án chia thành nhiều module trong thư mục `features`, mỗi module đảm nhiệm một chức năng riêng biệt, giúp dễ dàng mở rộng và bảo trì:

- **深入探索**: Các chức năng khám phá và phân tích nâng cao
- **agents**: Các tác nhân tự hành
- **assets**: Quản lý tài nguyên tệp
- **autoreadme**: Sinh README tự động khi khởi động
- **cache**: Bộ nhớ đệm dữ liệu
- **client**: Giao tiếp với dịch vụ bên ngoài
- **compose**: Cấu hình và kết hợp module
- **cors**: Cấu hình Cross-Origin Resource Sharing
- **docs**: Tài liệu dự án
- **extractors**: Trích xuất dữ liệu
- **features**: Các tính năng chính
- **generator**: Sinh nội dung tự động
- **language_processors**: Xử lý ngôn ngữ tự nhiên
- **llm**: Mô hình ngôn ngữ lớn
- **memory**: Quản lý trạng thái và trí nhớ
- **oauth2**: Xác thực OAuth2
- **outlet**: Xuất dữ liệu
- **preprocess**: Tiền xử lý dữ liệu
- **prompts**: Mẫu câu hỏi, kịch bản
- **rate_limit**: Giới hạn tần suất truy cập
- **research**: Nghiên cứu, phân tích
- **scripts**: Các script tự động
- **skill-litho**: Tích hợp kỹ năng chuyên sâu
- **src**: Mã nguồn chính
- **tools**: Tiện ích hỗ trợ
- **types**: Định nghĩa kiểu dữ liệu
- **utils**: Các hàm tiện ích chung
- **waf**: Tường lửa ứng dụng

## Hướng dẫn cài đặt trên Windows (PowerShell)

1. **Cài đặt Rust và Cargo**
   ```powershell
   iex ((New-Object System.Net.WebClient).DownloadString('https://sh.rustup.rs'))
   ```
   Sau đó khởi động lại PowerShell và kiểm tra:
   ```powershell
   rustc --version
   cargo --version
   ```

2. **Clone dự án**
   ```powershell
   git clone [URL của dự án]
   cd [thư mục dự án]
   ```

3. **Cài đặt các phụ thuộc**
   ```powershell
   cargo build
   ```

4. **Tùy chỉnh biến môi trường**
   Tạo file `.env` trong thư mục gốc với các nội dung:
   ```dotenv
   SERVER_PORT=8080
   DATABASE_URL=your_database_connection_string
   # Thêm các biến cấu hình cần thiết
   ```
   Sau đó chạy:
   ```powershell
   $env:DOTENV=".env"
   ```

5. **Chạy server**
   ```powershell
   cargo run
   ```
   Server sẽ khởi động tại `http://localhost:8080` (hoặc port bạn đã cấu hình).

## Mô tả `.env` và các biến môi trường quan trọng
- `SERVER_PORT`: cổng chạy server (mặc định 8080)
- `DATABASE_URL`: kết nối database
- Các biến cấu hình bảo mật khác như API keys, OAuth tokens nếu cần

## Giới thiệu tính năng autoreadme
Khi server khởi động, module `autoreadme` sẽ chạy nền, tự động sinh và cập nhật file README.md dựa trên trạng thái hệ thống và các dữ liệu mới. Điều này giúp duy trì tài liệu cập nhật mà không cần thao tác thủ công.

## API mẫu và mở rộng
### Ví dụ API mẫu
```http
GET /api/v1/status
```
Phản hồi:
```json
{
  "status": "ok",
  "timestamp": "2023-10-24T12:00:00Z"
}
```

### Cách mở rộng API
Bạn có thể thêm các route mới trong mã nguồn chính, ví dụ:
```rust
axum::Router::new()
  .route("/api/v1/new-feature", get(new_feature_handler))
```

Cũng có thể viết các module mới trong thư mục `features`, đăng ký route trong router chính để mở rộng hệ thống.

---

Nếu cần, tôi có thể giúp bạn cụ thể hơn về cấu trúc thư mục hoặc ví dụ về code.