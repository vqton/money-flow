# Task Backlog & Execution Specification: `money-flow`

**Tracking Standard**: Sized for single pull-request (PR) units. TDD Test-First specification for all financial logic.  
**Traceability**: Every task links directly to an identified Gap (`G#`) and regulatory citation (`[VERIFIED]` or `[SECONDARY]`).  
**Updated**: 2026-11-23  

---

## 1. Backlog Task Matrix

| Task ID | Title | Gap | Regulation Citation | Phase | Type | Est. | Owner | Module Impacted | Depends On | Status |
| :--- | :--- | :---: | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| **P0-01** | Audit Route Auth & Restrict `/audit-logs` | G9, G12 | TT 99/2025 Điều 3 k2 `[VERIFIED]` | 0 | Fix | **S** | Backend | `handlers.rs`, `openapi.rs` | None | **READY** |
| **P0-02** | Upgrade `rusqlite` & Verify SQLite Version $\ge 3.53$ | G11 | WAL-Reset Resilience `[VERIFIED]` | 0 | Ops | **M** | Backend | `backend/Cargo.toml` | None | **READY** |
| **P0-03** | Implement Automated Disaster Restore Logic & CLI | G10 | Luật Kế toán 2015 Điều 18 `[VERIFIED]` | 0 | Feature | **M** | Backend | `database.rs` *(Hotspot)* | None | **READY** |
| **P0-04** | Automated Zero-Diff Disaster Recovery Drill Suite | G10 | Luật Kế toán 2015 Điều 18 `[VERIFIED]` | 0 | Test | **S** | Backend | `tests/api_tests.rs` | P0-03 | **READY** |
| **P0-05** | Repository Hygiene & `.gitignore` Coverage | G16 | Engineering Standard `[VERIFIED]` | 0 | Ops | **S** | Backend | `.gitignore` | None | **READY** |
| **P0-06** | Commit Formal `LICENSE` File & SPDX Header | G18 | Open Source / Commercial Law | 0 | Doc | **S** | PO | Root `LICENSE` | None | **READY** |
| **P1-01** | Dynamic Chart of Accounts Entity & Schema | G3 | TT 99/2025 Phụ lục II `[VERIFIED]` | 1 | Feature | **M** | Backend | `database.rs`, `models.rs` | P0-02 | **READY** |
| **P1-02** | Enforce Active Account Validation in `validate_voucher` | G3 | TT 99/2025 Phụ lục II `[VERIFIED]` | 1 | Feature | **S** | Backend | `ledger.rs`, `handlers.rs` | P1-01 | **READY** |
| **P1-03** | Configurable Fiscal Year Start Date | G4 | TT 99/2025 Điều 31 `[VERIFIED]` | 1 | Feature | **S** | Backend | `config.rs`, `database.rs` | None | **READY** |
| **P1-04** | Voucher Approval Workflow (`DRAFT` $\rightarrow$ `POSTED`) | G2 | TT 99/2025 Điều 3 k2 `[VERIFIED]` | 1 | Feature | **M** | Backend | `database.rs`, `dto.rs` | P1-02 | **READY** |
| **P1-05** | Segregation of Duties (Creator Cannot Approve) | G2 | TT 99/2025 Điều 3 k2 `[VERIFIED]` | 1 | Feature | **S** | Backend | `handlers.rs`, `auth.rs` | P1-04 | **READY** |
| **P1-06** | Supporting Document Attachments with SHA-256 Hash | G5 | TT 99/2025 Phụ lục I `[VERIFIED]` | 1 | Feature | **M** | Backend | `database.rs`, `handlers.rs` | P1-04 | **READY** |
| **P1-07** | Rename to Financial Position & Implement B03-DN Cash Flow | G14 | TT 99/2025 Phụ lục IV `[VERIFIED]` | 1 | Feature | **M** | Backend | `database.rs`, `handlers.rs` | P1-02 | **READY** |
| **P1-08** | TT 200 $\rightarrow$ TT 99 Opening Balances Migration Protocol | G1 | TT 99/2025 Điều 31 `[VERIFIED]` | 1 | Feature | **L** | BA / Backend | `closing.rs`, `database.rs` | P1-01, P1-03 | **READY** |
| **P1-09** | Partner Subledger Dimension (`partner_id`) on TK 131/331 | G15 | TT 99/2025 Phụ lục II `[VERIFIED]` | 1 | Feature | **M** | Backend | `database.rs`, `dto.rs` | P1-01, D4 (Approved) | **READY** |
| **P2-01** | VND Currency Integer Rounding Policy Engine | G6 | QĐ 1233 trang 4 `[SECONDARY]` | 2 | Feature | **M** | Backend | `ledger.rs`, `closing.rs` | D2 (Approved) | **READY** |
| **P2-02** | E-Invoice Metadata Schema (14 Reasons, VAT Codes) | G7 | QĐ 1233 danh mục `[SECONDARY]` | 2 | Feature | **M** | Backend | `models.rs`, `dto.rs` | P1-04 | **READY** |
| **P2-03** | E-Invoice Integration Adapter (Provider REST API) | G8 | QĐ 1233 msg 200/202 `[SECONDARY]` | 2 | Feature | **L** | Backend | New `einvoice/` module | P2-01, P2-02, D1 (Approved) | **READY** |
| **P2-04** | Digital Signature (XMLDSig / XAdES) Verification Engine | G8 | QĐ 1233 chuẩn ký `[SECONDARY]` | 2 | Feature | **L** | Backend | New `crypto/` module | P2-03, D1 (Approved) | **READY** |
| **P3-01** | Windows Server Service Daemon Wrapper (`windows-service`) | G17 | Enterprise Ops `[VERIFIED]` | 3 | Ops | **M** | Ops / Backend | `backend/src/main.rs` | P1-08 | **READY** |
| **P3-02** | Disaster Recovery & Log Retention Runbook | G10, G17 | Luật Kế toán 2015 Điều 18 `[VERIFIED]` | 3 | Doc | **S** | Ops | `docs/runbooks/` | P0-04, P3-01 | **READY** |
| **P3-03** | Parallel Pilot Run Reconciling 0-VND Variance | D5 | TT 99/2025 Điều 31 `[VERIFIED]` | 3 | Ops | **L** | CFO / BA | Operations | P1-08, D5 (Approved) | **READY** |
| **P3-04** | Multi-Tenancy Architecture (Database-per-Client) | G13 | SaaS Architecture | 4 | Feature | **L** | Tech Lead | `state.rs`, `database.rs` | D3 | **DEFERRED (D3)** |

