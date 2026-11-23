# Lộ trình đồng bộ tính năng với MantisBT 2.28.4 (`rust-mantis-bt`)

Tài liệu này tổng hợp kết quả đối chiếu giữa `rust-mantis-bt` và bản gốc **MantisBT 2.28.4** (schema v213), đồng thời đề xuất lộ trình 5 giai đoạn (**Phase 1–5**) để đưa `rust-mantis-bt` tiệm cận độ hoàn thiện của bản gốc.

> **Phạm vi:** Tài liệu **không bao gồm** phần tích hợp đồng bộ Jira (`src/jira.rs`, `src/controllers/jira.rs`, `src/views/jira.rs`, `src/models/jira.rs`), vốn là tính năng mở rộng riêng của `rust-mantis-bt`.

---

## 1. Bảng so sánh tổng quan

| Nhóm tính năng | MantisBT 2.28.4 (gốc) | `rust-mantis-bt` | Trạng thái |
| :--- | :--- | :--- | :--- |
| Issue CRUD, Notes, History, Attachments, Monitors, Relationships | Đầy đủ + Revisions, Clone, Move, Bulk, Sticky, Reminders, Sponsorship | Core CRUD, Notes, History, Attachments, Self-monitor, Relationships cơ bản | **Một phần (~50%)** |
| Workflow & Permissions UI | Ma trận Workflow Transitions, Thresholds, Graphviz, DB Config editor | Hardcode trong code (chưa có UI cấu hình) | **Chưa có UI cấu hình** |
| Dashboard (`My View`), Filter & Export | 9 box + Timeline; Filter multi-select/ANY-ALL/Custom field/Permalink; CSV, Excel, Print, Word, RSS | 6 box; Filter đơn giá trị cơ bản; chỉ có CSV export | **Một phần (~35%)** |
| Planning & Reporting | Roadmap, Changelog, Summary (15+ bảng) + MantisGraph (Chart.js), Billing (date range, cost, export) | Roadmap, Changelog, Summary (5 bảng cơ bản), Billing cơ bản | **Một phần (~40%)** |
| Projects, Subprojects, Categories, Versions, Docs | Đầy đủ + Copy giữa project + Project Documentation (`proj_doc_*`) | CRUD cơ bản + Subproject cycle check; chưa có Copy & Project Docs | **Một phần (~65%)** |
| Custom Fields & Tags | 11 kiểu render/validate/regex/required/dynamic values + Tag view/merge | 11 kiểu khai báo nhưng UI chỉ render text input; Tag CRUD cơ bản | **Một phần (~40%)** |
| Users, Auth, Preferences & Profiles | LDAP/HTTP/MD5, Signup verify, Lost pwd, CAPTCHA, Anonymous, Impersonate, Prune, Prefs, Profiles | Local auth (PBKDF2-SHA256), Signup trực tiếp, Lockout; thiếu các phần còn lại | **Một phần (~35%)** |
| Hệ thống Plugin | Kiến trúc 83 event hooks + 4 plugin thật (`MantisCoreFormatting`, `MantisGraph`, `XmlImportExport`, `Gravatar`) | Chỉ có bảng bật/tắt + 2 plugin stub (`MantisCoreFormatting` chưa nối, `EventAuditLog` đếm số) | **Chưa có** |
| API (REST & SOAP) | REST API đầy đủ (~40+ routes, OpenAPI, ETag) + SOAP API (72 hàm) | REST API cơ bản (9 routes), không có SOAP | **Một phần (~20%)** |
| Email, i18n, Admin & Tích hợp khác | SMTP/PHPMailer + DKIM/S-MIME, 66 ngôn ngữ, Timezone, Wiki, OpenSearch, 16 bộ Admin Check | Chỉ SendGrid API (plain text), chỉ tiếng Anh & UTC, chưa có Admin Check/Wiki | **Một phần (~20%)** |
| **MCP Server (AI Agent)** | Không có | Có sẵn SSE MCP Server với 8 tools | **Chỉ có ở Rust** |

---

## 2. Điểm nổi bật riêng của `rust-mantis-bt` (ngoài Jira sync)

