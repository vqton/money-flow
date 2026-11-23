# Delivery Roadmap & Milestones: `money-flow`

**Baseline Date**: 2026-11-23  
**Target General Ledger Cutover**: 2026-12-31 (Effective for Fiscal Year 2026 under TT 99)  
**Hard E-Invoice Statutory Mandate**: 2027-01-01 (QĐ 1233/QĐ-CT Transition Deadline)  
**Engineering Cadence**: 2-Week Sprints (4 Sprints total)  
**Engineering Capacity Assumption**: 2 Senior Rust Engineers (Full-Time) + 1 BA/QA  

---

## 1. Milestones Overview

| Milestone | Description | Target Date | Dependent Tasks | Status |
| :--- | :--- | :---: | :--- | :---: |
| **M0** | Baseline Committed, Tested & Hardened | **2026-11-20** | Commit `71795db`, P0-01, P0-02, P0-05, P0-06 | **COMPLETED** |
| **M1** | Disaster Recovery & Restore Drill Passed | **2026-11-23** | P0-03, P0-04, Commit `715f26d` | **COMPLETED** |
| **M2** | Dynamic Chart of Accounts & Fiscal Year Configured | **2026-11-23** | P1-01, P1-02, P1-03, Commit `715f26d` | **COMPLETED** |
| **M3** | Segregated 3-State Approval Workflow Live | **2026-12-07** | P1-04, P1-05, P1-06 | **ON TRACK** |
| **M4** | Tax Rounding Policy Implemented & Tested | **2026-12-14** | Decision D2 (Approved), P2-01 | **ON TRACK** |
| **M5** | Opening Balances (TT 200 $\rightarrow$ TT 99) Reconciled 0-Diff | **2026-12-21** | P1-07, P1-08, P1-09 | **ON TRACK** |
| **M6** | E-Invoice Provider Integration Verified in Sandbox | **2026-12-28** | Decision D1 (Approved Option B), P2-02, P2-03 | **ON TRACK** |
| **M7** | DSig / XAdES Digital Signing Verified | **2026-12-30** | Decision D1 (Approved Option B), P2-04 | **ON TRACK** |
| **M8** | Pilot Parallel Run & Production Go/No-Go Review | **2027-01-15** | P3-01, P3-02, P3-03, Decision D5 | **SCHEDULED** |

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
  [Phase 0 Complete]
    P0-01..P0-06 [M0, M1]
  [Phase 1 Part 1]
    P1-01..P1-03 [M2]
    P1-04 (Approval WF)
    P1-05 (Segregation)
    P1-06 (Attachments) -> [M3]
                        [Phase 1 Part 2]
                          P1-07 (B01/B03 Reports)
                          P1-09 (Partner Subledger)
                          P1-08 (Open Balances) -> [M5]
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

---

## 3. Capacity Assumptions & Sensitivity Analysis

### Baseline Capacity
- **Headcount**: 2 Senior Rust Engineers (100% allocation) + 1 Domain BA / QA.
- **Velocity**: ~8 story points / PRs per 2-week sprint.
- **Schedule**: All Phase 1 GL features ready by 2026-12-21; E-invoice provider adapter delivered by 2026-12-30.

### Halved Capacity Scenario (1 Rust Engineer)
If capacity is halved to 1 engineer:
1. **Critical Path Impact**: Delivery velocity drops by 50%. Total backlog requires 6 sprints instead of 3.
2. **Phase 1 Target Slip**: Opening balance migration (P1-08) and statutory statements (P1-07) push from 2026-12-21 to **2027-01-25**.
3. **E-Invoice Mandate Violation**: Milestone M6 (E-invoice provider integration) cannot be completed before **2027-02-15**, breaching the 2027-01-01 statutory deadline.
4. **Emergency Mitigation**:
   - Defer P1-06 (Voucher attachments) and P1-09 (Partner subledger) to Phase 2.
   - Restrict Phase 1 exclusively to P1-04 (Approval workflow) and P1-08 (Opening balances).
   - Outsource provider REST adapter integration or use off-the-shelf desktop invoice client temporarily.
