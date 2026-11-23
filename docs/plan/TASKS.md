# Task Backlog & Execution Specification: `money-flow`

**Tracking Standard**: Sized for single pull-request (PR) units. TDD Test-First specification for all financial logic.  
**Traceability**: Every task links directly to an identified Gap (`G#`) and regulatory citation (`[VERIFIED]` or `[SECONDARY]`).  
**Updated**: 2026-11-23  

---

## 1. Backlog Task Matrix

| Task ID | Title | Gap | Regulation Citation | Phase | Type | Est. | Owner | Module Impacted | Depends On | Status |
| :--- | :--- | :---: | :--- | :---: | :---: | :---: | :---: | :--- | :--- | :---: |
| **P0-01** | Audit Route Auth & Restrict `/audit-logs` | G9, G12 | TT 99/2025 Điều 3 k2 `[VERIFIED]` | 0 | Fix | **S** | Backend | `handlers/`, `auth.rs` | None | **DONE** |
| **P0-02** | Upgrade `rusqlite` & Verify SQLite Version $\ge 3.53$ | G11 | WAL-Reset Resilience `[VERIFIED]` | 0 | Ops | **M** | Backend | `backend/Cargo.toml` | None | **DONE** |
| **P0-03** | Implement Automated Disaster Restore Logic & CLI | G10 | Luật Kế toán 2015 Điều 18 `[VERIFIED]` | 0 | Feature | **M** | Backend | `db/disaster.rs`, `main.rs` | None | **DONE** |
| **P0-04** | Automated Zero-Diff Disaster Recovery Drill Suite | G10 | Luật Kế toán 2015 Điều 18 `[VERIFIED]` | 0 | Test | **S** | Backend | `tests/api_tests.rs` | P0-03 | **DONE** |
| **P0-05** | Repository Hygiene & `.gitignore` Coverage | G16 | Engineering Standard `[VERIFIED]` | 0 | Ops | **S** | Backend | `.gitignore` | None | **DONE** |
| **P0-06** | Commit Formal `LICENSE` File & SPDX Header | G18 | Open Source / Commercial Law | 0 | Doc | **S** | PO | Root `LICENSE` | None | **DONE** |
| **P1-01** | Dynamic Chart of Accounts Entity & Schema | G3 | TT 99/2025 Phụ lục II `[VERIFIED]` | 1 | Feature | **M** | Backend | `db/account_repo.rs`, `catalog/` | P0-02 | **DONE** |
| **P1-02** | Enforce Active Account Validation in `validate_voucher` | G3 | TT 99/2025 Phụ lục II `[VERIFIED]` | 1 | Feature | **S** | Backend | `ledger.rs`, `handlers/` | P1-01 | **DONE** |
| **P1-03** | Configurable Fiscal Year Start Month | G4 | TT 99/2025 Điều 31 `[VERIFIED]` | 1 | Feature | **S** | Backend | `config.rs`, `closing.rs` | None | **DONE** |
| **P1-04** | Voucher Approval Workflow (`DRAFT` $\rightarrow$ `POSTED`) | G2 | TT 99/2025 Điều 3 k2 `[VERIFIED]` | 1 | Feature | **M** | Backend | `db/voucher_repo.rs`, `dto/` | P1-02 | **SCHEDULED** |
| **P1-05** | Segregation of Duties (Creator Cannot Approve) | G2 | TT 99/2025 Điều 3 k2 `[VERIFIED]` | 1 | Feature | **S** | Backend | `handlers/voucher.rs`, `auth.rs` | P1-04 | **SCHEDULED** |
| **P1-06** | Supporting Document Attachments with SHA-256 Hash | G5 | TT 99/2025 Phụ lục I `[VERIFIED]` | 1 | Feature | **M** | Backend | `db/schema.rs`, `handlers/` | P1-04 | **SCHEDULED** |
| **P1-07** | Statement of Financial Position & B03 Cash Flow Report | G14 | TT 99/2025 Phụ lục IV `[VERIFIED]` | 1 | Feature | **M** | Backend | `db/report_repo.rs`, `handlers/` | P1-02 | **SCHEDULED** |
| **P1-08** | TT 200 $\rightarrow$ TT 99 Opening Balances Migration Tool | G1 | TT 99/2025 Điều 31 `[VERIFIED]` | 1 | Feature | **L** | BA / Backend | `closing.rs`, `db/` | P1-01, P1-03 | **SCHEDULED** |
| **P1-09** | Partner Subledger Dimension (`partner_id`) on TK 131/331 | G15 | TT 99/2025 Phụ lục II `[VERIFIED]` | 1 | Feature | **M** | Backend | `db/schema.rs`, `dto/` | P1-01, D4 (Approved) | **SCHEDULED** |
| **P2-01** | VND Currency Integer Rounding Policy Engine | G6 | QĐ 1233 trang 4 `[SECONDARY]` | 2 | Feature | **M** | Backend | `ledger.rs`, `closing.rs` | D2 (Approved) | **SCHEDULED** |
| **P2-02** | E-Invoice Metadata Schema (14 Reasons, VAT Codes) | G7 | QĐ 1233 danh mục `[SECONDARY]` | 2 | Feature | **M** | Backend | `models.rs`, `dto/` | P1-04 | **SCHEDULED** |
| **P2-03** | E-Invoice Provider REST Adapter (Certified API) | G8 | QĐ 1233 msg 200/202 `[SECONDARY]` | 2 | Feature | **L** | Backend | New `backend/src/einvoice/` | P2-01, P2-02, D1 (Approved) | **SCHEDULED** |
| **P2-04** | Digital Signature (XMLDSig / XAdES) Verification Engine | G8 | QĐ 1233 chuẩn ký `[SECONDARY]` | 2 | Feature | **L** | Backend | New `backend/src/crypto/` | P2-03, D1 (Approved) | **SCHEDULED** |
| **P3-01** | Windows Server Service Daemon Wrapper (`windows-service`) | G17 | Enterprise Ops `[VERIFIED]` | 3 | Ops | **M** | Ops / Backend | `backend/src/main.rs` | P1-08 | **SCHEDULED** |
| **P3-02** | Disaster Recovery & Log Retention Runbook | G10, G17 | Luật Kế toán 2015 Điều 18 `[VERIFIED]` | 3 | Doc | **S** | Ops | `docs/runbooks/` | P0-04, P3-01 | **SCHEDULED** |
| **P3-03** | Parallel Pilot Run Reconciling 0-VND Variance | D5 | TT 99/2025 Điều 31 `[VERIFIED]` | 3 | Ops | **L** | CFO / BA | Operations | P1-08, D5 (Approved) | **SCHEDULED** |
| **P3-04** | Multi-Tenancy Architecture (Database-per-Client) | G13 | SaaS Architecture | 4 | Feature | **L** | Tech Lead | `state.rs`, `db/` | D3 | **DEFERRED (D3)** |