---

## 2. Detailed Task Specifications (Test-First & Acceptance Criteria)

### P0-01: Audit Route Auth & Restrict `/audit-logs`
- **Linked Gap**: G9, G12.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 3 khoản 2 `[VERIFIED]`.
- **Acceptance Criteria**:
  - `GET /api/v1/audit-logs` requires `RequireController` extractor (accessible only by `Controller` and `Admin` roles).
  - Attempting to access `/api/v1/audit-logs` as an `Accountant` returns `403 Forbidden`.
  - Unauthenticated requests return `401 Unauthorized`.
  - OpenAPI 3.1 specification updated with security requirements for `/api/v1/audit-logs`.
- **Test-First Note**:
  - `test_api_audit_logs_forbidden_for_accountant_role`: Post with JWT containing `Role::Accountant` against `/api/v1/audit-logs`, asserting `status == 403`.
- **Estimate**: S.

---

### P0-02: Upgrade `rusqlite` & Verify SQLite Version $\ge 3.53$
- **Linked Gap**: G11.
- **Regulatory Basis**: Data durability & WAL crash resilience `[VERIFIED]`.
- **Acceptance Criteria**:
  - Update `rusqlite` in `backend/Cargo.toml` to latest stable release.
  - Runtime check `SELECT sqlite_version()` asserts version meets target security and corruption-fix standards.
  - Zero regression in WAL pragmas, busy timeouts, or existing trigger executions.
- **Test-First Note**:
  - `test_sqlite_engine_version_and_wal_resilience`: Asserts `SELECT sqlite_version()` returns major/minor version $\ge$ required baseline.
- **Estimate**: M.

---

### P0-03: Implement Automated Disaster Restore Logic & CLI
- **Linked Gap**: G10.
- **Regulatory Basis**: Luật Kế toán 2015 Điều 18 (Bảo toàn dữ liệu kế toán) `[VERIFIED]`.
- **Acceptance Criteria**:
  - Implement `LedgerRepository::restore_from_snapshot(&Path) -> Result<(), AccountingError>`.
  - Expose authenticated endpoint `POST /api/v1/system/restore` restricted to `Admin` role only.
  - Implement CLI sub-command: `backend.exe restore --snapshot <path> --target <database_path>`.
  - Atomically validates snapshot schema integrity, triggers, and journal equilibrium prior to swapping active database handle.
- **Test-First Note**:
  - `test_restore_corrupt_database_rejected_cleanly`: Attempts restore of a truncated file, asserting `Err(AccountingError::DatabaseError(...))` with active database intact.
- **Estimate**: M.

---

