# Delivery Roadmap & Milestones: `money-flow`

**Baseline Date**: 2026-11-23  
**Target General Ledger Cutover**: 2026-12-31 (Effective for Fiscal Year 2026/2027 under TT 99)  
**Hard E-Invoice Statutory Mandate**: 2027-01-01 (QĐ 1233/QĐ-CT Transition Deadline)  
**Engineering Cadence**: 2-Week Sprints  

---

## 1. Milestones Overview

| Milestone | Description | Target Date | Dependent Tasks | Status |
| :--- | :--- | :---: | :--- | :---: |
| **M0** | Baseline Committed, Tested & Documented | **2026-11-20** | Commit `71795db` | **COMPLETED** |
| **M1** | Disaster Recovery & Restore Drill Passed | **2026-12-05** | P0-01, P0-02, P0-03, P0-04 | **SCHEDULED** |
| **M2** | Dynamic Chart of Accounts & Fiscal Year Configured | **2026-12-12** | P1-01, P1-02, P1-03 | **SCHEDULED** |
| **M3** | Segregated 3-State Approval Workflow Live | **2026-12-19** | P1-04, P1-05, P1-06 | **SCHEDULED** |
| **M4** | Tax Rounding Policy Approved & Implemented | **2026-12-19** | Decision D2 (Approved), P2-01 | **SCHEDULED** |
| **M5** | Opening Balances (TT 200 $\rightarrow$ TT 99) Reconciled 0-Diff | **2026-12-28** | P1-07, P1-08 | **SCHEDULED** |
| **M6** | E-Invoice Provider Integration Verified in Test Environment | **2026-12-30** | Decision D1 (Approved Option B), P2-02, P2-03 | **SCHEDULED (ON TRACK)** |
| **M7** | DSig / XAdES Digital Signing Verified | **2026-12-31** | Decision D1 (Approved Option B), P2-04 | **SCHEDULED (ON TRACK)** |
| **M8** | Pilot Parallel Run & Production Go/No-Go Review | **2027-01-16** | P3-01, P3-02, P3-03, D5 | **SCHEDULED** |

---

## 2. Sprint Timeline & Workstream Gantt

```
2026-11-23                                              2026-12-31            2027-01-18
    │                                                       │ (Statutory)         │
    ▼                                                       ▼                     ▼
┌───────────────────┬───────────────────┬───────────────────┬─────────────────────┐
│     Sprint 1      │     Sprint 2      │     Sprint 3      │      Sprint 4       │
│ (11/24 - 12/07)   │ (12/08 - 12/21)   │ (12/22 - 01/04)   │  (01/05 - 01/18)    │
└───────────────────┴───────────────────┴───────────────────┴─────────────────────┘
  [Phase 0 Hardening]
    P0-01 (Auth audit)
    P0-02 (rusqlite)
    P0-03 (Restore CLI)
    P0-04 (DR Drill) -> [M1]
  [Phase 1 General Ledger]
    P1-01 (Dynamic COA)
    P1-02 (Account Valid)
    P1-03 (Fiscal Year) -> [M2]
                        P1-04 (Approval WF)
                        P1-05 (Segregation)
                        P1-06 (Attachments) -> [M3]
                        P1-07 (B01/B03 Rpts)
                        P1-08 (Open Balances)-> [M5]
                        [Gate D4: P1-09 Subledger]
                                            [Phase 2 Tax & E-Invoice]
                                              P2-01 (Rounding Engine) -> [M4]
                                              P2-02 (Metadata Schema)
                                              P2-03 (Provider Adapter) -> [M6]
                                              P2-04 (Signing Engine)  -> [M7]
                                            [Phase 3 Operations & Pilot]
                                              P3-01 (Windows Service)
                                              P3-02 (DR Runbooks)
                                                                  P3-03 (Pilot Run)
                                                                  Formal Go/No-Go -> [M8]
```

### Sprint Schedule Breakdown

#### Sprint 1 (2026-11-24 to 2026-12-07): Foundation & Chart of Accounts
- **Focus**: Close security and restore gaps; introduce dynamic chart of accounts.
- **Tasks**:
  - `P0-01`: Restrict `/api/v1/audit-logs` to Controller/Admin role.
  - `P0-02`: Upgrade `rusqlite` to latest stable engine (SQLite $\ge 3.53$).
  - `P0-03`: Implement restore engine & CLI.
  - `P0-04`: Automated backup/restore drill suite (**Milestone M1**).
  - `P0-05`: `.gitignore` audit and hygiene.
  - `P0-06`: Commit formal `LICENSE` file.
  - `P1-01`: Dynamic `accounts` schema and statutory seed.
  - `P1-02`: Active account validation in `validate_voucher()`.
  - `P1-03`: Configurable fiscal year start date (**Milestone M2**).