---

## 2. Detailed Task Specifications (Test-First & Acceptance Criteria)

### P1-04: Voucher Approval Workflow (`DRAFT` $\rightarrow$ `POSTED`)
- **Linked Gap**: G2.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 3 khoản 2 `[VERIFIED]`.
- **Phase**: 1. **Type**: Feature. **Estimate**: M. **Owner**: Backend.
- **Module Impacted**: `db/voucher_repo.rs`, `dto/requests.rs`, `handlers/voucher.rs`.
- **Depends On**: P1-02.
- **Acceptance Criteria**:
  - `POST /api/v1/vouchers` accepts `status: "DRAFT"` or defaults to draft.
  - Vouchers with status `DRAFT` are saved without impacting general ledger account balances or financial reports (`trial-balance`, `financial-position`, `income-statement`).
  - New endpoint `POST /api/v1/vouchers/{id}/approve` transitions status from `DRAFT` to `POSTED`.
  - Once `POSTED`, the voucher immediately reflects in trial balance calculations.
- **Test-First Note**:
  - `test_draft_voucher_does_not_affect_trial_balance`: Create draft voucher for 50,000,000 VND; fetch trial balance and assert total debits remain 0.
  - `test_approve_voucher_transitions_to_posted_and_updates_balance`: Approve draft; assert status is `POSTED` and trial balance reflects debits.