1. **MCP Server tích hợp sẵn ([`src/mcp/`](../src/mcp/mod.rs)):**
   - Hỗ trợ Model Context Protocol (`2024-11-05`, JSON-RPC 2.0 qua SSE tại `/mcp/sse` và `/mcp/messages`) xác thực bằng API token, cung cấp 8 tools cho AI Agent (`whoami`, `list_projects`, `get_project_metadata`, `search_issues`, `get_issue`, `create_issue`, `update_issue`, `add_issue_note`).
2. **Bảo mật mật khẩu & xuất CSV hiện đại hơn mặc định:**
   - Hash mật khẩu mặc định bằng **PBKDF2-SHA256** (600.000 vòng lặp ở bản release trong [`src/auth.rs`](../src/auth.rs)) kèm tự động nâng cấp hash cũ (`MD5`/`SHA256`) và cân bằng thời gian chống timing attack (bản gốc vẫn mặc định `MD5`).
   - Xuất CSV tự động làm sạch ký tự đầu `=`, `+`, `-`, `@` để chống lỗi CSV/Excel Formula Injection.
3. **Vận hành đơn giản & tiện ích riêng:**
   - Đóng gói single-binary kèm SQLite (WAL mode), nhúng sẵn CSS/assets, có background worker gửi email định kỳ tích hợp sẵn và lệnh CLI `create-token` ([`src/lib.rs`](../src/lib.rs)).

---

## 3. Tổng quan lộ trình triển khai (5 Phases)

| Phase | Trọng tâm | Mục tiêu chính |
| :--- | :--- | :--- |
| **Phase 1** | **Quick Wins & Đóng phần làm dở** | Nối nốt schema/route đã có sẵn, sửa lỗi phân trang filter, hoàn thiện render/validate Custom Fields |
| **Phase 2** | **Daily Workflow cho Issue & Note** | Bulk actions, Clone/Move, sửa Bugnote + Revisions, Markdown & `#123`/`@mention`, Dropzone & Preview |
| **Phase 3** | **Lọc nâng cao, Báo cáo & Export** | Multi-select filter, Tùy biến cột, Print/Excel/RSS, Activity Timeline, Summary mở rộng & Biểu đồ |
| **Phase 4** | **Quản trị Cấu hình, Auth & SMTP** | Web UI cấu hình Workflow/Thresholds/Email Matrix, User Prefs & Profiles, Lost Password/Verify, SMTP |
| **Phase 5** | **Hệ sinh thái API, MCP & Migrate** | REST API parity với MantisBT 2.28, nâng cấp MCP tools, XML Import/Export, Đa ngôn ngữ & Timezone |

---

## 4. Chi tiết từng giai đoạn

### Phase 1 — Đóng các phần đang làm dở & Quick Wins

*Mục tiêu: Hoàn thiện các tính năng đã có sẵn cột DB hoặc route nhưng chưa nối lên UI/logic, tạo nền tảng sạch trước khi xây tính năng mới.*

1. **Sửa phân trang, sắp xếp & Saved Filter trên `View Issues`** ([`src/models/filter.rs`](../src/models/filter.rs), [`src/controllers/bugs.rs`](../src/controllers/bugs.rs), [`src/controllers/filters.rs`](../src/controllers/filters.rs), [`src/views/bugs.rs`](../src/views/bugs.rs))
   - Giữ nguyên query params của bộ lọc khi chuyển trang phân trang.
   - Thêm nút xóa Saved Filter trên UI (nối với route `POST /filter_delete/:id` đã có).
   - Bổ sung chọn `per_page` và sắp xếp theo cột (`id`, `priority`, `severity`, `status`, `last_updated`, `summary`).
2. **Nối đủ các trường Issue & quy tắc Project đã có trong DB** ([`src/models/bug.rs`](../src/models/bug.rs), [`src/db.rs`](../src/db.rs), [`src/controllers/bugs.rs`](../src/controllers/bugs.rs))
   - Mở UI/controller cho `sticky` (ghim issue), `due_date` (kèm highlight quá hạn), `build` (product build); đưa `os`/`os_build`/`platform` vào `update_bug_page`.
   - Gom issue của **subprojects** khi lọc theo project cha trong `list_bugs` / `filter_bugs` (vẫn kiểm tra quyền truy cập từng subproject).
   - Tự động gán `handler_id` theo `category.user_id` (default assignee) khi tạo issue mới.
   - Áp dụng `Project.access_min` trong `query_effective_access_level`.
   - Hiển thị tên trạng thái/resolution/priority/severity/user dễ đọc trong bảng **Issue History** trên UI (thay vì hiển thị mã số).
