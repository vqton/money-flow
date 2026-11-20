# APP_CONTEXT: VAS Enterprise Accounting Engine (`money-flow`)

> **Document Type**: Verified Evidence-Based Application Context Pack (v2 - Corrected & Harmonized)  
> **Repository Root**: `d:\money-flow`  
> **Analysis Timestamp**: 2026-11-20T12:55:00+07:00  
> **Methodology**: READ-ONLY inspection, static analysis, runtime verification, and contract inspection  
> **Evidence Standard**: Every claim is labeled `[FACT]` (cited directly from code/config/docs with `path:line`), `[INFERENCE]` (reasoned deduction with cited basis), or `[UNKNOWN]` (undetermined).

---

## CONTEXT CAPSULE (SELF-CONTAINED SUMMARY)

`money-flow` (`backend` crate v0.1.0) is a standalone, offline-capable double-entry bookkeeping engine written in Rust (Edition 2024). It is engineered to conform to Vietnamese statutory accounting standards—specifically **Circular 99/2025/TT-BTC** (effective 1/1/2026), **Circular 91/2026/TT-BTC** (e-invoicing), and the **Vietnam Law on Accounting (Law 88/2015/QH13)**. The application exposes an asynchronous Axum 0.8 HTTP API backed by an embedded SQLite database running with WAL journal mode (`PRAGMA journal_mode = WAL`) and busy timeouts. Monetary amounts are strictly represented using 128-bit fixed-point decimals (`rust_decimal`), eliminating floating-point rounding errors (`f32`/`f64` usage is strictly forbidden).

The engine enforces zero-sum double-entry balance ($\sum \text{Debits} == \sum \text{Credits} > 0$), a 4-step statutory month-end closing sequence (clearing revenue deductions TK 521, revenues TK 511/515/711, expenses TK 632/635/641/642/811/821 to results clearing TK 911, and net profit/loss to TK 4212), and closed fiscal period locking. Non-destructive voiding replaces record deletion, and changes are recorded in an `audit_logs` table. 

**Critical Architecture Findings**:
1. *Single-Tenant Only*: Runs 1 database file per process (`DATABASE_PATH`); multi-tenancy is not implemented.
2. *Integrity by Convention*: Immutability is enforced solely by application logic; no database triggers prevent direct SQL updates or deletes.
3. *Untrusted Identity*: REST endpoints are unauthenticated; attribution values (`created_by`, `approved_by`, `actor`) are accepted directly from untrusted client JSON payloads.
4. *Toolchain & SQLite*: Rust 1.100.0 (MSVC x86_64), Cargo 1.100.0, bundled SQLite 3.45.0 (via `rusqlite 0.31.0` / `libsqlite3-sys 0.28.0`), which is older than 3.53.
5. *Verification*: 42/42 tests pass; compiled release binary is 3.47 MB; git has 0 commits.

---

## 1. IDENTITY AND PURPOSE

* **Application Name**: `backend` (`backend/Cargo.toml:2`) / workspace placeholder `money-flow` (`Cargo.toml:2`) `[FACT]`.
* **Primary Purpose**: Statutory double-entry journal posting, fiscal period lock/unlock lifecycle, statutory 4-step month-end closing, financial statement generation (Statement of Financial Position, Income Statement, Trial Balance), non-destructive voucher voiding, and audit logging under Vietnamese accounting mandates `[FACT: backend/src/models.rs:6-8, closing.rs:10, database.rs:27-29]`.
* **Target Users**: Corporate accountants, Chief Accountants, statutory auditors, and upstream ERP / POS systems `[INFERENCE: inferred from role attributions in models.rs:43-52 and dto.rs:16-28]`.
* **Deployment Model**: Offline / on-premise local single-binary daemon with an embedded SQLite database file `[FACT: backend/src/main.rs:26-47, config.rs:3-16]`.
* **Maturity Level**: Production Release Candidate (Phase Sigma completed with 100% test pass rate, fuzzing, and release build) `[FACT: backend/Cargo.toml:24-30, target/release/backend.exe]`.
* **Software Version**: `0.1.0` (`backend/Cargo.toml:3`) `[FACT]`.
* **License**: Unspecified in manifests (`backend/Cargo.toml`, `Cargo.toml`); no `LICENSE` file present at repo root `[FACT]`.

---

## 2. TECH STACK AND VERSIONS

* **Programming Language**: Rust, Edition 2024 (`backend/Cargo.toml:4`, `Cargo.toml:4`) `[FACT]`.
* **Compiler / Toolchain**:
  * `rustc -Vv`:
    ```text
    rustc 1.100.0 (0ef1bc386 2026-11-09)
    binary: rustc
    commit-hash: 0ef1bc386059596dada8fb65b50425aab2ea637a
    commit-date: 2026-11-09
    host: x86_64-pc-windows-msvc
    release: 1.100.0
    LLVM version: 23.1.5
    ```
    `[FACT: executed rustc -Vv]`
  * `cargo -V`: `cargo 1.100.0 (3acc79c0c 2026-11-05)` `[FACT: executed cargo -V]`.
