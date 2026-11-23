# BÁO CÁO TÌNH TRẠNG PHÁT TRIỂN: `money-flow` (DEVELOPMENT STATUS REPORT)

**Sản phẩm**: `money-flow` (VAS Enterprise Double-Entry Accounting Engine)  
**Crate**: `backend` v0.1.0 (Rust Edition 2024, Axum 0.8, SQLite WAL, `rust_decimal`)  
**Kho mã nguồn**: `d:\money-flow`  
**Ngày lập báo cáo**: 2026-11-23  
**Tiêu chuẩn đánh giá**: Bằng chứng thực tế (`[FACT]`), suy luận kỹ thuật (`[INFERENCE]`), chưa xác định (`[UNKNOWN]`). Chế độ READ-ONLY tuyệt đối với mã nguồn và cấu hình; các quyết định D1–D6 và đề xuất nghiệp vụ được định danh là **ĐỀ XUẤT (PROPOSALS)** cho đến khi có văn bản phê duyệt chính thức.

---

## 1. Kết luận tổng quan (Verdict)

### Đánh giá: **HEALTHY (THEO DÕI CÁC ĐIỀU KIỆN GO-LIVE)**
> **Tóm tắt cốt lõi**: Động cơ kế toán tổng hợp, logic định khoản kép cân bằng tuyệt đối, cơ chế lưu vết bất biến SQLite WAL kèm trigger, xác thực phân quyền Bearer JWT RBAC, danh mục tài khoản động (103 tài khoản mẫu TT 99) và cơ chế khôi phục thảm họa zero-diff đã được xây dựng hoàn chỉnh, tái cấu trúc theo mô hình SOLID dưới 500 dòng/file và được bảo chứng bởi **256/256 kiểm thử vượt qua 100% với 0 cảnh báo Clippy** `[FACT: cargo test, cargo clippy, commit a15226b]`. Tuy nhiên, các quyết định D1–D6 hiện là các phương án đề xuất (PROPOSALS) cần văn bản phê duyệt chính thức trước khi triển khai Phase 1 cutover và Phase 2 hóa đơn điện tử `[INFERENCE]`.

---

## 2. Bảng chỉ số chính (Key Numbers Table)

| Chỉ số kỹ thuật | Giá trị thực tế | Lệnh thực thi / Bằng chứng kiểm tra | Phân loại |
| :--- | :--- | :--- | :---: |
| **Tổng số Commit** | `7` | `git log --all --oneline \| Measure-Object -Line` | `[FACT]` |
| **Số ngày phát triển thực tế** | `2` (2026-11-20, 2026-11-23) | `git log --all --pretty=format:'%ad' --date=short \| Sort -Unique` | `[FACT]` |
| **Số lập trình viên đóng góp** | `1` (VŨ QUANG TÔN) | `git shortlog -sne --all` | `[FACT]` |
| **Tần suất commit (4 tuần gần nhất)** | `7` commit / 0 commit trước đó | `git log --all --since="90 days ago"` | `[FACT]` |
| **Tổng số kiểm thử vượt qua** | **`256 / 256`** (100% pass) | `cargo test --manifest-path backend/Cargo.toml` | `[FACT]` |
| - *Unit Tests (`--lib`)* | `55` | `src/lib.rs` test runner output | `[FACT]` |
| - *API Integration Tests* | `47` | `tests/api_tests.rs` test runner output | `[FACT]` |
| - *E2E Multi-Tier Tests* | `149` (Tiers 1–4) | `tests/e2e_tests.rs` test runner output | `[FACT]` |
| - *Property Fuzzing Suites* | `5` (1,100+ vòng lặp) | `tests/property_tests.rs` test runner output | `[FACT]` |
| **Trạng thái Clippy Linter** | `0 warnings` (-D warnings) | `cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings` | `[FACT]` |
| **Định dạng mã nguồn (`fmt`)** | `Clean (0 diffs)` | `cargo fmt --manifest-path backend/Cargo.toml --check` | `[FACT]` |
| **Dòng mã nguồn thực thi (Prod)** | `7,032 dòng` | `backend/src/*.rs` loại trừ `tests.rs` và `#[cfg(test)]` | `[FACT]` |
| **Dòng mã kiểm thử (Test)** | `10,723 dòng` | `backend/tests/*.rs` (8,227) + `tests.rs` và inline tests (2,496) | `[FACT]` |
| **Tỷ lệ Test / Code** | `1.52 : 1.0` | `10,723 test LOC / 7,032 prod LOC` | `[FACT]` |
| **Số file prod > 1,000 dòng** | `0` (Đã phân rã hoàn toàn) | `backend/src/**/*.rs` | `[FACT]` |
| **Sử dụng số thực (`f32`/`f64`)** | `0` (Đảm bảo Zero-Float) | `Select-String` trên toàn bộ mã nguồn `backend/src/` | `[FACT]` |
| **TODO / FIXME / HACK** | `0` | `Select-String` trên toàn bộ mã nguồn `backend/src/` | `[FACT]` |
| **Dung lượng file nhị phân Release**| `4.37 MB` (4,581,376 bytes) | `Get-Item backend/target/release/backend.exe` | `[FACT]` |
| **Phiên bản rusqlite** | `0.43.0` (SQLite $\ge 3.53.0$) | `backend/Cargo.toml:17`, `db/tests.rs:13` | `[FACT]` |