- **Critical Decision**: Sign off **D4** (Subledger Partner Dimension) and **D6** (Go-Live Target) by end of week 1.

#### Sprint 2 (2026-12-08 to 2026-12-21): Workflow, Balances & Financial Statements
- **Focus**: Voucher approval governance, attachments, statement renaming, and opening balance reconciliation.
- **Tasks**:
  - `P1-04`: 3-state voucher lifecycle (`DRAFT` $\rightarrow$ `APPROVED` $\rightarrow$ `POSTED`).
  - `P1-05`: Segregation of duties enforcement (**Milestone M3**).
  - `P1-06`: Voucher attachments with SHA-256 integrity hashes.
  - `P1-07`: Rename Balance Sheet to Statement of Financial Position; build B03-DN Cash Flow.
  - `P1-08`: TT 200 $\rightarrow$ TT 99 opening balances migration tool.
  - `P1-09`: Partner subledger dimension (if approved in D4).
- **Critical Decision**: Sign off **D1** (E-Invoice Build vs Provider) and **D2** (Tax Rounding Protocol) by **2026-12-15**.

#### Sprint 3 (2026-12-22 to 2027-01-04): Tax Rounding, E-Invoice Integration & GL Cutover
- **Focus**: Deliver tax calculation engine, connect e-invoice transmitter, and cut over Phase 1 General Ledger.
- **Tasks**:
  - `P1-08`: Execute opening balance reconciliation test (**Milestone M5 - 2026-12-28**).
  - `P2-01`: VND integer rounding policy per QĐ 1233 (**Milestone M4**).
  - `P2-02`: E-invoice metadata schema (14 adjustment codes, 10 VAT codes).
  - `P2-03`: E-invoice integration adapter (Provider REST API or GDT XML) (**Milestone M6**).
  - `P2-04`: Digital signature validation engine (**Milestone M7**).
  - `P3-01`: Windows Server service wrapper (`windows-service`).

#### Sprint 4 (2027-01-05 to 2027-01-18): Pilot Operations & Final Audit Sign-Off
- **Focus**: Parallel pilot run with live legacy data and disaster recovery runbook validation.
- **Tasks**:
  - `P3-02`: Disaster recovery & log retention runbook publication.
  - `P3-03`: Historical quarter parallel run reconciling 0-VND variance.
  - Executive review & formal Go/No-Go sign-off (**Milestone M8**).

---

## 3. Decision Feasibility & Milestone Risk Warning

> [!WARNING]
> ### Critical Milestone Alert: Milestones M6 and M7 (E-Invoice Mandate)
> As of today (**2026-11-23**), there are exactly **38 calendar days** (~5.5 weeks) until the statutory **01/01/2027** e-invoice transition deadline specified in QĐ 1233/QĐ-CT.
>
> 1. **If Decision D1 chooses Option A (In-House XML & GDT Connector)**:
>    - Building custom XML schema generators for message types 200/202/203/300, XMLDSig / XAdES-BES digital signature wrappers, and direct tax authority cryptographic handshakes requires a minimum of **8 to 12 weeks**.
>    - **Consequence**: **Milestones M6 and M7 WILL MISS the 2027-01-01 deadline**, exposing the organization to statutory non-compliance.
> 2. **If Decision D1 chooses Option B (Certified E-Invoice Provider API)**:
>    - Integrating an authorized provider's REST API (VNPT, Viettel, MISA, BKAV) requires **2 to 3 weeks** (Sprint 3).
>    - **Consequence**: **Milestones M6 and M7 CAN MEET the 2027-01-01 deadline**, provided Decision D1 is signed off no later than **2026-12-05**.

---

## 4. Engineering Capacity Assumptions

### Baseline Capacity (Assumed Plan)
- **Team Size**: **2 Full-Time Senior Backend Engineers (Rust)** + **1 Part-Time Financial Domain Business Analyst / Chief Accountant**.
- **Delivery Velocity**: ~2 Medium tasks or 1 Large task per engineer per sprint.
- **Outcome**: Meets Phase 1 General Ledger target by **2026-12-31** and e-invoice provider integration by **2027-01-01**.

### Halved Capacity Scenario (1 Backend Engineer)
If resource availability is constrained to **1 Senior Engineer**:
1. **Schedule Slip**: Phase 1 General Ledger completion slips from **2026-12-31** to **2027-01-31** (+4 weeks).
2. **Mandatory Scope Pruning**:
   - In-house e-invoice build (Option A) is completely eliminated.
   - Provider e-invoice integration (Option B) must be pushed into late January 2027.
   - Subledger partner dimension (P1-09) must be deferred to Phase 1.5.
   - Windows Server service wrapper (P3-01) must use a third-party wrapper (`nssm.exe`) rather than custom Rust code.