### P0-04: Automated Zero-Diff Disaster Recovery Drill Suite
- **Linked Gap**: G10.
- **Regulatory Basis**: Luật Kế toán 2015 Điều 18 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Test populates a complete fiscal month of compound vouchers and runs 4-step closing.
  - Computes pre-backup trial balance, balance sheet, and audit log row count.
  - Executes backup, initializes fresh database, and runs restore.
  - Asserts trial balance, balance sheet, voucher counts, and audit logs match with 0-byte and 0-VND discrepancy.
- **Test-First Note**:
  - `test_e2e_backup_restore_drill_zero_diff`: Asserts `pre_tb == post_tb && pre_bs == post_bs && pre_audit_count == post_audit_count`.
- **Estimate**: S.

---

### P1-01: Dynamic Chart of Accounts Entity & Schema
- **Linked Gap**: G3.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục II `[VERIFIED]`.
- **Acceptance Criteria**:
  - Create table `accounts (code TEXT PRIMARY KEY, name TEXT NOT NULL, parent_code TEXT, class TEXT NOT NULL, is_active INTEGER NOT NULL DEFAULT 1, normal_balance TEXT NOT NULL)`.
  - Provide database seed with all statutory Circular 99 accounts (Loại 1 - 9).
  - Implement endpoints: `GET /api/v1/accounts`, `POST /api/v1/accounts` (Admin/Controller only).
- **Test-First Note**:
  - `test_seed_circular_99_chart_of_accounts_completeness`: Queries `accounts` table asserting all standard level-1 accounts (111, 112, 131, 156, 211, 331, 411, 511, 632, 911) are present and active.
- **Estimate**: M.

---

### P1-02: Enforce Active Account Validation in `validate_voucher`
- **Linked Gap**: G3.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục II `[VERIFIED]`.
- **Acceptance Criteria**:
  - `validate_voucher()` verifies every `voucher_line.account_code` against the active `accounts` registry.
  - Posting a line with an unknown account code returns `422 UNKNOWN_ACCOUNT`.
  - Posting a line with a disabled (`is_active = 0`) account code returns `422 ACCOUNT_INACTIVE`.
- **Test-First Note**:
  - `test_voucher_with_unknown_account_rejected`: Submits voucher with account code `"9999"`, asserting rejection `UNKNOWN_ACCOUNT`.
- **Estimate**: S.

---

### P1-03: Configurable Fiscal Year Start Date
- **Linked Gap**: G4.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 31, Luật Kế toán 2015 Điều 12 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Add `pub fiscal_year_start_month: u8` (default: 1) to `AppConfig` (env: `FISCAL_YEAR_START_MONTH`).
  - Period calculations and annual closing transfers accommodate fiscal years starting on month 4, 7, or 10.
- **Test-First Note**:
  - `test_non_calendar_fiscal_year_period_calculation`: Configures `fiscal_year_start_month = 4`, verifies period (2026, 1) represents April 2026.
- **Estimate**: S.

---

### P1-04: Voucher Approval Workflow (`DRAFT` $\rightarrow$ `POSTED`)
- **Linked Gap**: G2.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 3 khoản 2 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Add `status` enum variants: `DRAFT`, `APPROVED`, `POSTED`, `VOIDED`.
  - General Ledger and financial reports strictly aggregate vouchers where `status = 'POSTED'`.
  - Endpoints: `POST /api/v1/vouchers` (creates `DRAFT`), `POST /api/v1/vouchers/{id}/approve`, `POST /api/v1/vouchers/{id}/post`.
- **Test-First Note**:
  - `test_draft_voucher_excluded_from_trial_balance`: Creates a voucher in `DRAFT` state, asserts it does not appear in trial balance balances until transition to `POSTED`.
- **Estimate**: M.

---

### P1-05: Segregation of Duties (Creator Cannot Approve)
- **Linked Gap**: G2.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 3 khoản 2 (Kiểm soát nội bộ) `[VERIFIED]`.
- **Acceptance Criteria**:
  - When approving a voucher (`POST /api/v1/vouchers/{id}/approve`), the backend extracts JWT `sub`.
  - If `user.sub == voucher.created_by`, the request fails with `403 CANNOT_APPROVE_OWN_VOUCHER`.
  - Approval succeeds only when executed by a distinct authenticated actor holding `Role::Controller` or `Role::Admin`.
- **Test-First Note**:
  - `test_approval_by_voucher_creator_rejected_with_403`: Creates voucher as `"accountant_1"`, attempts approval using token for `"accountant_1"`, asserting `403 Forbidden`.
- **Estimate**: S.

---

