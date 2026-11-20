# Project: Accounting Backend

## Architecture
- Module: `backend/`
- Data Flow: HTTP (Axum) -> Logic (TBD) -> Storage (SQLx/SQLite)

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Health Check | Basic endpoint to verify service | M1 | alpha_plan.md |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | Scaffolding | Basic structure, dependencies, health check | none | DONE |

## Interface Contracts
### API
- GET `/`: returns "Accounting Engine Online"