3. **Hoàn thiện Custom Fields** ([`src/models/custom_field.rs`](../src/models/custom_field.rs), [`src/views/bugs.rs`](../src/views/bugs.rs), [`src/views/custom_fields.rs`](../src/views/custom_fields.rs), [`src/db.rs`](../src/db.rs))
   - Render đúng HTML input cho cả 11 `CustomFieldType` (`select`, `select multiple`, `checkbox`, `radio`, `date`, `textarea`, `email`, `number`) thay vì chỉ `<input type="text">`; di chuyển toàn bộ logic render HTML từ controller về `src/views/`.
   - Thực thi đầy đủ `display_*`, `require_*`, `access_level_r`/`rw`, kiểm tra kiểu dữ liệu, `possible_values` (kể cả giá trị động `=versions`, `=released_versions`, `=future_versions`, `=categories`), và `valid_regexp` *(lưu ý: nếu cần regex đầy đủ sẽ phải xác nhận thêm crate `regex` vào `Cargo.toml`)*.
4. **Hoàn thiện UI CRUD còn thiếu & các chi tiết nhỏ** ([`src/controllers/projects.rs`](../src/controllers/projects.rs), [`src/views/projects.rs`](../src/views/projects.rs), [`src/controllers/planning.rs`](../src/controllers/planning.rs), [`src/views/planning.rs`](../src/views/planning.rs), [`src/controllers/tags.rs`](../src/controllers/tags.rs), [`src/views/tags.rs`](../src/views/tags.rs), [`src/controllers/auth.rs`](../src/controllers/auth.rs), [`src/views/auth.rs`](../src/views/auth.rs))
   - Thêm form sửa cho Category (`POST /manage_proj_cat_update/:cat_id`), Version (`POST /manage_proj_ver_update/:ver_id` + lưu `date_order`), Tag (`POST /tag_update/:id` + trang `tag_view_page` & đếm số issue), và News (`news_edit_page` / `news_update`).
   - Sắp xếp **Roadmap** theo `ProjectVersion.date_order` và tôn trọng cờ `released`/`obsolete`.
   - Nối checkbox `perm_login` (Remember me) trong [`src/controllers/auth.rs`](../src/controllers/auth.rs).

---

### Phase 2 — Nâng cấp Daily Workflow cho Issue & Note

*Mục tiêu: Đạt đầy đủ các thao tác cốt lõi mà Developer / QA / Manager dùng hằng ngày trên MantisBT.*

1. **Bugnotes đầy đủ, Revisions & Optimistic Locking** ([`src/db.rs`](../src/db.rs), [`src/controllers/bugs.rs`](../src/controllers/bugs.rs))
   - Sửa nội dung bugnote (`bugnote_edit_page`) và đổi public/private sau khi tạo (`bugnote_set_view_state`).
   - Bổ sung bảng `mantis_bug_revision_table` lưu lịch sử sửa cho `description`, `steps_to_reproduce`, `additional_information`, và `bugnote` kèm UI xem/xóa revision.
   - Kiểm tra `last_updated` khi cập nhật issue (Optimistic Locking) để chặn ghi đè xung đột.
2. **Clone, Move & Thao tác hàng loạt (`bug_actiongroup`)**
   - **Clone issue** (tùy chọn copy relationship & file đính kèm) và **Move issue** sang project khác.
   - Thêm checkbox chọn nhiều issue trên `View Issues` kèm các hành động hàng loạt: Close, Resolve, Assign, Delete, Move, Copy, Update Priority/Severity/Status/Category/Version/Due Date/View State/Sticky, Add Note, Attach Tags.
3. **Định dạng văn bản thật (`MantisCoreFormatting`)** ([`src/views/mod.rs`](../src/views/mod.rs))
   - Chuyển `MantisCoreFormatting` từ stub thành bộ format hoạt động thật (tôn trọng công tắc bật/tắt trong `mantis_plugin_table`):
     - Tự động link `#123` (issue, kèm tooltip & gạch ngang khi đã resolve/close), `~123` (bugnote), và URL an toàn (`rel="noopener noreferrer nofollow"`).
     - Hỗ trợ `@username` mentions kèm gửi email thông báo.
     - Hỗ trợ Markdown an toàn (escape HTML gốc trước khi render để đảm bảo chống XSS) và highlight code block.