---

## 3. Động lực & Tốc độ phát triển (Momentum & Velocity)

- **Chuỗi commit theo tuần**:
  - Tuần 1 (kết thúc 2026-11-22): 5 commits (`4a0fe27`, `3322421`, `ea4c57e`, `0d7f8d7`, `71795db`).
  - Tuần 2 (kết thúc 2026-11-29): 2 commits (`715f26d`, `a15226b`).
- **Xu hướng**: **TĂNG TỐC VỀ ĐỘ HOÀN THIỆN VÀ KIẾN TRÚC**. Dự án ban đầu có các tệp đơn khối lớn (`database.rs` 2,165 dòng, `openapi.rs` 1,280 dòng), nay đã được chuẩn hóa theo quy tắc AGENTS.md §6 ($\le 500$ dòng prod). Số lượng kiểm thử tăng trưởng nhảy vọt từ 88 lên **256 bài kiểm thử tự động**, bao phủ sâu 4 tầng E2E.
- **Top 10 tệp biến động nhiều nhất (Churn)**:
  1. `backend/src/database.rs`: Đã phân rã thành thư mục `backend/src/db/`.
  2. `backend/tests/api_tests.rs`: Bổ sung kiểm thử auth RBAC, idempotency và phục hồi thảm họa.
  3. `backend/tests/e2e_tests/`: 4 tệp kiểm thử tích hợp luồng nghiệp vụ với hơn 3,600 dòng test.
  4. `backend/src/openapi/`: Tách thành các submodule routes và schemas.
  5. `backend/src/catalog/`: Tách thành các cấp tài khoản (`level1_2.rs`, `level3.rs`, `level4_5.rs`).
  6. `backend/src/dto/`: Tách thành `requests.rs`, `error.rs`, `idempotency.rs`.
  7. `backend/src/handlers/`: Tách thành các use-case handlers độc lập.
  8. `backend/src/config.rs`: Mở rộng nạp cấu hình 12-factor môi trường.
  9. `backend/src/db/disaster.rs`: Động cơ sao lưu snapshot WAL và phục hồi nguyên tử rollback.
  10. `AGENTS.md`: Bổ sung các chỉ dẫn kiến trúc SOLID và ràng buộc an toàn tài chính.

---

## 4. Danh mục tính năng: Đã làm (BUILT) vs Đang kế hoạch (PLANNED)