### P1-06: Supporting Document Attachments with SHA-256 Hash
- **Linked Gap**: G5.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục I, Luật Kế toán 2015 Điều 18 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Create table `voucher_attachments (id TEXT PRIMARY KEY, voucher_id TEXT NOT NULL, filename TEXT NOT NULL, mime_type TEXT NOT NULL, sha256_hash TEXT NOT NULL, file_size INTEGER NOT NULL, uploaded_by TEXT NOT NULL, uploaded_at TEXT NOT NULL)`.
  - Endpoints to upload and inspect supporting documents linked to voucher IDs.
  - SHA-256 hash verified upon upload to guarantee tamper-evident document integrity.
- **Test-First Note**:
  - `test_voucher_attachment_integrity_hash_verification`: Uploads sample file bytes, asserts stored `sha256_hash` matches independent SHA-256 computation.
- **Estimate**: M.

---

### P1-07: Rename to Financial Position & Implement B03-DN Cash Flow
- **Linked Gap**: G14.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục IV `[VERIFIED]`.
- **Acceptance Criteria**:
  - Mount `/api/v1/reports/financial-position` returning `StatementOfFinancialPosition` (retaining `/api/v1/reports/balance-sheet` as deprecated redirect/alias).
  - Implement `/api/v1/reports/cash-flow` (B03-DN) calculating operating, investing, and financing cash flows from cash accounts (111, 112) using direct method.
- **Test-First Note**:
  - `test_cash_flow_statement_direct_method_equilibrium`: Posts cash receipts and disbursements, asserts net cash flow matches delta in accounts 111 + 112.
- **Estimate**: M.

---

### P1-08: TT 200 $\rightarrow$ TT 99 Opening Balances Migration Protocol
- **Linked Gap**: G1.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 31 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Implement migration endpoint `POST /api/v1/migration/tt200-to-tt99` accepting closing account balances under TT 200.
  - Automatically maps deprecated/renamed accounts to TT 99 accounts.
  - Inserts opening balance voucher dated day 1 of the new fiscal year.
  - Asserts that total opening debits equal total opening credits ($\sum \text{Debit} == \sum \text{Credit}$) with exact 0-VND variance before unlocking period.
- **Test-First Note**:
  - `test_tt200_to_tt99_opening_balance_reconciliation_zero_diff`: Supplies full sample TT 200 trial balance, asserts migration completes with `0.0000` discrepancy and trial balance equilibrium.
- **Estimate**: L.

---

### P1-09: Partner Subledger Dimension (`partner_id`) on TK 131/331 *(BLOCKED by D4)*
- **Linked Gap**: G15.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục II `[VERIFIED]`.
- **Acceptance Criteria**:
  - Add `partner_id TEXT` to `voucher_lines`.
  - When line `account_code` begins with `131` or `331`, `partner_id` is required.
  - Implement `/api/v1/reports/partner-ledger` returning statement of account per partner.
- **Test-First Note**:
  - `test_tk131_posting_without_partner_id_rejected`: Posts debit to `131` without `partner_id`, asserting `422 PARTNER_ID_REQUIRED`.
- **Estimate**: M.

---

### P2-01: VND Currency Integer Rounding Policy Engine *(BLOCKED by D2)*
- **Linked Gap**: G6.
- **Regulatory Basis**: QĐ 1233/QĐ-CT trang 4, NĐ 174/2016/NĐ-CP Điều 4 `[SECONDARY]`.
- **Acceptance Criteria**:
  - Implements the chosen rounding policy from Decision D2.
  - All calculated VAT totals and invoice payable totals round to integer *đồng* (zero decimal places).
  - General Ledger retains exact `rust_decimal::Decimal` fixed-point precision with automated adjustment lines if rounding produces 1-đồng balancing variance.
- **Test-First Note**:
  - `test_vnd_invoice_tax_rounding_produces_zero_decimals`: Computes VAT on fractional prices, asserts resulting tax amount has 0 decimal scale and zero fractional parts.
- **Estimate**: M.

---

### P2-02: E-Invoice Metadata Schema (14 Reasons, VAT Codes)
- **Linked Gap**: G7.
- **Regulatory Basis**: QĐ 1233/QĐ-CT danh mục thông điệp 100–999, TT 91/2026/TT-BTC `[SECONDARY]`.
- **Acceptance Criteria**:
  - Extend voucher domain entities with e-invoice fields: `invoice_serial`, `invoice_template`, `invoice_no`, `seller_tax_code`, `buyer_tax_code`.
  - Enforce enum validation for 14 statutory adjustment reason codes and 10 VAT rate codes (0%, 5%, 8%, 10%, KCT, KKKNT, KHAC, CTTC, KTPVDC, KTGTT).