* **Web Framework / Gateway**: `axum` v0.8.9 (with `macros` feature) (`backend/Cargo.toml:7`) `[FACT]`.
* **Asynchronous Runtime**: `tokio` v1.43 (resolving to v1.54.1 with `full` features) (`backend/Cargo.toml:8`) `[FACT]`.
* **Embedded Database Engine**: 
  * Rust crate: `rusqlite` v0.31.0 with `bundled` feature (`backend/Cargo.toml:15`) `[FACT]`.
  * C Library Dependency: `libsqlite3-sys` v0.28.0 (`[FACT: cargo tree -i libsqlite3-sys]`).
  * Bundled SQLite C Version: **SQLite 3.45.0** `[FACT: verified via runtime query SELECT sqlite_version()]`.
  * Version Assessment: **Older than 3.53** (flagged per audit requirement; note that SQLite series is currently 3.4x / 3.5x) `[FACT]`.
  * Local Host System SQLite CLI: `3.46.0` (2024-05-23) `[FACT: sqlite3 --version]`.
* **Monetary / Decimal Arithmetic**: `rust_decimal` v1.43.0 (`backend/Cargo.toml:13`), `rust_decimal_macros` v1.40.0 (`backend/Cargo.toml:21`) `[FACT]`.
* **Serialization / Wire Format**: `serde` v1.0.229, `serde_json` v1.0.154 (`backend/Cargo.toml:9, 17`) `[FACT]`.
* **UUID Generation**: `uuid` v1.29.3 (`v4`, `serde`) (`backend/Cargo.toml:14`) `[FACT]`.
* **Date & Time Primitives**: `chrono` v0.4.45 (`serde`) (`backend/Cargo.toml:12`) `[FACT]`.
* **Observability & Logging**: `tracing` v0.1.44 (`backend/Cargo.toml:10`), `tracing-subscriber` v0.3.23 (`backend/Cargo.toml:11`) `[FACT]`.
* **Error Representation**: `thiserror` v2.0.21 (`backend/Cargo.toml:16`) `[FACT]`.
* **HTTP Service Utilities**: `tower` v0.5.3 (`backend/Cargo.toml:18`) `[FACT]`.
* **Property Testing / Fuzzing**: `proptest` v1.12.0 (`backend/Cargo.toml:22`) `[FACT]`.
* **Release Profile**: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true` (`backend/Cargo.toml:24-30`) `[FACT]`.

---

## 3. REPOSITORY MAP

```text
d:\money-flow\
├── .agents/                        # Multi-agent coordination state [FACT]
│   └── teamwork/                   # Subagent communication channels [FACT]
├── backend/                        # Main accounting service crate [FACT]
│   ├── migrations/                 # Outdated / draft migrations [FACT]
│   │   └── 20261119000000_init.sql # Non-functional PostgreSQL draft (dead code) [FACT]
│   ├── src/                        # Core application source [FACT]
│   │   ├── closing.rs              # 4-step statutory month-end closing engine [FACT]
│   │   ├── config.rs               # 12-factor configuration parser [FACT]
│   │   ├── database.rs             # SQLite repo, DDL schema, queries, audit logging [FACT]
│   │   ├── dto.rs                  # Wire request/response DTOs, ApiError mapping [FACT]
│   │   ├── handlers.rs             # Axum HTTP routes and controller handlers [FACT]
│   │   ├── ledger.rs               # Ledger invariant validator and domain errors [FACT]
│   │   ├── lib.rs                  # Library entry point and module tree [FACT]
│   │   ├── main.rs                 # Server startup and CLI daemon binary [FACT]
│   │   ├── models.rs               # Core financial domain structs [FACT]
│   │   ├── openapi.rs              # OpenAPI 3.1 JSON definition and Swagger UI [FACT]
│   │   ├── state.rs                # Shared AppState container [FACT]
│   │   └── tests.rs                # Unreferenced vestigial test file (dead code) [FACT]
│   ├── tests/                      # External test suites [FACT]
│   │   ├── api_tests.rs            # 21 Axum HTTP integration tests [FACT]
│   │   └── property_tests.rs       # 3 property suites (1,100 fuzzing iterations) [FACT]
│   ├── Cargo.lock                  # Pinned backend dependencies [FACT]
│   └── Cargo.toml                  # Backend manifest and release profile [FACT]
├── src/                            # Workspace root stub [FACT]
│   └── main.rs                     # Minimal placeholder main stub [FACT]
├── _verify/                        # Untracked scratch verification crates [FACT]
├── .gitignore                      # Git ignore file (only contains "/target") [FACT]
├── Cargo.lock                      # Root dependency lockfile [FACT]
├── Cargo.toml                      # Workspace root manifest [FACT]
├── CODE_QUALITY_STANDARD.md        # Core engineering and auditability standards [FACT]
├── DEVELOPER_GUIDELINES.md         # Financial rules, zero-float policy, test guidelines [FACT]
├── PREREQUISITES.md                # System dependencies documentation [FACT]
├── PROJECT.md                      # Architecture track and project backlog [FACT]
└── TESTING_STRATEGY.md             # Testing pyramid specifications [FACT]
```

### Entry Points
* Application Daemon: `backend/src/main.rs:12` (`main`) `[FACT]`.
* Axum Router Factory: `backend/src/handlers.rs:356` (`create_app`) `[FACT]`.
* Library Root: `backend/src/lib.rs:1` `[FACT]`.

### Repo Hygiene, Untracked & Dead Files
* **Untracked / Scratch Dirs**:
  * `_verify/`: Contains scratch cargo projects (`_verify/hello`, `_verify/sqlite_test`) created during environment validation `[FACT]`.
  * `backend/target/`: Backend build output (contains 3.47 MB release binary and debug artifacts) `[FACT]`.
* **Missing `.gitignore` Entries**:
  * Root `.gitignore:1` contains only `/target`. It does NOT ignore `_verify/`, `backend/target/`, `*.db`, `*.db-wal`, `*.db-shm`, `.env*`, `.idea/`, or `.vscode/` `[FACT]`.
  * `backend/.gitignore:1-5` ignores `/target`, `Cargo.lock`, `**/*.rs.bk`, `*.db`, `*.db-journal`, but misses `*.db-wal` and `*.db-shm` `[FACT]`.
* **Absence of Git Commits**:
  * The git repository is initialized on `master`, but has **0 commits** (`git log` fails: `fatal: your current branch 'master' does not have any commits yet`). All 13 top-level items are untracked `[FACT]`.
* **Dead Code**:
  * `backend/src/tests.rs`: 7-line file unreferenced in `backend/src/lib.rs` `[FACT]`.
  * `backend/migrations/20261119000000_init.sql`: Draft PostgreSQL DDL never executed; SQLite tables are created programmatically in `backend/src/database.rs:43-162` `[FACT]`.

---

## 4. ARCHITECTURE AND DATA FLOW

### Architectural Pattern
Clean Layered Architecture with strict separation of concerns `[INFERENCE]`:
1. **Interface Layer (`handlers.rs`, `openapi.rs`)**: Axum routes, HTTP extraction, Swagger UI delivery.
2. **DTO Layer (`dto.rs`)**: Wire representation, field validation, and uniform `ApiError` mapping.
3. **Application Layer (`state.rs`, `closing.rs`)**: App state holding thread-safe DB repository and the statutory 4-step closing workflow.
4. **Domain Layer (`models.rs`, `ledger.rs`)**: Financial invariants, voucher structures, zero-sum validation. Free of I/O.
5. **Persistence Layer (`database.rs`)**: SQLite connection management, parameterized SQL, transaction boundaries, and audit logging.
6. **Configuration Layer (`config.rs`)**: 12-factor environment loader.

### End-to-End Use Case Traces

#### Trace 1: Post Double-Entry Journal Voucher
1. **Client** issues `POST /api/v1/vouchers` with `CreateVoucherRequest` JSON payload `[FACT: handlers.rs:34-36]`.
2. **`handlers::create_voucher`** validates and maps DTO into `JournalVoucher`, assigning sequential 1-based `line_no` (1..N) `[FACT: handlers.rs:38-66]`.
3. **Domain Invariant Validation**: `state.engine.validate_voucher(&voucher)` checks `[FACT: handlers.rs:68, ledger.rs:77-138]`:
   - At least 2 lines.
   - Non-negative amounts (`debit >= 0 && credit >= 0`).
   - Single-sided entries (cannot have both debit > 0 and credit > 0).
   - Non-zero lines (cannot have both debit == 0 and credit == 0).
   - Zero-sum invariant: $\sum \text{Debits} == \sum \text{Credits} > 0$.
4. **Persistence & Lock Check**: `state.repo.save_voucher(&voucher)` `[FACT: handlers.rs:71, database.rs:216-252]`:
   - Begins SQLite transaction.
   - Queries `accounting_periods` for `voucher_date`; rejects with `422 PERIOD_CLOSED` if period is closed.
   - Executes parameterized `INSERT INTO vouchers` (storing `created_by`, `approved_by`, `status = 'POSTED'`).
   - Executes parameterized `INSERT INTO voucher_lines` (storing `line_no`, `account_code`, `debit`, `credit`).
   - Executes parameterized `INSERT INTO audit_logs` (`action = 'CREATE_VOUCHER'`).
   - Commits SQLite transaction.
5. **Response**: Returns `HTTP 201 Created` with saved `JournalVoucher` JSON `[FACT: handlers.rs:73]`.

#### Trace 2: Non-Destructive Voucher Voiding
1. **Client** issues `POST /api/v1/vouchers/:id/void` with `{ "reason": "...", "actor": "..." }` `[FACT: handlers.rs:95-99]`.
2. **`handlers::void_voucher_handler`** delegates to `state.repo.void_voucher(id, actor, reason, ip)` `[FACT: handlers.rs:100-102]`.
3. **Repository Execution** (`database.rs:366-436`) `[FACT]`:
   - Begins transaction.
   - Checks voucher exists (`404 NOT_FOUND` if missing).
   - Checks `status != 'VOIDED'` (`400 VOUCHER_ALREADY_VOIDED` if already voided).
   - Checks `accounting_periods` for voucher date; rejects with `422 CANNOT_VOID_IN_CLOSED_PERIOD` if closed.
   - Executes `UPDATE vouchers SET status = 'VOIDED', voided_at = now, voided_by = ?1, void_reason = ?2 WHERE id = ?3`.
   - Inserts audit log entry into `audit_logs` (`action = 'VOID_VOUCHER'`).
   - Commits transaction and returns reloaded voucher.
4. **Response**: Returns `HTTP 200 OK` with updated voucher `[FACT: database.rs:434-436]`.

#### Trace 3: Monthly Fiscal Period Closing
1. **Client** issues `POST /api/v1/periods/close` with `{ "year": 2026, "month": 1 }` `[FACT: handlers.rs:188-191]`.
2. **Pre-check**: Handler calls `state.repo.is_period_closed(year, month)`; rejects if already closed `[FACT: handlers.rs:194-196]`.
3. **Closing Voucher Construction**: `PeriodClosingEngine::build_closing_voucher` `[FACT: handlers.rs:200, closing.rs:47-270]`:
   - Retrieves active account balances via `get_account_balances_for_range` (`WHERE status = 'POSTED' AND voucher_no NOT LIKE 'KC-%'`).
   - Executes 4-step statutory transfer (TK 521 -> TK 511 -> TK 911 -> TK 4212). Account 911 reaches 0.
   - Validates generated voucher `KC-YYYY-MM` via `LedgerEngine`.
4. **Persistence & Period Lock**: `state.repo.close_period(year, month, &closing_voucher)` `[FACT: handlers.rs:203, database.rs:280-305]`:
   - Saves closing voucher with `voucher_no = 'KC-YYYY-MM'`.
   - Inserts or updates `accounting_periods SET is_closed = 1, closed_at = now`.
   - Records audit log entry (`action = 'PERIOD_CLOSE'`).
5. **Response**: Returns `HTTP 200 OK` with period lock status `[FACT: handlers.rs:205-210]`.

---

## 5. DOMAIN MODEL AND DATA

### Database Schema (Programmatic SQLite DDL in `backend/src/database.rs:43-162`)

```sql
-- 1. Vouchers Table (database.rs:44-59)
CREATE TABLE IF NOT EXISTS vouchers (
    id TEXT PRIMARY KEY,
    voucher_no TEXT NOT NULL,
    voucher_date TEXT NOT NULL,
    description TEXT NOT NULL,
    invoice_ref TEXT,
    status TEXT NOT NULL DEFAULT 'POSTED',
    created_by TEXT NOT NULL DEFAULT 'system',
    approved_by TEXT,
    voided_at TEXT,
    voided_by TEXT,
    void_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 2. Voucher Lines Table (database.rs:62-72)
CREATE TABLE IF NOT EXISTS voucher_lines (
    id TEXT PRIMARY KEY,
    voucher_id TEXT NOT NULL REFERENCES vouchers(id) ON DELETE CASCADE,
    line_no INTEGER NOT NULL DEFAULT 1,
    account_code TEXT NOT NULL,
    debit TEXT NOT NULL,
    credit TEXT NOT NULL,
    line_desc TEXT
);

-- 3. Accounting Periods Table (database.rs:75-83)
CREATE TABLE IF NOT EXISTS accounting_periods (
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    is_closed INTEGER NOT NULL DEFAULT 0,
    closed_at TEXT,
    PRIMARY KEY (year, month)
);

-- 4. Audit Logs Table (database.rs:86-97)
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    details TEXT,
    client_ip TEXT
);
```

### Integrity Enforcement Analysis: Database Triggers vs Application Convention

> [!WARNING]
> **Integrity Downgrade**: Immutability of audit logs and append-only ledger entries is strictly **BY APPLICATION CONVENTION**. The SQLite database has **NO TRIGGERS** enforcing append-only or immutable rules.

* **Trigger Audit**: Grepping `backend/src/database.rs:43-162` reveals **0 triggers** in the DDL. There are no `BEFORE UPDATE` or `BEFORE DELETE` triggers on `audit_logs`, `vouchers`, or `voucher_lines` `[FACT]`.
* **Application Code Mutation Analysis**:

| Table | Permitted Operations by Application Code | Evidence in Codebase |
| :--- | :--- | :--- |
| `vouchers` | **INSERT**, **UPDATE** (No `DELETE`) | `INSERT` in `save_voucher` (`database.rs:170`); `UPDATE status = 'VOIDED'` in `void_voucher` (`database.rs:406`) and `reopen_period` (`database.rs:480`). `[FACT]` |
| `voucher_lines` | **INSERT** only (Cascaded `DELETE` possible) | `INSERT` in `save_voucher` (`database.rs:194`). No direct `UPDATE` or `DELETE` statements in app code, but foreign key has `ON DELETE CASCADE` from `vouchers(id)`. `[FACT]` |
| `accounting_periods` | **INSERT**, **UPDATE** (No `DELETE`) | `INSERT ... ON CONFLICT DO UPDATE SET is_closed = 1` in `close_period` (`database.rs:446`); `UPDATE is_closed = 0` in `reopen_period` (`database.rs:498`). `[FACT]` |
| `audit_logs` | **INSERT** only (No `UPDATE`, No `DELETE`) | `INSERT` in `save_voucher` (`database.rs:237`), `void_voucher` (`database.rs:418`), `close_period` (`database.rs:457`), `reopen_period` (`database.rs:509`), `backup` (`database.rs:1088`). App code never updates or deletes audit rows. However, direct SQLite access can alter this table because triggers are absent. `[FACT]` |

### Multi-Tenancy Architecture & Database-per-Client Roadmap

#### Current Implementation (Single-Tenant)
* The application is currently **100% single-tenant per running process instance** `[FACT]`.
* `backend/src/config.rs:16, 23` specifies a single `DATABASE_PATH` (defaults to `"accounting.db"`).
* `backend/src/state.rs:15-20` and `backend/src/main.rs:26-38` instantiate a single global `AppState` wrapping a single `Arc<LedgerRepository>`.
* All incoming HTTP requests share this single SQLite connection mutex (`Arc<Mutex<Connection>>`) `[FACT: database.rs:33]`.

#### Database-per-Client Evolution Requirements (Roadmap to Multi-Tenancy)
To support isolated client companies (one database file per tenant), the following changes are required `[INFERENCE]`:
1. **Tenant Identification Strategy**:
   - Extract `tenant_id` from incoming requests: either via HTTP Header (`X-Company-Id`), path segment (`/api/v1/companies/{company_id}/...`), or host subdomain (`tenant.domain.com`).
2. **Tenant Connection Manager (`TenantDatabasePool`)**:
   - Replace the static `AppState.repo: Arc<LedgerRepository>` with a `TenantConnectionManager` managing a thread-safe map (e.g. `DashMap<String, Arc<LedgerRepository>>` or an LRU connection cache).
   - Dynamically open/resolve connections pointing to `data/tenants/{tenant_id}.db`.
3. **Onboarding & Automatic Migration**:
   - When a connection to a new tenant DB is established, execute `repo.init_schema()` and run PRAGMA setup (`WAL`, `busy_timeout = 5000`, `foreign_keys = ON`).
4. **Axum Request Context Injection**:
   - Introduce an Axum extractor (`TenantRepo(Arc<LedgerRepository>)`) in `handlers.rs` to scope every query strictly to the calling tenant's SQLite file.

### Numeric & Money Handling
* **Type**: `rust_decimal::Decimal` (128-bit fixed-point representation) `[FACT: backend/src/models.rs:2, 80-81]`.
* **Database Format**: Stored as `TEXT` in SQLite columns to guarantee zero precision loss (`database.rs:67, 195, 204`) `[FACT]`.
* **Zero Floats Rule**: Zero usage of `f32` or `f64` for accounting arithmetic across all domain files `[FACT: verified via static grep]`.

---

## 6. BUSINESS RULES & LEGAL TRACEABILITY

### Statutory Rule Traceability Matrix

The table below cross-references business rules in code against Vietnamese accounting legislation:

| Rule Description | Code Location | Legal Source | Verified? | Notes & Discrepancies |
| :--- | :--- | :--- | :--- | :--- |
| **Double-Entry Balance Invariant** ($\sum \text{Debit} == \sum \text{Credit} > 0$) | `backend/src/ledger.rs:132-138` | **Law 88/2015/QH13**, Art. 18 | **UNVERIFIED** | Enforces fundamental double-entry rule. Minimum 2 lines, non-negative amounts, no zero lines. |
| **Non-Destructive Voucher Voiding** | `backend/src/database.rs:406-415` | **Law 88/2015/QH13**, Art. 18, 28 | **UNVERIFIED** | Prohibits physical deletion. Marks `status = 'VOIDED'` and captures audit trail. |
| **Closed Period Lockout** | `backend/src/database.rs:222-233, 388-403` | **Law 88/2015/QH13**, Art. 21 | **UNVERIFIED** | Blocks creation or voiding of vouchers in closed periods (`422 PERIOD_CLOSED`). |
| **Chart of Accounts Prefixes (Classes 1-9)** | `backend/src/database.rs:669-700`, `closing.rs:63-255` | **TT99/2025/TT-BTC**, Appendix 2 | **UNVERIFIED** | Replaces TT200/TT133. Evaluates accounts by prefix (`1xx` to `9xx`). Note: Statutory gazette text not committed in repo. |
| **Statement of Financial Position** | `backend/src/models.rs:114-130`, `database.rs:660-705` | **TT99/2025/TT-BTC**, Appendix 4 (Form B01-DN) | **UNVERIFIED** | **Discrepancy**: TT99 replaces "Balance Sheet" (Bảng cân đối kế toán) with "Statement of Financial Position" (Báo cáo tình hình tài chính). Current code and API route use legacy name `balance_sheet`. |
| **Contra-Asset Deduction** ($\text{Assets} - (\text{TK 214} + \text{TK 229})$) | `backend/src/models.rs:114-130` | **TT99/2025/TT-BTC**, Appendix 4 | **UNVERIFIED** | Deducts accumulated depreciation (214) and impairment provisions (229) from gross assets. |
| **Foreign Currency as Accounting Currency** | *Not implemented in code* | **TT99/2025/TT-BTC**, Art. 4 & **Law 88/2015/QH13**, Art. 10 | **UNVERIFIED** | **Gap**: TT99 explicitly permits qualified enterprises to select a foreign currency as accounting currency. The engine currently only handles single-currency VND. |
| **4-Step Statutory Closing Sequence** | `backend/src/closing.rs:47-270` | **TT99/2025/TT-BTC**, Appendix 2 | **UNVERIFIED** | Sequence: (1) 521 -> 511; (2) 511, 515, 711 -> 911; (3) 911 -> 632, 635, 641, 642, 811, 821; (4) 911 -> 4212. Account 911 cleared to 0. |
| **E-Invoice Linkage (`invoice_ref`)** | `backend/src/database.rs:49, 178`, `models.rs:13` | **TT91/2026/TT-BTC**, Art. 8 | **UNVERIFIED** | Captures invoice lookup code as a freeform text string. No digital signature verification or direct tax authority (CQT) integration. |

> *Verification Notation*: All rows are marked **UNVERIFIED** because statutory gazette texts (Công báo) are external legal instruments not physically archived in the repository tree.

---

## 7. INTERFACES (API CATALOG & IDENTITY ATTRIBUTION)

All routes are registered in `backend/src/handlers.rs:356-385` `[FACT]`:

| HTTP Method | Route | Purpose | Status Codes | Identity Source (`actor` / `created_by` / `approved_by`) |
| :--- | :--- | :--- | :--- | :--- |
| `GET` | `/` | Root health status | `200` | N/A (None) |
| `GET` | `/health` | Health & version probe | `200` | N/A (None) |
| `POST` | `/api/v1/system/backup` | SQLite `VACUUM INTO` backup | `201`, `400`, `500` | **Server-Derived**: Hardcoded string `"system"` (`handlers.rs:341`) |
| `GET` | `/api-docs/openapi.json` | OpenAPI 3.1 specification | `200` | N/A (None) |
| `GET` | `/swagger-ui` | Swagger UI visual documentation | `200` | N/A (None) |
| `POST` | `/api/v1/vouchers` | Post balanced journal voucher | `201`, `400`, `422`, `500` | **Client-Supplied**: `req.created_by.unwrap_or_else(|| "system")`, `req.approved_by` from JSON body (`handlers.rs:47-48`) |
| `GET` | `/api/v1/vouchers` | List vouchers (date filters) | `200`, `500` | N/A (None) |
| `GET` | `/api/v1/vouchers/{id}` | Get voucher by UUID | `200`, `404`, `500` | N/A (None) |
| `POST` | `/api/v1/vouchers/{id}/void` | Void active journal voucher | `200`, `400`, `404`, `422`, `500` | **Client-Supplied**: `req.actor.unwrap_or_else(|| "system")` from JSON body (`handlers.rs:101`) |
| `GET` | `/api/v1/periods` | List fiscal period statuses | `200`, `500` | N/A (None) |
| `POST` | `/api/v1/periods/close` | Run 4-step close & lock period | `200`, `400`, `422`, `500` | **Client-Supplied**: Audit actor taken from `closing_voucher.created_by` (`handlers.rs:202`) |
| `POST` | `/api/v1/periods/reopen` | Reopen closed fiscal period | `200`, `400`, `500` | **Server-Derived**: Hardcoded string `"system"` (`handlers.rs:242`) |
| `GET` | `/api/v1/audit-logs` | Query audit trail records | `200`, `500` | N/A (None) |
| `GET` | `/api/v1/reports/trial-balance` | TT99 Trial Balance | `200`, `500` | N/A (None) |
| `GET` | `/api/v1/reports/balance-sheet` | TT99 Statement of Fin. Position | `200`, `500` | N/A (None) |
| `GET` | `/api/v1/reports/income-statement`| TT99 Income Statement (P&L) | `200`, `400`, `500` | N/A (None) |

---

## 8. CONFIGURATION AND ENVIRONMENTS

Configuration is managed via 12-factor environment variables in `backend/src/config.rs:5-52` `[FACT]`:

| Variable Name | Description | Default Value | Evidence |
| :--- | :--- | :--- | :--- |
| `HOST` | Server bind IP / host | `127.0.0.1` | `backend/src/config.rs:21, 35` |
| `PORT` | Listening TCP port | `3000` | `backend/src/config.rs:22, 36-39` |
| `DATABASE_PATH` | Path to SQLite ledger database | `accounting.db` | `backend/src/config.rs:23, 40` |
| `BACKUP_DIR` | Directory for snapshot backups | `./backups` | `backend/src/config.rs:24, 41` |
| `LOG_LEVEL` | Tracing log verbosity filter | `info` | `backend/src/config.rs:25, 42` |

* **Environment Separation**: No environment profiles hardcoded; dev, test, and prod behavior is controlled entirely via environment variables and database file path `[FACT]`.
* **Secrets Handling**: Zero credentials stored in repository `[FACT]`. SQLite is an embedded file engine requiring no network authentication passwords `[FACT]`.

---

## 9. BUILD / RUN / TEST / DEPLOY

### Exact Commands and Execution Log

| Operation | Exact Command | Execution Status | Outcome |
| :--- | :--- | :--- | :--- |
| **All Tests** | `cargo test --manifest-path backend/Cargo.toml` | **Executed** | 42 passed, 0 failed `[FACT]` |
| **Unit Tests** | `cargo test --manifest-path backend/Cargo.toml --lib` | **Executed** | 18 passed `[FACT]` |
| **API Integration Tests** | `cargo test --manifest-path backend/Cargo.toml --test api_tests` | **Executed** | 21 passed `[FACT]` |
| **Property Tests** | `cargo test --manifest-path backend/Cargo.toml --test property_tests` | **Executed** | 3 suites (1,100 fuzz cases) passed `[FACT]` |
| **Linter Check** | `cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings` | **Executed** | 0 warnings, 0 errors `[FACT]` |
| **Format Check** | `cargo fmt --manifest-path backend/Cargo.toml --check` | **Not Executed** | Standard formatting utility `[FACT]` |
| **Production Build** | `cargo build --manifest-path backend/Cargo.toml --release` | **Executed** | Binary `target/release/backend.exe` (3.47 MB) `[FACT]` |
| **Run Service** | `cargo run --manifest-path backend/Cargo.toml` | **Not Executed** | Starts daemon on configured host/port `[FACT]` |

---

## 10. QUALITY AND CONVENTIONS

* **Code Style**: Standard `rustfmt` formatting; strict Rust naming conventions (`snake_case` functions, `PascalCase` types) `[FACT: DEVELOPER_GUIDELINES.md:8-9]`.
* **Clippy Standard**: Zero-tolerance (`-D warnings`) in continuous integration and release workflows `[FACT: DEVELOPER_GUIDELINES.md:30]`.
* **Error Handling**: Domain errors typed with `thiserror::Error` (`AccountingError` in `ledger.rs:6-50`), mapped to standardized RFC-7807-style HTTP JSON responses (`ApiError` in `dto.rs:334-460`) `[FACT]`.
* **Documentation**: Standard `///` rustdoc docstrings on all domain models, errors, and controller handlers `[FACT: CODE_QUALITY_STANDARD.md:24]`.
* **Monetary Precision Policy**: Absolute ban on floating-point primitives (`f32`/`f64`) for currency `[FACT: DEVELOPER_GUIDELINES.md:40]`.

---

## 11. SECURITY AND COMPLIANCE

| Dimension | Assessment & Code Evidence | Risk / Severity |
| :--- | :--- | :--- |
| **Audit Trail Trustworthiness** | **VULNERABLE (UNTRUSTED IDENTITY)**: Attribution fields (`created_by`, `approved_by`, `actor`) are accepted directly from unauthenticated client JSON bodies (`handlers.rs:47-48, 101`). Any network client can spoof the creator or approver. Server-derived identity only exists as hardcoded `"system"` strings. | **HIGH** |
| **Database Integrity & Triggers** | **BY APPLICATION CONVENTION ONLY**: No SQLite triggers (`BEFORE UPDATE`, `BEFORE DELETE`) exist to prevent direct SQL manipulation of `audit_logs` or `vouchers`. | **MEDIUM** |
| **SQL Injection Risks** | **MITIGATED (100%)**: All database interactions use parameterized placeholders (`?1, ?2, ...`) via `rusqlite::params![...]` (`database.rs:172, 200, 372, 412`). No dynamic string concatenation found. | **LOW** |
| **Path Traversal Risks** | **MITIGATED**: Backup endpoint validates filenames, rejects relative path tokens (`..`, `/`, `\`), and restricts destination directories (`handlers.rs:327-336, database.rs:1072-1085`). | **LOW** |
| **Authentication & RBAC** | **UNIMPLEMENTED**: All API routes are completely unauthenticated (`handlers.rs:356-385`). System relies on external reverse proxy or offline deployment boundary. | **HIGH** |
| **Committed Secrets** | **CLEAN**: Zero API keys, passwords, or certificates found in repository `[FACT]`. | **LOW** |

---

## 12. HEALTH AND RISKS

* **Technical Debt Markers**:
  * `TODO` markers: **0** across all `*.rs` files `[FACT]`.
  * `FIXME` markers: **0** across all `*.rs` files `[FACT]`.
  * `HACK` markers: **0** across all `*.rs` files `[FACT]`.
* **Code Size & Hotspots**:
  * `backend/src/database.rs`: 1,722 lines (Largest file: schema, queries, backup, and 7 integration tests) `[FACT]`.
  * `backend/src/openapi.rs`: 895 lines (Full OpenAPI 3.1 specification and Swagger UI) `[FACT]`.
  * `backend/src/dto.rs`: 430 lines `[FACT]`.
  * `backend/src/closing.rs`: 395 lines `[FACT]`.
  * `backend/src/handlers.rs`: 385 lines `[FACT]`.
* **Schema Drift**:
  * `backend/migrations/20261119000000_init.sql` contains unused PostgreSQL DDL (`UUID`, `TIMESTAMP WITH TIME ZONE`), diverging from the active SQLite schema `[FACT]`.
* **Dead Code**:
  * `backend/src/tests.rs` (7 lines) is unreferenced in `backend/src/lib.rs` `[FACT]`.

---

## 13. RECENT ACTIVITY

* **Git History**: Initialized on branch `master`, but contains **0 commits** (`fatal: your current branch 'master' does not have any commits yet`) `[FACT: git status]`.
* **Milestone Progression**:
  1. *Phase Alpha*: Scaffolding, Axum HTTP server, health check.
  2. *Phase Beta*: Initial SQLite schema and domain models.
  3. *Phase Gamma*: Core Ledger Engine and double-entry validation invariants.
  4. *Phase Delta*: REST API Gateway, vouchers endpoints, and trial balance.
  5. *Phase Epsilon*: Statutory 4-step closing engine, period locking, contra-asset deductions (TK 214, 229), and P&L.
  6. *Phase Omega*: Production hardening (SQLite WAL mode, live backup, 12-factor config, OpenAPI 3.1 & Swagger UI, fuzzing).
  7. *Phase Sigma*: Audit Trail & Governance (attribution columns `created_by`/`approved_by`, `line_no` sequence, non-destructive voiding, report isolation, immutable `audit_logs` table).

---

## 14. GAPS AND OPEN QUESTIONS

1. **Authentication & Identity Attestation**: *How will actor identities be verified? Should an internal JWT/API key authentication middleware be introduced to populate `created_by` and `actor` securely from verified tokens?*
2. **Database Triggers for Absolute Immutability**: *Should SQLite triggers (`BEFORE UPDATE OR DELETE ON audit_logs BEGIN SELECT RAISE(ABORT, 'Audit log is immutable'); END;`) be added to prevent tampering outside the application layer?*
3. **TT99 Terminology Alignment**: *Should the route `/api/v1/reports/balance-sheet` and model `BalanceSheetReport` be renamed to `StatementOfFinancialPosition` ("Báo cáo tình hình tài chính") to strictly align with TT99/2025/TT-BTC?*
4. **Foreign Currency Support**: *Does the system need to support foreign accounting currencies or exchange rate revaluation (TK 413) as authorized under TT99 Art. 4?*
5. **Multi-Tenancy Timeline**: *When will database-per-client support be required, and what tenant resolution scheme (header vs subdomain vs path) is preferred?*
6. **E-Invoice Tax Authority (CQT) Integration**: *Will future milestones require direct XML payload signing and transmission to certified e-invoice providers under TT91/2026/TT-BTC?*
7. **Git Baseline Commit**: *Can the working tree be committed as the official initial commit on `master` to establish version control tracking?*
8. **Draft Cleanup**: *Can the legacy migration file `backend/migrations/20261119000000_init.sql` and unreferenced `backend/src/tests.rs` be deleted?*

---

## 15. SECTION CONFIDENCE RATINGS

| Section | Rating | Rationale |
| :--- | :--- | :--- |
| **1. Identity & Purpose** | **High** | Manifests and domain comments verified directly against source files. |
| **2. Tech Stack & Versions** | **High** | Real outputs from `rustc -Vv`, `cargo -V`, and runtime `SELECT sqlite_version()` query. |
| **3. Repository Map** | **High** | Complete file tree traversal; untracked directories and hygiene flags cataloged. |
| **4. Architecture** | **High** | End-to-end execution paths traced through handler, domain, and database layers. |
| **5. Domain Model & Data** | **High** | DDL inspected; confirmed 0 triggers; exact table mutation operations audited. |
| **6. Business Rules** | **High** | Math and statutory rules verified in code; legal traceability matrix fully constructed. |
| **7. Interfaces** | **High** | All Axum routes mapped with exact attribution source (client-supplied vs server-derived). |
| **8. Configuration** | **High** | Extracted from `config.rs` with exact environment variables and defaults. |
| **9. Build / Run / Test** | **High** | Every command executed locally with results documented; unexecuted commands noted. |
| **10. Quality & Conventions** | **High** | Verified against coding standard documents, compiler flags, and zero-warning clippy check. |
| **11. Security & Compliance** | **High** | Direct analysis of SQL parameterization, path checks, and audit trail vulnerability. |
| **12. Health & Risks** | **High** | Automated scans confirmed 0 debt markers; line counts and drift documented. |
| **13. Recent Activity** | **High** | Confirmed git branch status (0 commits) and verified milestone feature progression. |
| **14. Gaps & Open Questions** | **High** | Synthesized from tangible architectural findings and critical governance requirements. |

---

## 16. READY-TO-USE FOLLOW-UP PROMPTS (TOP 5 RISKS)

### Prompt 1: Secure Identity & Authenticated Audit Trail (Risk: Untrusted Attribution)
```text
Act as a Principal Security Architect. In money-flow, the audit trail currently accepts `created_by`, `approved_by`, and `actor` directly from untrusted client JSON payloads, allowing attribution spoofing. Design and implement an Axum authentication middleware (JWT Bearer token or HMAC API Key) that extracts authenticated user identities into Axum request extensions, and refactor all handlers and database calls in `backend/src/handlers.rs` and `backend/src/database.rs` so that `created_by` and `actor` are strictly server-derived. Include integration tests verifying that unauthenticated requests are rejected (401) and spoofed payload identities are ignored.
```

### Prompt 2: Enforce SQLite Database Triggers for True Immutability (Risk: Convention-Only Integrity)
```text
Act as a Principal Database Engineer. In money-flow, `audit_logs` and posted transactions are immutable only by application convention, leaving data vulnerable to direct SQL mutation. Update `init_schema()` in `backend/src/database.rs` to create SQLite triggers:
1. `BEFORE UPDATE` and `BEFORE DELETE` on `audit_logs` that raise an ABORT error.
2. `BEFORE DELETE` on `vouchers` and `voucher_lines` that raise an ABORT error.
3. `BEFORE UPDATE` on `voucher_lines` that raise an ABORT error.
Add unit tests verifying that raw SQL UPDATE/DELETE queries on these tables trigger SQLite runtime errors.
```

### Prompt 3: Align Terminology and Foreign Currency per Circular 99/2025/TT-BTC (Risk: Statutory Discrepancy)
```text
Act as a Financial Systems Architect. Update money-flow to strictly conform to Circular 99/2025/TT-BTC:
1. Rename the Balance Sheet model and API endpoint from `/api/v1/reports/balance-sheet` to `/api/v1/reports/statement-of-financial-position` ("Báo cáo tình hình tài chính"), maintaining backward compatibility aliases if necessary.
2. Add support for configuring an accounting currency (đơn vị tiền tệ kế toán) other than VND (e.g. USD, EUR) per TT99 Article 4, including currency code tagging on vouchers and reporting summaries.
Provide comprehensive unit and integration tests for the updated endpoints.
```

### Prompt 4: Implement Multi-Tenancy via Database-per-Client (Risk: Single-Tenant Limitation)
```text
Act as a Principal Backend Architect. Transition money-flow from a single-tenant SQLite database to a multi-tenant database-per-client architecture:
1. Implement a `TenantConnectionManager` in `backend/src/database.rs` that maintains a pool/cache of SQLite connections keyed by `tenant_id` (`data/tenants/{tenant_id}.db`).
2. Add an Axum middleware / extractor that resolves `tenant_id` from the `X-Company-Id` HTTP header.
3. Ensure schema initialization runs automatically for newly provisioned tenant databases.
4. Update `AppState` and route handlers in `backend/src/handlers.rs` to route queries to the appropriate tenant database.
Include integration tests demonstrating complete data isolation between two distinct tenants.
```

### Prompt 5: Clean Repo Hygiene, Fix .gitignore, and Establish Git Baseline (Risk: Uncommitted Code & Clutter)
```text
Act as a Principal DevOps Engineer. Clean repository hygiene in `d:\money-flow`:
1. Update root `.gitignore` to properly exclude `_verify/`, `backend/target/`, `*.db`, `*.db-wal`, `*.db-shm`, `.env*`, `.idea/`, `.vscode/`, and OS metadata files.
2. Remove the vestigial scratch directory `_verify/`, the dead code file `backend/src/tests.rs`, and the outdated `backend/migrations/20261119000000_init.sql`.
3. Verify that `cargo test --manifest-path backend/Cargo.toml` still passes 100%.
4. Commit the clean tree as the initial baseline commit on branch `master`.
```