| Tính năng / Nghiệp vụ | Trạng thái | Vị trí mã nguồn (path:line) | Kiểm thử bao phủ | Bằng chứng kiểm tra thực tế |
| :--- | :---: | :--- | :--- | :--- |
| **1. Danh mục tài khoản (COA)** | **BUILT** | `backend/src/catalog/`, `backend/src/db/account_repo.rs:1-246` | `catalog/tests.rs`, `e2e_tests/tier1` | 103 tài khoản mẫu TT 99; hỗ trợ CRUD tài khoản động, chống vòng lặp cha-con (Cycle-Guard) `[FACT]`. So với TT 99 Phụ lục II: Có 30 mã cấp 1-2, 61 mã cấp 3, 51 mã cấp 4-5 `[FACT]`. |
| **2. Định khoản kép (Debit = Credit)**| **BUILT** | `backend/src/ledger.rs:77-138`, `backend/src/db/voucher_repo.rs:50-120` | `ledger/tests.rs`, `api_tests.rs:180` | Bắt buộc $\ge 2$ dòng, không âm, không dòng rỗng, tổng Nợ = tổng Có > 0. Dùng `Decimal` `[FACT]`. |
| **3. Hủy chứng từ (Voiding)** | **BUILT** | `backend/src/db/voucher_repo.rs:140-200` | `db/tests.rs`, `api_tests.rs:450` | Thiết lập `status = 'VOIDED'`, ghi nhận actor/lý do. Trigger SQLite chặn sửa đổi dòng `[FACT]`. |
| **4. Khóa sổ 4 bước (Closing Engine)**| **BUILT** | `backend/src/closing.rs:47-270` | `closing/tests.rs:408-569` | Bước 1: 521 $\rightarrow$ 511; Bước 2: 511/515/711 $\rightarrow$ 911; Bước 3: 632/635/641/642/811/821 $\rightarrow$ 911; Bước 4: 911 $\rightarrow$ 4212 `[FACT]`. |
| **5. Khóa kỳ kế toán (Period Lock)**| **BUILT** | `backend/src/db/period_repo.rs:60-110` | `db/tests.rs`, `e2e_tests/tier1` | Chặn ghi sổ / hủy chứng từ trong kỳ đã đóng với lỗi `422 PERIOD_CLOSED` `[FACT]`. |
| **6. Chuyển đổi số dư đầu kỳ (G1)** | **PLANNED** | Chưa có mã nguồn | Chưa có | Chưa có endpoint nhập số dư đầu kỳ hoặc quy tắc tái phân loại tự động Điều 29 TT 99 `[FACT]`. |
| **7. Báo cáo tài chính (B01, B02, TB)**| **BUILT** | `backend/src/db/report_repo.rs:1-334` | `db/tests.rs:30-80`, `property_tests.rs` | Báo cáo tình hình tài chính (B01-DN), Kết quả kinh doanh (B02-DN), Bảng cân đối tài khoản `[FACT]`. |
| **8. Báo cáo Lưu chuyển tiền tệ (B03)**| **PLANNED** | Chưa có mã nguồn | Chưa có | Chưa có luồng mã hóa luồng tiền và lập báo cáo B03-DN `[FACT]`. |
| **9. Nhật ký kiểm toán (Audit Logs)** | **BUILT** | `backend/src/db/audit_repo.rs:1-143`, `schema.rs:200-260` | `db/tests.rs`, `api_tests.rs:500` | Ghi log mọi biến động trạng thái; trigger SQLite chặn UPDATE/DELETE trên bảng audit `[FACT]`. |
| **10. Sao lưu & Khôi phục thảm họa** | **BUILT** | `backend/src/db/disaster.rs:1-356`, `main.rs:180-240` | `db/tests.rs:310`, `e2e_tests/tier4` | `POST /api/v1/system/backup`, `POST /api/v1/system/restore`, lệnh CLI `restore` có rollback an toàn `[FACT]`. |
| **11. Định dạng Nhập/Xuất dữ liệu** | **PARTIAL** | `backend/src/dto/requests.rs:1-320` | `api_tests.rs` | Chỉ hỗ trợ JSON qua REST API. Chưa hỗ trợ CSV, Excel (XLSX), XML `[FACT]`. |
| **12. Xác thực & Phân quyền (RBAC)** | **BUILT** | `backend/src/auth.rs:1-171`, `handlers/mod.rs` | `auth/tests.rs`, `api_tests.rs:1-80` | Bearer JWT; kiểm tra vai trò `Accountant`, `Controller`, `Admin`; trích xuất actor từ token `[FACT]`. |
| **13. Xử lý Idempotency-Key** | **BUILT** | `backend/src/dto/idempotency.rs:1-62`, `db/voucher_repo.rs` | `api_tests.rs`, `property_tests.rs` | Băm SHA-256 payload; phát lại kết quả 201 cũ; báo lỗi 409 Conflict nếu trùng key khác payload `[FACT]`. |
| **14. Đa đơn vị / Đa khách hàng** | **PLANNED** | `backend/src/config.rs:50` | Chưa có | Hệ thống hiện chạy 1 database SQLite cho 1 tiến trình service `[FACT]`. |
| **15. Hóa đơn điện tử (QĐ 1233)** | **PLANNED** | `backend/src/models.rs:114` | Chưa có | Chỉ có trường văn bản `invoice_ref TEXT`. Chưa có XML, ký số hay adapter T-VAN `[FACT]`. |
| **16. Thuế & Quy tắc làm tròn (D2)** | **PLANNED** | Chưa có mã nguồn | Chưa có | Chưa có module tính làm tròn số nguyên đồng theo QĐ 1233 và NĐ 174 `[FACT]`. |
| **17. Kế toán hàng tồn kho (151/152)** | **PLANNED** | `backend/src/catalog/level3.rs:86-94` | `db/tests.rs:49` | Chỉ có tài khoản 151, 152 trong danh mục; chưa có tính giá xuất kho (bình quân gia quyền) `[FACT]`. |
| **18. Đa tiền tệ (Ngoại tệ)** | **PLANNED** | Chưa có mã nguồn | Chưa có | Toàn bộ sổ cái hạch toán bằng đơn vị tiền tệ duy nhất (VND) `[FACT]`. |
| **19. Chứng từ đính kèm (G5)** | **PLANNED** | Chưa có mã nguồn | Chưa có | Chưa có bảng lưu trữ file đính kèm kèm mã băm SHA-256 `[FACT]`. |
| **20. Tài liệu OpenAPI & Swagger UI** | **BUILT** | `backend/src/openapi/` | `openapi/tests.rs:1-69` | OpenAPI 3.1 JSON tự sinh và Swagger UI nhúng sẵn tại `/swagger-ui` `[FACT]`. |