---

### P1-05: Segregation of Duties (Creator Cannot Approve)
- **Linked Gap**: G2.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 3 khoản 2 `[VERIFIED]`.
- **Phase**: 1. **Type**: Feature. **Estimate**: S. **Owner**: Backend.
- **Module Impacted**: `handlers/voucher.rs`, `auth.rs`.
- **Depends On**: P1-04.
- **Acceptance Criteria**:
  - `POST /api/v1/vouchers/{id}/approve` checks whether JWT `sub` equals `vouchers.created_by`.
  - If approver is the creator, request fails with `403 Forbidden` (`CANNOT_APPROVE_OWN_VOUCHER`).
  - Approval requires role `Controller` or `Admin`.
- **Test-First Note**:
  - `test_creator_cannot_approve_own_voucher`: User A creates draft voucher; User A attempts approval; assert `403 Forbidden` with error code `CANNOT_APPROVE_OWN_VOUCHER`.

---

### P1-06: Supporting Document Attachments with SHA-256 Hash
- **Linked Gap**: G5.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục I `[VERIFIED]`, Luật Kế toán 2015 Điều 18 `[VERIFIED]`.
- **Phase**: 1. **Type**: Feature. **Estimate**: M. **Owner**: Backend.
- **Module Impacted**: `db/schema.rs`, `db/voucher_repo.rs`, `handlers/voucher.rs`.
- **Depends On**: P1-04.
- **Acceptance Criteria**:
  - Create table `voucher_attachments (id TEXT PRIMARY KEY, voucher_id TEXT, file_name TEXT, file_size INTEGER, sha256_hash TEXT NOT NULL, uploaded_at TEXT, uploaded_by TEXT)`.
  - Endpoint `POST /api/v1/vouchers/{id}/attachments` uploads file metadata and verified SHA-256 hash.
  - Attachments on closed periods or voided vouchers are rejected with `422 Unprocessable Entity`.
- **Test-First Note**:
  - `test_voucher_attachment_sha256_hash_integrity`: Post attachment metadata with hash; verify retrieval returns exact hash and links to voucher.

---

### P1-07: Statement of Financial Position & B03 Cash Flow Report
- **Linked Gap**: G14.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục IV `[VERIFIED]`.
- **Phase**: 1. **Type**: Feature. **Estimate**: M. **Owner**: Backend.
- **Module Impacted**: `db/report_repo.rs`, `handlers/reports_period.rs`, `openapi/paths_reports.rs`.
- **Depends On**: P1-02.
- **Acceptance Criteria**:
  - Expose `/api/v1/reports/financial-position` complying with B01-DN (replaces Balance Sheet terminology).
  - Implement `/api/v1/reports/cash-flow` generating B03-DN (Direct method cash flow from operating, investing, and financing activities: TK 111, 112).
- **Test-First Note**:
  - `test_cash_flow_b03_direct_method_equilibrium`: Post operating revenue cash collection and vendor cash payment; assert net cash flow equals cash account delta.

---

### P1-08: TT 200 $\rightarrow$ TT 99 Opening Balances Migration Tool
- **Linked Gap**: G1.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 31 `[VERIFIED]`.
- **Phase**: 1. **Type**: Feature. **Estimate**: L. **Owner**: BA / Backend.
- **Module Impacted**: `closing.rs`, `db/voucher_repo.rs`.
- **Depends On**: P1-01, P1-03.
- **Acceptance Criteria**:
  - `POST /api/v1/migration/opening-balances` accepts account balance array from Circular 200.
  - Automatically maps deprecated accounts to Circular 99 equivalents.
  - Asserts $\sum \text{Opening Debits} == \sum \text{Opening Credits}$; rejects migration with `422 RECONCILIATION_DIFFERENCE` if variance $\neq 0$.
  - Atomic posting of initial opening voucher dated to first day of fiscal year.