4. **Monitors, Reminders & Attachments nâng cao**
   - Hiển thị danh sách người đang monitor trên `view_bug_page`; cho phép Developer/Manager thêm hoặc gỡ monitor cho người dùng khác.
   - Tính năng **Send Reminder** (`bug_reminder_page`) tới thành viên dự án (tùy chọn lưu thành note và tự động thêm vào danh sách monitor).
   - Cho phép đính kèm nhiều file (hỗ trợ đính kèm ngay khi tạo issue), kiểm tra whitelist/blacklist phần mở rộng (chặn inline `.svg`/HTML), và **xem trước ảnh/text/diff** trực tiếp.

---

### Phase 3 — Bộ lọc nâng cao, Báo cáo & Xuất dữ liệu

*Mục tiêu: Đáp ứng nhu cầu tìm kiếm phức tạp, theo dõi tiến độ và xuất báo cáo cho quản lý.*

1. **Bộ lọc nâng cao (Advanced Filter) & Tùy biến cột** ([`src/models/filter.rs`](../src/models/filter.rs), [`src/controllers/filters.rs`](../src/controllers/filters.rs))
   - Hỗ trợ chọn nhiều giá trị (multi-select), điều kiện `MATCH ALL / ANY`.
   - Bổ sung tiêu chí lọc: khoảng ngày (`date_submitted`, `last_updated`), `version`/`target_version`/`fixed_in_version`, `resolution`, `reproducibility`, `view_state`, `sticky`, `relationship`, `note_user`, và **Custom Fields**.
   - Trang quản lý Saved Filters (`manage_filter_page`) và **Permalink** chia sẻ đường dẫn bộ lọc.
   - Tùy biến danh sách cột hiển thị cho từng ngữ cảnh (`View`, `Print`, `CSV`, `Excel`).
2. **Mở rộng Export & Syndication**
   - Thêm **Print View HTML** (`print_all_bug_page`), **Word export**, và **Excel XML export** (vẫn giữ cơ chế chống Formula Injection).
   - Thêm **RSS Feeds** (`issues_rss`, `news_rss`) xác thực bằng API key/token.
3. **My View Timeline, Summary mở rộng, Billing & Biểu đồ** ([`src/controllers/planning.rs`](../src/controllers/planning.rs), [`src/views/planning.rs`](../src/views/planning.rs))
   - Bổ sung **Activity Timeline** và cho phép cấu hình đủ 9 box trên `My View`.
   - Mở rộng `Summary` với các bảng: By Project, By Category, By Resolution, By Date (opened/resolved/balance), Longest Open, Most Active, Developer×Resolution, Reporter Effectiveness.
   - Nâng cấp `Billing` (Time Tracking): lọc theo khoảng ngày, gom subprojects, tính chi phí theo đơn giá giờ, xuất CSV/Excel.
   - Biểu đồ thống kê & **Issues Trend** (tương đương `MantisGraph`, ưu tiên render SVG nhẹ server-side hoặc bundle script cục bộ không phụ thuộc CDN).

---

### Phase 4 — Quản trị Cấu hình, Auth/Account nâng cao & Email SMTP

*Mục tiêu: Đưa hệ thống đạt chuẩn triển khai cho doanh nghiệp (tự cấu hình không cần sửa code).*

1. **Giao diện Quản trị Cấu hình (`Manage Configuration`)**
   - Tận dụng `mantis_config_table` phân cấp theo `(project_id, user_id)` cho 5 màn hình:
     - **Permissions Report** (`adm_permissions_report`)
     - **Workflow Thresholds** (`manage_config_work_threshold`)
     - **Workflow Transitions Matrix** (`manage_config_workflow` — thay quy tắc hardcode `90 -> 20`, bổ sung kiểm tra chặn resolve nếu còn issue phụ thuộc)
     - **Email Notifications Matrix** (`manage_config_email`)
     - **Configuration Report Editor** (`adm_config_report`)