---

## 5. Khoảng cách giữa Quy định pháp lý và Mã nguồn (Spec Gaps)

1. **Thông tư 99/2025/TT-BTC**:
   - *Điều 28 (Yêu cầu phần mềm kế toán)*:
     - (a) Tính nhất quán: **ĐẠT** (Ràng buộc toàn vẹn cơ sở dữ liệu SQLite).
     - (b) Không sửa chữa được dữ liệu đã ghi sổ: **ĐẠT** (Trigger SQLite ngăn chặn UPDATE/DELETE chứng từ).
     - (c) Cảnh báo can thiệp: **MỘT PHẦN** (Có audit logs và crash hooks, chưa có cảnh báo tự động ra UI).
     - (d) Xuất dữ liệu theo yêu cầu: **MỘT PHẦN** (Mới chỉ hỗ trợ JSON, chưa có Excel/XML).
     - (e) Kết nối HĐĐT và chữ ký số: **CHƯA CÓ** (Đang ở giai đoạn lập kế hoạch Phase 2).
   - *Điều 29 (Chuyển đổi số dư đầu kỳ)*: **CHƯA CÓ** (Cần xây dựng công cụ nhập số dư và ánh xạ tài khoản).
   - *Điều 3 khoản 2 (Kiểm soát nội bộ / Segregation of Duties)*: **CHƯA CÓ** (Chưa có quy trình 3 bước Tạo dự thảo $\rightarrow$ Duyệt $\rightarrow$ Ghi sổ).
   - *Phụ lục I (Chứng từ kế toán)*: **CHƯA CÓ** (Chưa có tính năng đính kèm file chứng từ gốc kèm SHA-256).
   - *Phụ lục IV (Báo cáo tài chính)*: **MỘT PHẦN** (Đã có B01-DN, B02-DN; thiếu B03-DN và B09-DN).
2. **Luật Kế toán 2015**:
   - *Điều 18 (Lập và lưu trữ chứng từ)*: **MỘT PHẦN** (Đã có lưu vết và snapshot, chưa có chính sách phân tầng lưu trữ đĩa).
   - *Điều 26 khoản 7 (In sổ điện tử khi kết thúc kỳ)*: **CHƯA CÓ** (Chưa có tính năng kết xuất và ký số sổ cái định kỳ).
   - *Điều 27 (Sửa chữa sai sót bằng chứng từ ghi điều chỉnh)*: **ĐẠT** (Cơ chế hủy phi hủy diệt và ghi đè chứng từ mới).
   - *Điều 41 (Thời hạn lưu trữ)*: **KẾ HOẠCH** (Chiến lược lưu trữ phân tầng đề xuất bởi CFO cần văn bản ban hành).
