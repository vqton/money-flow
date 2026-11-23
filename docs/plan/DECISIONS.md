# Architectural & Statutory Decisions: `money-flow`

**Status**: Architectural Decision Register (ADR) — Open vs Approved Tracking  
**Standard**: Blocking decisions D1–D6 must be explicitly resolved before dependent tasks can be scheduled.  
**Updated**: 2026-11-23  

---

## 1. Decision Status Overview

| ID | Title | Owner | Impacted Gaps | Status | Decision Summary & Constraints |
| :--- | :--- | :--- | :---: | :---: | :--- |
| **D1** | E-Invoice Strategy: Provider API vs In-House | PO + CFO | G8 | **APPROVED** | **Option B (Certified Provider API)**: Integrate certified provider REST API (VNPT/Viettel/MISA) via Rust domain adapter. In-house direct T-VAN XML deferred. |
| **D2** | Tax & Invoice Rounding Protocol (QĐ 1233) | Chief Accountant / Tax Advisor | G6 | **APPROVED** | **Option B (Subtotal Grouped Rounding + GL Balancing)**: Calculate VAT on taxable subtotal per rate, half-up round to nearest đồng, auto-adjust $\pm 1$ đồng to GL (711/811). Ledger Decimal remains exact. |
| **D3** | Tenancy Model: Single-Tenant vs Pool | PO + Tech Lead | G13 | **APPROVED** | **Option A (Container/Process-per-Company)**: One `backend.exe` process per company with isolated SQLite DB (`data/{tax_code}/accounting.db`). Multi-tenant pool deferred. |
| **D4** | Partner Subledger Dimension in MVP | BA + PO | G15 | **APPROVED** | **Option A (Include in Phase 1 MVP)**: Require `partner_id` on AR (131) and AP (331) line items. |
| **D5** | Pilot Scope & Parallel Run Protocol | CFO | Phase 3 | **APPROVED** | **Option A (Single Entity Pilot)**: Back-test Q3/Q4 2026 historical vouchers against legacy system to prove zero variance. |
| **D6** | Go-Live Target Schedule | PO | All | **APPROVED** | **Target 2026-12-31 for Phase 1 GL cutover**; e-invoice provider integration target 2026-12-30. |

---

## 2. Detailed Decision Records

### D1: E-Invoice Strategy: In-House XML vs Provider Integration
- **Context & Problem**: QĐ 1233/QĐ-CT and NĐ 254/2026/NĐ-CP require direct or indirect T-VAN message catalogue (types 200, 202, 203, 300) with DSig / XAdES digital signatures by **01/01/2027**.
- **Options**:
  - *Option A (In-House Build)*: Implement XML schema builder, canonicalization (C14N), XAdES/DSig crypto, and direct T-VAN tax communication.
  - *Option B (Certified Provider API)*: Implement clean domain trait `InvoiceTransmitter` and integrate with a licensed provider (VNPT, Viettel, MISA, BKAV) via authenticated REST API.
- **Criteria**: Time-to-market (< 5 weeks remaining to 01/01/2027), risk of HSM compliance failure, maintenance cost.
- **Decision**: **Option B (Certified Provider API)**.
- **Owner**: PO + CFO.
- **Approval Date**: 2026-11-23.

---

### D2: Rounding Method for VND Totals (QĐ 1233 / NĐ 174)
- **Context & Problem**: QĐ 1233 (trang 4) mandates all VND invoice totals (taxable subtotal, VAT amount, payable) must be rounded to integer *đồng* (zero decimals). NĐ 174 Điều 4 khoản 5 mandates half-up rounding ($\ge 0.5 \rightarrow +1$).
- **Options**:
  - *Option A (Line-Level Rounding)*: Round tax on each invoice line to integer, sum lines. Risk: cumulative rounding drift on high-volume low-cost items.
  - *Option B (Subtotal Grouped Rounding per Tax Rate)*: Sum line subtotals per rate, calculate VAT, round to integer đồng. Any $\pm 1$ đồng discrepancy with line sum is adjusted to general ledger income/expense (TK 711/811).
- **Criteria**: Alignment with General Department of Taxation audit practices, mathematical stability.
- **Decision**: **Option B (Subtotal Grouped Rounding per Rate with GL balancing)**.
- **Owner**: Chief Accountant / Tax Advisor.
- **Approval Date**: 2026-11-23.

---

### D3: Tenancy Architecture
- **Context & Problem**: Statutory requirements demand strict data isolation between corporate entities during tax inspection and backup/restore drills.
- **Options**:
  - *Option A (Process-per-Company)*: One container/service instance per legal tax code, independent SQLite DB file.
  - *Option B (Multi-Tenant In-Process Pool)*: Dynamic tenant routing via `X-Company-ID` with pooled connection manager.
- **Criteria**: Delivery simplicity for Phase 1 cutover, audit safety, zero data bleed risk.
- **Decision**: **Option A (Process-per-Company)** for Phase 1. Option B deferred to Phase 4 SaaS backlog.
- **Owner**: PO + Tech Lead.
- **Approval Date**: 2026-11-23.

---

### D4: Partner Subledger Dimension (TK 131 / TK 331) in MVP
- **Context & Problem**: Circular 99 Phụ lục II requires tracking customer and supplier balances in detail.
- **Options**:
  - *Option A (Include in MVP)*: Add optional/mandatory `partner_id` to voucher lines posting to 131/331.
  - *Option B (Defer to Phase 2)*: Keep voucher lines account-only; rely on description text.
- **Criteria**: Compliance with statutory receivables/payables audit requirements.
- **Decision**: **Option A (Include in MVP)**.
- **Owner**: BA + PO.
- **Approval Date**: 2026-11-23.

---

### D5: Pilot Scope & Parallel Run Protocol
- **Context & Problem**: Phase 3 requires validating engine accuracy against existing corporate accounts before cutover.
- **Options**:
  - *Option A (Single Entity Historical Back-Testing)*: Re-run Q3 and Q4 2026 historical vouchers for 1 company, reconciling trial balance to 0 VND variance.
  - *Option B (Multi-Entity Live Parallel Run)*: Dual-entry into legacy and new systems for 60 days across 3 entities.
- **Criteria**: 2026-12-31 timeline constraint.
- **Decision**: **Option A (Single Entity Historical Back-Testing)**.
- **Owner**: CFO.
- **Approval Date**: 2026-11-23.

---

### D6: Go-Live Target Schedule
- **Context & Problem**: Statutory cutover from Circular 200 to Circular 99 occurs on 01/01/2026 (or fiscal year beginning 2026). E-invoice transition deadline under QĐ 1233 is 01/01/2027.
- **Decision**: **Phase 1 GL Go-Live target: 2026-12-31**; E-Invoice Provider Integration target: 2026-12-30.
- **Owner**: PO.
- **Approval Date**: 2026-11-23.
