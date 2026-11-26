-- Migration: create opening_balances table
CREATE TABLE IF NOT EXISTS opening_balances (
    fiscal_year INTEGER NOT NULL,
    effective_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (fiscal_year)
);