3. **Quyết định 1233/QĐ-CT (Hóa đơn điện tử)**:
   - Làm tròn tiền VND đến đồng nguyên: **CHƯA CÓ** (Cần triển khai động cơ làm tròn ở Phase 2).
   - Định dạng thông điệp 200/202 và chuẩn ký số DSig/XAdES: **CHƯA CÓ** (Phụ thuộc vào việc tích hợp nhà cung cấp T-VAN).

---

## 6. Chất lượng mã nguồn & Xu hướng rủi ro (Quality & Risks)

### Phân tích chất lượng tĩnh
- **Lỗi số thực**: **0** trường hợp `f32` hoặc `f64`. Toàn bộ tính toán tiền tệ sử dụng `rust_decimal::Decimal` `[FACT]`.
- **Cảnh báo Linter**: **0** cảnh báo trên toàn bộ các target với cờ `-D warnings` `[FACT]`.
- **Macro in ấn (`println!`/`eprintln!`)**: Có 23 dòng, toàn bộ nằm trong CLI tại `backend/src/main.rs`, hoàn toàn sạch trong mã thư viện logic kế toán `[FACT]`.
- **Unwrap / Expect trong mã nguồn prod**: 86 vị trí, tập trung ở khởi tạo regex cấu hình và unwrap Mutex runtime; không có trong logic cộng trừ cân đối Nợ - Có `[FACT]`.

### Ma trận xu hướng rủi ro

| Rủi ro kỹ thuật & nghiệp vụ | Mức độ | Xu hướng | Khu vực ảnh hưởng | Hiện trạng kiểm soát |
| :--- | :---: | :---: | :--- | :--- |
| **Tiến độ tích hợp HĐĐT (Hạn 01/01/2027)** | **CAO** | **ỔN ĐỊNH** | `backend/src/einvoice/` | Đề xuất D1 chọn tích hợp API nhà cung cấp (VNPT/Viettel/MISA) giảm thiểu rủi ro tự xây T-VAN. |
| **Lệch làm tròn số thuế đầu ra** | **CAO** | **ỔN ĐỊNH** | `ledger.rs`, `closing.rs` | Đề xuất D2 không đưa chênh lệch vào 3331, hạch toán vào 711/811. Cần văn bản duyệt. |
| **Chênh lệch số dư đầu kỳ khi chuyển đổi** | **TRUNG BÌNH** | **TĂNG** | Công cụ di trú số dư | Khách hàng chuyển từ TT 200 có thể có tài khoản riêng lẻ cần ánh xạ thủ công. |
| **Đồng bộ sao lưu file đính kèm** | **TRUNG BÌNH** | **ỔN ĐỊNH** | `disaster.rs`, Ổ cứng | Đề xuất lưu trên đĩa theo SHA-256; cần cơ chế sao lưu snapshot DB và file đồng thời. |
| **An toàn dịch vụ chạy ngầm Windows** | **THẤP** | **ỔN ĐỊNH** | Daemon wrapper | Đề xuất tạo tài khoản dịch vụ chuyên dụng, phân quyền thư mục chặt chẽ. |

---

## 7. Mức độ sẵn sàng vận hành (Operational Readiness)

- **Biên dịch Release**: Thành công `100%`, tạo ra file nhị phân `backend.exe` dung lượng `4.37 MB` (4,581,376 bytes).
- **Cấu hình 12-Factor**: Toàn bộ biến môi trường đều có giá trị mặc định an toàn:
  - `HOST` (mặc định: `127.0.0.1`)
  - `PORT` (mặc định: `3000`)
  - `DATABASE_PATH` (mặc định: `accounting.db`)
  - `BACKUP_DIR` (mặc định: `./backups`)
  - `LOG_LEVEL` (mặc định: `info`)
  - `LOG_DIR` (mặc định: `./logs`)
  - `LOG_FORMAT` (mặc định: `pretty`)
  - `JWT_SECRET` (bắt buộc trong môi trường production, có giá trị test khi kiểm thử)
  - `FISCAL_YEAR_START_MONTH` (mặc định: `1`)