- **Test-First Note**:
  - `test_einvoice_invalid_vat_code_rejected`: Submits e-invoice voucher with tax code `"15%"`, asserting `422 INVALID_VAT_RATE_CODE`.
- **Estimate**: M.

---

### P2-03: E-Invoice Integration Adapter *(BLOCKED by D1)*
- **Linked Gap**: G8.
- **Regulatory Basis**: QĐ 1233/QĐ-CT thông điệp 200/202 `[SECONDARY]`.
- **Acceptance Criteria**:
  - Implements `InvoiceTransmitter` adapter based on Decision D1.
  - Successfully transmits invoice payload to certified test environment and processes Message 202 acknowledgment.
- **Test-First Note**:
  - `test_einvoice_transmission_mock_receiver_roundtrip`: Sends invoice through adapter, asserting HTTP 200 with tax authority transmission ticket.
- **Estimate**: L.

---

### P2-04: Digital Signature (XMLDSig / XAdES) Verification Engine *(BLOCKED by D1)*
- **Linked Gap**: G8.
- **Regulatory Basis**: QĐ 1233/QĐ-CT chuẩn ký số `[SECONDARY]`.
- **Acceptance Criteria**:
  - Validates cryptographic digital signatures on incoming and outgoing electronic invoice documents against Vietnamese national root CAs.
- **Test-First Note**:
  - `test_xades_signature_validation_tampered_document_rejected`: Injects modified byte into signed invoice, asserting signature verification failure.
- **Estimate**: L.

---

### P3-01: Windows Server Service Daemon Wrapper (`windows-service`)
- **Linked Gap**: G17.
- **Regulatory Basis**: Operational standard `[VERIFIED]`.
- **Acceptance Criteria**:
  - `backend.exe` supports `--service` flag for Windows Service Control Manager (SCM) lifecycle (Start, Stop, Pause).
  - Handles graceful shutdown flushing non-blocking log appender on system stop.
- **Test-First Note**:
  - `test_cli_service_flag_registration`: Verifies CLI parser handles `--service` argument cleanly.
- **Estimate**: M.

---

### P3-02: Disaster Recovery & Log Retention Runbook
- **Linked Gap**: G10, G17.
- **Regulatory Basis**: Luật Kế toán 2015 Điều 18 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Comprehensive runbook committed to `docs/runbooks/disaster_recovery.md` documenting step-by-step restoration from snapshot, log archival policies, and emergency key rotations.
- **Estimate**: S.

---

### P3-03: Parallel Pilot Run Reconciling 0-VND Variance *(BLOCKED by D5)*
- **Linked Gap**: D5.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 31 `[VERIFIED]`.
- **Acceptance Criteria**:
  - Ingestion and parallel calculation of 1 fiscal quarter of live corporate vouchers.
  - Comparison of trial balance, net profit, and tax payable showing 0-VND variance against audited legacy records.
- **Test-First Note**:
  - `test_pilot_historical_quarter_reconciliation_zero_variance`: Validates that all account balances in pilot dataset match reference audited ledger.
- **Estimate**: L.

---

## 3. Dependency Graph & Topological Execution Order

To eliminate cyclic dependencies, tasks must execute in the following verified topological order:

$$\mathbf{P0\text{-}05} \rightarrow \mathbf{P0\text{-}06} \rightarrow \mathbf{P0\text{-}01} \rightarrow \mathbf{P0\text{-}02} \rightarrow \mathbf{P0\text{-}03} \rightarrow \mathbf{P0\text{-}04} \rightarrow \mathbf{P1\text{-}01} \rightarrow \mathbf{P1\text{-}02} \rightarrow \mathbf{P1\text{-}03} \rightarrow \mathbf{P1\text{-}04} \rightarrow \mathbf{P1\text{-}05} \rightarrow \mathbf{P1\text{-}06} \rightarrow \mathbf{P1\text{-}07} \rightarrow \mathbf{P1\text{-}08}$$

$$\text{Following Decision D4:} \quad \mathbf{P1\text{-}09}$$
$$\text{Following Decisions D1 \& D2:} \quad \mathbf{P2\text{-}01} \rightarrow \mathbf{P2\text{-}02} \rightarrow \mathbf{P2\text{-}03} \rightarrow \mathbf{P2\text{-}04}$$
$$\text{Following Decision D5:} \quad \mathbf{P3\text{-}01} \rightarrow \mathbf{P3\text{-}02} \rightarrow \mathbf{P3\text{-}03}$$