- **Test-First Note**:
  - `test_opening_balances_zero_difference_reconciliation`: Post balanced TT 200 trial balance; assert successful migration voucher created with 0 variance.
  - `test_opening_balances_unbalanced_rejected`: Post unbalanced trial balance; assert `422` error.

---

### P1-09: Partner Subledger Dimension (`partner_id`) on TK 131/331
- **Linked Gap**: G15.
- **Regulatory Basis**: TT 99/2025/TT-BTC Phụ lục II `[VERIFIED]`.
- **Phase**: 1. **Type**: Feature. **Estimate**: M. **Owner**: Backend.
- **Module Impacted**: `db/schema.rs`, `db/voucher_repo.rs`, `dto/requests.rs`.
- **Depends On**: P1-01, Decision D4 (Approved).
- **Acceptance Criteria**:
  - Add `partner_id TEXT` to `voucher_lines` schema.
  - When line item references accounts starting with `131` or `331`, `partner_id` is mandatory.
  - Endpoint `GET /api/v1/reports/partner-ledger` filters balances by `partner_id`.
- **Test-First Note**:
  - `test_posting_to_131_without_partner_id_rejected`: Post voucher debiting 131 without `partner_id`; assert `422 UNPROCESSABLE_ENTITY` with `PARTNER_ID_REQUIRED`.

---

### P2-01: VND Currency Integer Rounding Policy Engine
- **Linked Gap**: G6.
- **Regulatory Basis**: QĐ 1233 trang 4 `[SECONDARY]`, NĐ 174/2016 Điều 4 khoản 5 `[VERIFIED]`.
- **Phase**: 2. **Type**: Feature. **Estimate**: M. **Owner**: Backend.
- **Module Impacted**: `ledger.rs`, `closing.rs`.
- **Depends On**: Decision D2 (Approved).
- **Acceptance Criteria**:
  - Implement rounding engine per Decision D2: group lines by VAT rate, calculate VAT, half-up round to nearest integer *đồng*.
  - Internal ledger maintains exact `rust_decimal::Decimal`. Any $\pm 1$ đồng discrepancy between subtotal tax and line sum is adjusted to TK 711 or 811.
- **Test-First Note**:
  - `test_qd1233_subtotal_vat_half_up_integer_rounding`: Calculate VAT for fractional subtotal 123,456.50 VND; assert exact 123,457 VND with 0 decimals.

---

### P2-02: E-Invoice Metadata Schema
- **Linked Gap**: G7.
- **Regulatory Basis**: QĐ 1233/QĐ-CT danh mục mã `[SECONDARY]`.
- **Phase**: 2. **Type**: Feature. **Estimate**: M. **Owner**: Backend.
- **Module Impacted**: `models.rs`, `dto/requests.rs`.
- **Depends On**: P1-04.
- **Acceptance Criteria**:
  - Add models for invoice serial (`C26T`), invoice number, buyer/seller tax code, 14 adjustment reason codes, currency code (VND + 20 foreign currencies), and VAT rate enum (`0`, `5`, `8`, `10`, `KCT`, `KKKNT`, `KHAC`, `CTTC`, `KTPVDC`, `KTGTT`).
- **Test-First Note**:
  - `test_einvoice_metadata_validation_valid_vat_rates`: Validate all 10 statutory VAT rate codes serialize and parse correctly.

---

### P2-03: E-Invoice Provider REST Adapter (Certified API)
- **Linked Gap**: G8.
- **Regulatory Basis**: QĐ 1233/QĐ-CT msg 200/202 `[SECONDARY]`, NĐ 254/2026 `[SECONDARY]`.
- **Phase**: 2. **Type**: Feature. **Estimate**: L. **Owner**: Backend.
- **Module Impacted**: `backend/src/einvoice/`.
- **Depends On**: P2-01, P2-02, Decision D1 (Approved).
- **Acceptance Criteria**:
  - Implement trait `InvoiceTransmitter` in Rust.
  - Build HTTP REST client adapter targeting certified provider sandbox (e.g., VNPT/Viettel/MISA).
  - Map voucher data to provider payload; transmit; handle response containing CQT invoice code and reservation number.