- **Tài liệu triển khai**: Chưa có Dockerfile hay file dịch vụ Windows MSI/Service wrapper (`MISSING`).
- **Khôi phục thảm họa**: Đã có cả endpoint API và CLI khôi phục cơ sở dữ liệu với bài test đối chiếu zero-diff `[FACT]`.

---

## 8. Điểm nóng kiến trúc (Top 5 Hotspot Files)

1. **`backend/src/closing.rs`** (569 dòng, gồm 161 dòng test): Logic phân bổ kết chuyển 4 bước phức tạp.
2. **`backend/src/ledger.rs`** (384 dòng, gồm 88 dòng test): Trọng tâm bất biến định khoản kép.
3. **`backend/src/db/disaster.rs`** (356 dòng): Quản lý snapshot nóng SQLite WAL và rollback file dữ liệu trực tiếp.
4. **`backend/src/db/account_repo.rs`** (246 dòng): Cây phân cấp tài khoản động và thuật toán chống vòng lặp đệ quy.
5. **`backend/src/db/schema.rs`** (279 dòng): Định nghĩa DDL, pragma WAL và các trigger chặn sửa xóa SQL.

---

## 9. Ba phương án định hướng phát triển (Decision Options)

### Phương án A: Hoàn thiện tính năng kế toán theo Thông tư 99 để Go-Live (Ưu tiên cao)
- **Giải phóng năng lực**: Đưa hệ thống vào vận hành ghi sổ chính thức cho năm tài chính 2026.
- **Hạng mục chính**: Quy trình duyệt chứng từ 3 bước kèm phân quyền độc lập (P1-04/P1-05), lưu trữ chứng từ đính kèm (P1-06), báo cáo lưu chuyển tiền tệ B03-DN (P1-07), công cụ nhập số dư đầu kỳ từ TT 200 (P1-08), công nợ chi tiết 131/331 (P1-09).
- **Rủi ro lớn nhất**: Khách hàng phát sinh tài khoản đặc thù chưa kịp ánh xạ số dư trước ngày khóa sổ.
- **Điều kiện tiên quyết**: Phê duyệt chính thức tài liệu ánh xạ tài khoản và quy chế duyệt chứng từ.

### Phương án B: Tập trung tích hợp Hóa đơn điện tử theo QĐ 1233
- **Giải phóng năng lực**: Kết nối trực tiếp hệ thống hóa đơn điện tử với nhà cung cấp được cấp phép.
- **Hạng mục chính**: Động cơ làm tròn tiền VND đến đồng nguyên (P2-01), lược đồ dữ liệu hóa đơn (P2-02), adapter kết nối REST API nhà cung cấp (P2-03).
- **Rủi ro lớn nhất**: Phụ thuộc vào tiến độ cung cấp sandbox và đàm phán hợp đồng thương mại với bên thứ ba.
- **Điều kiện tiên quyết**: Ký kết hợp đồng và nhận tài khoản sandbox từ nhà cung cấp HĐĐT (D1).

### Phương án C: Mở rộng kiến trúc Đa đơn vị (Multi-Tenancy)
- **Giải phóng năng lực**: Khả năng phục vụ nhiều công ty con trong cùng một hạ tầng dịch vụ.
- **Rủi ro lớn nhất**: Rủi ro lẫn lộn dữ liệu giữa các pháp nhân trong quá trình thanh tra thuế; tốn nguồn lực trước hạn go-live 31/12/2026.
- **Điều kiện tiên quyết**: Quyết định mô hình DB riêng theo tiến trình hay gộp chung trong connection pool (CFO đã đề xuất chọn phương án tiến trình riêng cho Phase 1).

---

## 10. Danh sách câu hỏi mở cho Chủ sở hữu & CFO