2. **User Preferences, Profiles & Quản trị User/Project nâng cao**
   - Bảng `mantis_user_pref_table`: tùy chọn nhận email chi tiết theo từng sự kiện/severity, thứ tự bugnote, `default_project`, `refresh_delay`, `timezone`.
   - Bảng `mantis_user_profile_table`: quản lý Profile cá nhân & toàn hệ thống (Platform / OS / OS Build).
   - Bổ sung **Lost Password / Reset qua email**, **Email Verification** khi đăng ký & đổi email, **Admin Re-authentication** (nhập lại mật khẩu sau 5 phút vào mục quản trị), **Impersonate User**, **Prune unused accounts**, gán project ngay trong trang sửa user, **Copy categories/versions/users/custom fields** giữa các project, và **Project Documentation** (`proj_doc_*`).
   - *(Tùy nhu cầu triển khai)*: Anonymous/Guest access, LDAP hoặc OIDC/SSO.
3. **Email SMTP & Tiện ích Quản trị hệ thống** ([`src/mailer.rs`](../src/mailer.rs))
   - Hỗ trợ **SMTP transport** (TLS/STARTTLS, auth) song song với SendGrid API hiện có *(lưu ý: cần xác nhận thêm crate như `lettre` vào `Cargo.toml` khi triển khai)*.
   - Áp dụng ma trận Email Notification & `user_pref` khi tạo hàng đợi email.
   - Trang **Manage Overview / System Info**, kiểm tra cấu hình (System Checks cơ bản), thống kê DB, công cụ di chuyển file đính kèm DB ↔ Disk, và chế độ bảo trì Offline.

---

### Phase 5 — Hoàn thiện REST API, MCP Parity, Migrate & i18n

*Mục tiêu: Đồng bộ hoàn toàn cho tích hợp bên ngoài, AI agents và di chuyển dữ liệu từ MantisBT PHP.*

1. **REST API Parity với MantisBT 2.28** ([`src/controllers/api.rs`](../src/controllers/api.rs))
   - Mở rộng `/api/rest/issues/:id`: thêm `files` (upload/download/delete), `tags`, `monitors`, `relationships`, `DELETE /notes/:note_id`; hỗ trợ `custom_fields`, `tags`, `handler` khi create/patch và `filter_id`/`select` khi query.
   - Bổ sung CRUD cho `/api/rest/projects` (kèm `versions`, `subprojects`, `users`, `handlers`), `/api/rest/users` (kèm `reset`, `tokens`), `/api/rest/filters`, `/api/rest/config`.
   - Hỗ trợ `ETag` / `If-None-Match` và cung cấp file OpenAPI spec.
2. **Nâng cấp MCP Server** ([`src/mcp/tools.rs`](../src/mcp/tools.rs))
   - Bổ sung `tags`, `relationships`, `custom_fields`, `attachments` vào kết quả của `get_issue` (hiện mô tả tool có nhắc `tags` nhưng payload chưa trả về).
   - Thêm tools cho quản lý tags, relationships, filters và metadata file đính kèm.
3. **XML Import/Export & Đa ngôn ngữ (i18n)**
   - Hỗ trợ xuất/nhập issue theo chuẩn XML (`mantis.dtd`) tương thích plugin `XmlImportExport` của MantisBT gốc để dễ migrate dữ liệu.
   - Bổ sung hệ thống i18n gọn nhẹ (trước mắt tiếng Anh + tiếng Việt) và hiển thị thời gian theo `timezone` của user thay vì cố định UTC.

---

## 5. Ngoài phạm vi lộ trình (Out of Scope / Non-Goals)

- **SOAP API (`mantisconnect.php`):** Công nghệ cũ; REST API và MCP Server đã thay thế hiệu quả hơn.
- **Hệ thống Plugin PHP động:** Không phù hợp với kiến trúc single-binary của Rust; thay vào đó nên tích hợp sẵn dưới dạng module bật/tắt qua trait event hooks nội bộ (Formatting, Graphs, XML Import/Export, Gravatar).
- **Sponsorships & Legacy Wiki adapters (TWiki, WackoWiki, WikkaWiki...):** Mức độ sử dụng thực tế rất thấp so với chi phí bảo trì.
- **Đa CSDL (Oracle / MS SQL / MySQL via ADOdb):** Chỉ nên cân nhắc thêm PostgreSQL về sau nếu có nhu cầu scale vượt SQLite.
