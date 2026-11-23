# Project Decisions: `money-flow`

**Status**: Architectural Decision Register (ADR) — **APPROVED BY CFO**  
**Tracking Context**: Implementation Roadmap (TT 99/2025/TT-BTC, QĐ 1233/QĐ-CT)  
**Updated & Approved**: 2026-11-23  

---

## Decision Gate Summary

All primary blocking architectural and regulatory decisions have been formally evaluated and **APPROVED** by the Principal CFO on 2026-11-23. Downstream tasks in `TASKS.md` previously blocked are now **UNBLOCKED** and scheduled.

| ID | Decision Title | Owner | Impacted Gaps | Decision Result | Status |
| :--- | :--- | :--- | :---: | :--- | :---: |
| **D1** | E-Invoice Strategy: Provider API vs. In-House XML | PO + CFO | G8 | **Option B**: Certified Provider REST API (VNPT/Viettel/MISA) | **APPROVED** |
| **D2** | Tax & Invoice Rounding Protocol (QĐ 1233) | Chief Accountant / Tax Advisor | G6 | **Option B**: Subtotal per Tax Rate + GL Residual Adjustment | **APPROVED** |
| **D3** | Tenancy Model: Single-Tenant vs. In-Process Pool | PO + Tech Lead | G13 | **Option A**: Process/Container-per-Company for Phase 1 | **APPROVED** |
| **D4** | Subledger Partner Dimension (TK 131/331) in MVP | BA + PO | G15 | **Option A**: Include in Phase 1 MVP (`partner_id` mandatory) | **APPROVED** |
| **D5** | Pilot Scope & Parallel Run Protocol | CFO | Phase 3 | **Option A**: Single Entity Pilot with Q3/Q4 2026 Back-Testing | **APPROVED** |
| **D6** | Go-Live Target Schedule | PO | All | **2026-12-31 (Phase 1 GL Cutover) / 2026-12-30 (E-Invoice)** | **APPROVED** |

---

## Detailed Decision Records

### D1: E-Invoice Delivery Strategy (Build vs. Partner API)
- **Problem Statement**: QĐ 1233/QĐ-CT mandates e-invoice data components and message schemas (types 200, 202, 203, 300) with DSig / XAdES digital signatures by **01/01/2027**.
- **Determination**: **OPTION B (Certified E-Invoice Provider Integration via Clean Domain Adapter)**.
  - Implement a Rust domain trait `InvoiceTransmitter` in `backend/src/`.
  - Provide an initial production adapter targeting a certified Vietnamese e-invoice provider REST API (e.g., VNPT, Viettel, MISA, BKAV).
  - The provider absorbs tax authority transmission protocols, GDT test certifications, and cryptographic HSM token handshakes.
  - Eliminates the risk of missing the statutory 01/01/2027 deadline. In-house direct T-VAN XML transmission is deferred to Phase 4.
- **Approved By**: Principal CFO & PO (2026-11-23).
- **Status**: **APPROVED** (Unblocks P2-03, P2-04).

---

### D2: Rounding Method for VND Totals (QĐ 1233 / NĐ 174)
- **Problem Statement**: QĐ 1233/QĐ-CT (trang 4) mandates that for VND transactions, all invoice totals (taxable amount, VAT amount, total payable) must be rounded to the nearest integer (*đồng*), with zero fractional decimals.
- **Determination**: **OPTION B WITH GL RESIDUAL ADJUSTMENT (Subtotal Grouped Rounding per VAT Rate)**.
  - Group voucher lines by statutory VAT rate (0%, 5%, 8%, 10%).
  - Compute VAT on the exact Decimal taxable subtotal per rate.
  - Round subtotal VAT to the nearest integer *đồng* using standard half-up arithmetic ($\ge 0.5$ rounds up, $< 0.5$ rounds down per NĐ 174 Điều 4 khoản 5).
  - Double-entry ledger equilibrium strictly holds ($\sum \text{Debits} == \sum \text{Credits}$). Any $\pm 1$ đồng rounding discrepancy between subtotal tax and individual line sums is balanced automatically through the primary tax account or general ledger adjustment (`811` / `711`).
- **Approved By**: Principal CFO & Chief Accountant (2026-11-23).
- **Status**: **APPROVED** (Unblocks P2-01).

---

### D3: Tenancy Architecture (Multi-Tenancy Model)
- **Problem Statement**: Corporate clients require strict data isolation across subsidiary companies.
- **Determination**: **OPTION A (Single-Tenant Isolated Process/Container per Company)**.
  - Run one `backend.exe` container/process per legal company, mapped to its own isolated SQLite database (`data/{company_tax_code}/accounting.db`).
  - Guarantees 100% physical and cryptographic data segregation during statutory audits.
  - Defer in-process multi-tenant dynamic connection pooling to Phase 4 (SaaS expansion).
- **Approved By**: Principal CFO & Tech Lead (2026-11-23).
- **Status**: **APPROVED**.

---

### D4: Subledger Partner Dimension (TK 131 / TK 331) in Phase 1 MVP
- **Problem Statement**: Circular 99/2025/TT-BTC mandates partner-level accounting (*đối tượng công nợ*) for accounts receivable (TK 131) and accounts payable (TK 331).
- **Determination**: **OPTION A (Include in Phase 1 MVP)**.
  - Add `partner_id TEXT` (Tax Code/MST or Partner Code) and `partner_name TEXT` to `voucher_lines` table.
  - In `validate_voucher()`, require `partner_id` whenever `account_code` begins with `131` or `331` (rejecting with `422 PARTNER_ID_REQUIRED` if omitted).
  - Prevents breaking database migrations after initial go-live.
- **Approved By**: Principal CFO & BA Lead (2026-11-23).
- **Status**: **APPROVED** (Unblocks P1-09 for Sprint 2).

---

### D5: Pilot Scope & Parallel Run Protocol
- **Problem Statement**: Audit-grade verification requires proving zero discrepancies against legacy software prior to decommissioning.
- **Determination**: **OPTION A (Single Operating Entity Pilot with Historical Back-Testing)**.
  - Ingest and post audited historical transactions from Q3 and Q4 2026 for 1 representative enterprise subsidiary.
  - Run 4-step closing and compare Trial Balance, Statement of Financial Position, and Income Statement against reference audited statements.
  - Require a signed **Zero-Variance Reconciliation Certificate** by the Chief Accountant before final production cutover.
- **Approved By**: Principal CFO (2026-11-23).
- **Status**: **APPROVED** (Unblocks P3-03).

---

### D6: Target Go-Live Schedule vs. Regulatory Deadlines
- **Problem Statement**: Align delivery with TT 99/2025/TT-BTC (effective 01/01/2026) and QĐ 1233 e-invoice deadline (01/01/2027).
- **Determination**: **TARGET 2026-12-31 FOR GL CUTOVER; 2026-12-30 FOR E-INVOICE TEST CERTIFICATION**.
  - Phase 1 General Ledger cutover scheduled for **2026-12-31**.
  - Phase 2 E-Invoice provider integration certified in test environment by **2026-12-30**.
- **Approved By**: Principal CFO & PO (2026-11-23).
- **Status**: **APPROVED**.