1. **Nhà cung cấp HĐĐT (D1)**: Đã có danh sách 2 đơn vị cung cấp HĐĐT (ví dụ VNPT, Viettel, MISA) để lấy báo giá và mở môi trường sandbox kiểm thử hay chưa?
2. **Quy định làm tròn bằng văn bản (D2)**: Kế toán trưởng có thể ký duyệt biên bản quy tắc làm tròn (không đưa vào 3331, phần dư đưa vào 711/811) trước ngày 30/11/2026 không?
3. **Danh mục mã luồng tiền B03 (G14)**: Bộ phận kế toán có thể cung cấp bảng mã luồng tiền chuẩn theo Phụ lục IV TT 99 cho các tài khoản tiền (111, 112) không?
4. **Bảng ánh xạ số dư đầu kỳ (G1)**: Ai sẽ chịu trách nhiệm ký duyệt file seed ánh xạ các tài khoản đặc thù từ TT 200 sang TT 99?
5. **Hạ tầng máy chủ Windows (G17)**: Máy chủ triển khai thực tế sẽ sử dụng tài khoản Windows cục bộ riêng biệt hay tài khoản gMSA trong Active Directory?

---

## 11. Đánh giá độ tin cậy (Confidence) & Các yếu tố chưa xác định (UNKNOWN)

- **Mức độ tin cậy theo từng phần**:
  - Chỉ số kiểm thử, biên dịch và chất lượng tĩnh: **CAO** (Xác minh trực tiếp từ công cụ dòng lệnh).
  - Lịch sử Git, Churn và Cấu trúc tệp: **CAO** (Trích xuất trực tiếp từ Git repo).
  - Tình trạng triển khai tính năng: **CAO** (Đối chiếu mã nguồn thực tế với kiểm thử bao phủ).
  - Tuân thủ quy định pháp lý: **TRUNG BÌNH** (Các văn bản Thông tư 99 và Luật Kế toán đã được đối chiếu; quy định QĐ 1233 và NĐ 254 trích xuất từ tài liệu tóm tắt kỹ thuật `[SECONDARY]`).
- **Các yếu tố chưa xác định (UNKNOWN)**:
  - Danh tính nhà cung cấp HĐĐT chính thức và thông số API thực tế `[UNKNOWN]`.
  - Môi trường domain mạng nội bộ của máy chủ Windows Server khách hàng `[UNKNOWN]`.

---

## 12. PHẦN TỔNG HỢP & BÀI HỌC (DISTILL)

### 12.1 Bài học kinh nghiệm từ các báo cáo trước
1. **Đồng bộ số liệu kiểm thử**: Các báo cáo cũ ghi nhận 42 hoặc 88 tests do chỉ chạy một phần suite; hiện tại toàn bộ suite gồm 4 test binary độc lập với **256 bài test**. Luôn phải chạy lệnh kiểm thử đầy đủ trước khi kết luận.
2. **Tránh ghi nhận đề xuất thành quyết định**: Các phản hồi ý kiến chuyên môn từ CFO nếu chưa có chữ ký hoặc biên bản nghiệm thu thì phải ghi nhận là ĐỀ XUẤT (PROPOSALS), không được tự ý coi là đã phê duyệt chính thức.
3. **Phân rã tệp lớn kịp thời**: Việc chủ động tách `database.rs` thành các tệp nhỏ chuyên biệt trong `src/db/` giúp giảm xung đột mã nguồn và kiểm soát tốt chất lượng kiến trúc.

### 12.2 Đề xuất bổ sung vào `AGENTS.md`
1. *Không hạch toán làm tròn vào 3331*: Nghiêm cấm đưa chênh lệch làm tròn thuế vào tài khoản 3331; bắt buộc hạch toán vào 711 hoặc 811.
2. *Ràng buộc mã luồng tiền*: Mọi bút toán định khoản liên quan đến tài khoản 111, 112 phải bắt buộc có mã lưu chuyển tiền tệ hợp lệ.
3. *Đồng bộ sao lưu đính kèm*: Lệnh sao lưu hoặc phục hồi database phải luôn đi kèm kiểm tra tính toàn vẹn của thư mục chứa file chứng từ gốc (`sha256`).

### 12.3 Nợ kỹ thuật được ghi nhận
- `backend/src/main.rs:180-240`: 23 lệnh `println!` trong CLI, cần chuyển sang cơ chế logging có cấu trúc.
- `backend/src/config.rs:50-80`: 8 vị trí `.unwrap()` khi nạp cấu hình mặc định, cần thay bằng ánh xạ lỗi tường minh.
- `backend/src/db/disaster.rs:120`: Cơ chế kiểm tra chống directory traversal cần bổ sung kiểm tra tiền tố đường dẫn chuẩn hóa.