- **Test-First Note**:
  - `test_einvoice_adapter_mock_transmission_success`: Submit invoice through adapter against mock wiremock server; assert returns valid invoice code and status 200.

---

### P2-04: Digital Signature (XMLDSig / XAdES) Verification Engine
- **Linked Gap**: G8.
- **Regulatory Basis**: QĐ 1233/QĐ-CT chuẩn ký số `[SECONDARY]`.
- **Phase**: 2. **Type**: Feature. **Estimate**: L. **Owner**: Backend.
- **Module Impacted**: `backend/src/crypto/`.
- **Depends On**: P2-03, Decision D1 (Approved).
- **Acceptance Criteria**:
  - Verify signature envelope and digest on incoming/outgoing e-invoice XML payloads.
  - Assert cryptographic validity against provider root certificate.
- **Test-First Note**:
  - `test_verify_xades_xml_signature_valid`: Verify signed XML fixture; assert signature status `VALID`.

---

### P3-01: Windows Server Service Daemon Wrapper
- **Linked Gap**: G17.
- **Regulatory Basis**: Enterprise Operational Standards `[VERIFIED]`.
- **Phase**: 3. **Type**: Ops. **Estimate**: M. **Owner**: Ops / Backend.
- **Module Impacted**: `backend/src/main.rs`.
- **Depends On**: P1-08.
- **Acceptance Criteria**:
  - Add Windows Service control dispatcher via `windows-service` crate.
  - Supports `install`, `start`, `stop`, `uninstall` service subcommands on Windows Server 2022/2025.
- **Test-First Note**:
  - `test_service_dispatcher_startup_flags`: Verify `--service` flag initializes service control handler.

---

### P3-02: Disaster Recovery & Log Retention Runbook
- **Linked Gap**: G10, G17.
- **Regulatory Basis**: Luật Kế toán 2015 Điều 18 `[VERIFIED]`.
- **Phase**: 3. **Type**: Doc. **Estimate**: S. **Owner**: Ops.
- **Module Impacted**: `docs/runbooks/DR_RUNBOOK.md`.
- **Depends On**: P0-04, P3-01.
- **Acceptance Criteria**:
  - Step-by-step operational runbook detailing cold-start restore, live restore API invocation, log archive rotation, and 10-year statutory retention schedule.

---

### P3-03: Parallel Pilot Run Reconciling 0-VND Variance
- **Linked Gap**: D5.
- **Regulatory Basis**: TT 99/2025/TT-BTC Điều 31 `[VERIFIED]`.
- **Phase**: 3. **Type**: Ops. **Estimate**: L. **Owner**: CFO / BA.
- **Module Impacted**: Operations / Test fixture verification.
- **Depends On**: P1-08, Decision D5 (Approved).
- **Acceptance Criteria**:
  - Ingest 500+ production vouchers from legacy system for Q3/Q4 2026.
  - Compute trial balance in `money-flow` and compare against legacy audited statements.
  - Discrepancy on all accounts must be exactly 0 VND.
- **Test-First Note**:
  - `test_pilot_historical_data_zero_variance`: Load Q4 2026 batch; assert balance delta between legacy and `money-flow` is `Decimal::ZERO`.

---

## 3. Dependency Topological Ordering

The task execution graph contains zero circular dependencies. The valid linear dependency order is:

```
[P0-01..P0-06] (Completed) -> [P1-01, P1-02, P1-03] (Completed)
  -> P1-04 (Approval Workflow)
  -> P1-05 (Segregation of Duties)
  -> P1-06 (Attachments & SHA-256)
  -> P1-07 (B01/B03 Reports)
  -> P1-09 (Partner Subledger 131/331)
  -> P1-08 (Opening Balances Migration)
  -> P2-01 (VND Rounding Engine)
  -> P2-02 (E-Invoice Metadata)
  -> P2-03 (Provider REST Adapter)
  -> P2-04 (XMLDSig/XAdES Engine)
  -> P3-01 (Windows Server Daemon)
  -> P3-02 (DR Runbooks)
  -> P3-03 (Parallel Pilot Run)
```
