use serde_json::{json, Value};

/// Generates a complete OpenAPI 3.1.0 specification for the VAS Accounting Engine.
/// Documents all statutory endpoints, request/response models, and error structures
/// under VAS Circular 99/2025/TT-BTC and Circular 91/2026/TT-BTC.
pub fn generate_openapi_spec() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "VAS Enterprise Accounting API",
            "version": "1.0.0",
            "description": "Statutory Accounting Appliance compliant with VAS Circular 99/2025/TT-BTC and Circular 91/2026/TT-BTC. Provides double-entry journal voucher processing, period lock governance, disaster recovery snapshots, and statutory reporting."
        },
        "servers": [
            {
                "url": "/",
                "description": "Current Environment"
            }
        ],
        "tags": [
            { "name": "System", "description": "System health and administrative operations" },
            { "name": "Vouchers", "description": "Double-entry journal voucher lifecycle" },
            { "name": "Period Governance", "description": "Monthly accounting period closing and reopening" },
            { "name": "Audit", "description": "Immutable forensic audit trail and regulatory governance" },
            { "name": "Statutory Reports", "description": "Financial statements and trial balance under Circular 99" },
            { "name": "Documentation", "description": "API contract and interactive documentation" }
        ],
        "paths": {
            "/": {
                "get": {
                    "tags": ["System"],
                    "summary": "Service Root Status",
                    "description": "Returns a simple plaintext status confirming the accounting engine is online.",
                    "responses": {
                        "200": {
                            "description": "Accounting Engine Online",
                            "content": {
                                "text/plain": {
                                    "schema": { "type": "string", "example": "Accounting Engine Online" }
                                }
                            }
                        }
                    }
                }
            },
            "/health": {
                "get": {
                    "tags": ["System"],
                    "summary": "Health Check",
                    "description": "Returns operational health status and application version.",
                    "responses": {
                        "200": {
                            "description": "Service health status",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "status": { "type": "string", "example": "ok" },
                                            "version": { "type": "string", "example": "0.1.0" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/vouchers": {
                "post": {
                    "tags": ["Vouchers"],
                    "summary": "Post Double-Entry Journal Voucher",
                    "description": "Validates statutory invariants (sum of debits == sum of credits, period open, valid accounts) and saves the voucher.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateVoucherRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Voucher created and persisted",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/VoucherResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Malformed request, duplicate voucher number, or validation error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "422": {
                            "description": "Statutory rule violation: UNBALANCED_VOUCHER or PERIOD_CLOSED",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                },
                "get": {
                    "tags": ["Vouchers"],
                    "summary": "List Journal Vouchers",
                    "description": "Retrieves vouchers optionally filtered by an inclusive date range.",
                    "parameters": [
                        {
                            "name": "from_date",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string", "format": "date", "example": "2026-01-01" },
                            "description": "Start date for voucher filtering"
                        },
                        {
                            "name": "to_date",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string", "format": "date", "example": "2026-01-31" },
                            "description": "End date for voucher filtering"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of journal vouchers",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/VoucherResponse" }
                                    }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/vouchers/{id}": {
                "get": {
                    "tags": ["Vouchers"],
                    "summary": "Get Voucher by ID",
                    "description": "Retrieves details of a single journal voucher by its UUID.",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" },
                            "description": "UUID of the journal voucher"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Journal voucher details",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/VoucherResponse" }
                                }
                            }
                        },
                        "404": {
                            "description": "Voucher not found",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/vouchers/{id}/void": {
                "post": {
                    "tags": ["Vouchers", "Audit"],
                    "summary": "Void Journal Voucher",
                    "description": "Non-destructively cancels an active journal voucher. The voucher is marked VOIDED, isolated from reports, and permanently retained with audit logs.",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" },
                            "description": "UUID of the journal voucher to void"
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/VoidVoucherRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Voucher successfully voided",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/VoucherResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Voucher is already voided",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "404": {
                            "description": "Voucher not found",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "422": {
                            "description": "Cannot void voucher in a closed accounting period",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/periods": {
                "get": {
                    "tags": ["Period Governance"],
                    "summary": "List Accounting Periods",
                    "description": "Retrieves the statutory lock status of recorded accounting periods.",
                    "responses": {
                        "200": {
                            "description": "List of accounting periods",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/PeriodStatusResponse" }
                                    }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/periods/close": {
                "post": {
                    "tags": ["Period Governance"],
                    "summary": "Close Accounting Period",
                    "description": "Performs month-end closing: calculates P&L, generates the closing voucher (transferring revenues/expenses to TK 911 and TK 4212), and locks the period.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ClosePeriodRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Period closed successfully",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PeriodStatusResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Period is already closed or invalid month/year",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/periods/reopen": {
                "post": {
                    "tags": ["Period Governance"],
                    "summary": "Reopen Accounting Period",
                    "description": "Reopens a previously closed period for statutory corrections and voids the closing voucher.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/ReopenPeriodRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Period reopened successfully",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/PeriodStatusResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Period is not closed or invalid month/year",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/audit-logs": {
                "get": {
                    "tags": ["Audit"],
                    "summary": "Query Audit Trail",
                    "description": "Retrieves immutable forensic audit trail records with multi-dimensional filtering by actor, action, resource, and date range.",
                    "parameters": [
                        { "name": "actor", "in": "query", "required": false, "schema": { "type": "string" }, "description": "Filter by actor identity" },
                        { "name": "action", "in": "query", "required": false, "schema": { "type": "string" }, "description": "Filter by action type (CREATE_VOUCHER, VOID_VOUCHER, PERIOD_CLOSE, PERIOD_REOPEN, SYSTEM_BACKUP)" },
                        { "name": "resource_type", "in": "query", "required": false, "schema": { "type": "string" }, "description": "Filter by resource type (VOUCHER, PERIOD, SYSTEM)" },
                        { "name": "resource_id", "in": "query", "required": false, "schema": { "type": "string" }, "description": "Filter by resource ID" },
                        { "name": "from_date", "in": "query", "required": false, "schema": { "type": "string" }, "description": "Start timestamp filter" },
                        { "name": "to_date", "in": "query", "required": false, "schema": { "type": "string" }, "description": "End timestamp filter" },
                        { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer", "default": 100 }, "description": "Maximum records to return (up to 1000)" }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of audit log entries",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/AuditLogEntry" }
                                    }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/reports/trial-balance": {
                "get": {
                    "tags": ["Statutory Reports"],
                    "summary": "Statutory Trial Balance Report (Bảng cân đối số phát sinh)",
                    "description": "Aggregates all ledger postings across all accounts and verifies equilibrium (total debit == total credit).",
                    "responses": {
                        "200": {
                            "description": "Trial balance report",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TrialBalanceResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/reports/balance-sheet": {
                "get": {
                    "tags": ["Statutory Reports"],
                    "summary": "Statutory Balance Sheet Report (Bảng cân đối kế toán)",
                    "description": "Computes Balance Sheet as of cut-off date. Accurately deducts contra-asset accounts (TK 214 accumulated depreciation, TK 229 provisions) from total assets.",
                    "parameters": [
                        {
                            "name": "as_of_date",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string", "format": "date", "example": "2026-01-31" },
                            "description": "Cut-off date (defaults to UTC today if omitted)"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Balance sheet report",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/BalanceSheetResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/reports/income-statement": {
                "get": {
                    "tags": ["Statutory Reports"],
                    "summary": "Statutory Income Statement Report (Báo cáo kết quả hoạt động kinh doanh)",
                    "description": "Computes statutory profit & loss metrics across the specified reporting window.",
                    "parameters": [
                        {
                            "name": "from_date",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string", "format": "date", "example": "2026-01-01" },
                            "description": "Start of reporting window"
                        },
                        {
                            "name": "to_date",
                            "in": "query",
                            "required": true,
                            "schema": { "type": "string", "format": "date", "example": "2026-01-31" },
                            "description": "End of reporting window"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Income statement report",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/IncomeStatementResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Missing or invalid query dates",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Internal database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api/v1/system/backup": {
                "post": {
                    "tags": ["System"],
                    "summary": "Create Atomic Database Backup Snapshot",
                    "description": "Initiates an atomic zero-downtime point-in-time SQLite snapshot via VACUUM INTO under the configured backup directory.",
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/BackupRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Backup created successfully",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/BackupResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid filename (directory traversal forbidden)",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "500": {
                            "description": "Backup failure or database error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/api-docs/openapi.json": {
                "get": {
                    "tags": ["Documentation"],
                    "summary": "OpenAPI 3.1 Specification JSON",
                    "description": "Returns this raw machine-readable OpenAPI 3.1.0 document.",
                    "responses": {
                        "200": {
                            "description": "OpenAPI 3.1 JSON document",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            },
            "/swagger-ui": {
                "get": {
                    "tags": ["Documentation"],
                    "summary": "Interactive Swagger UI",
                    "description": "Renders the interactive Swagger UI interface in the browser.",
                    "responses": {
                        "200": {
                            "description": "HTML page rendering Swagger UI",
                            "content": {
                                "text/html": {
                                    "schema": { "type": "string" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "CreateVoucherRequest": {
                    "type": "object",
                    "required": ["voucher_no", "voucher_date", "description", "lines"],
                    "properties": {
                        "voucher_no": { "type": "string", "example": "PKT-2026-001" },
                        "voucher_date": { "type": "string", "format": "date", "example": "2026-01-15" },
                        "description": { "type": "string", "example": "Purchased office supplies on credit" },
                        "invoice_ref": { "type": "string", "nullable": true, "example": "INV-8899" },
                        "created_by": { "type": "string", "nullable": true, "example": "accountant_lan" },
                        "approved_by": { "type": "string", "nullable": true, "example": "cfo_minh" },
                        "lines": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/CreateVoucherLineRequest" },
                            "minItems": 2
                        }
                    }
                },
                "VoidVoucherRequest": {
                    "type": "object",
                    "required": ["reason"],
                    "properties": {
                        "reason": { "type": "string", "example": "Duplicate invoice entered in error" },
                        "actor": { "type": "string", "nullable": true, "example": "cfo_minh" }
                    }
                },
                "CreateVoucherLineRequest": {
                    "type": "object",
                    "required": ["account_code", "debit", "credit"],
                    "properties": {
                        "account_code": { "type": "string", "example": "152" },
                        "debit": { "type": "string", "example": "15000000" },
                        "credit": { "type": "string", "example": "0" },
                        "line_desc": { "type": "string", "nullable": true, "example": "Raw materials inventory" }
                    }
                },
                "VoucherResponse": {
                    "type": "object",
                    "required": ["id", "voucher_no", "voucher_date", "description", "status", "created_by", "created_at", "lines"],
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "voucher_no": { "type": "string" },
                        "voucher_date": { "type": "string", "format": "date" },
                        "description": { "type": "string" },
                        "invoice_ref": { "type": "string", "nullable": true },
                        "status": { "type": "string", "enum": ["POSTED", "VOIDED"], "example": "POSTED" },
                        "created_by": { "type": "string", "example": "accountant_lan" },
                        "approved_by": { "type": "string", "nullable": true, "example": "cfo_minh" },
                        "voided_at": { "type": "string", "nullable": true, "format": "date-time" },
                        "voided_by": { "type": "string", "nullable": true },
                        "void_reason": { "type": "string", "nullable": true },
                        "created_at": { "type": "string", "format": "date-time" },
                        "lines": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/VoucherLineResponse" }
                        }
                    }
                },
                "VoucherLineResponse": {
                    "type": "object",
                    "required": ["id", "line_no", "account_code", "debit", "credit"],
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "line_no": { "type": "integer", "example": 1 },
                        "account_code": { "type": "string" },
                        "debit": { "type": "string" },
                        "credit": { "type": "string" },
                        "line_desc": { "type": "string", "nullable": true }
                    }
                },
                "AuditLogEntry": {
                    "type": "object",
                    "required": ["id", "timestamp", "actor", "action", "resource_type"],
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "timestamp": { "type": "string", "format": "date-time" },
                        "actor": { "type": "string", "example": "cfo_minh" },
                        "action": { "type": "string", "example": "VOID_VOUCHER" },
                        "resource_type": { "type": "string", "example": "VOUCHER" },
                        "resource_id": { "type": "string", "nullable": true, "example": "PKT-2026-001" },
                        "details": { "type": "string", "nullable": true, "example": "Duplicate invoice" },
                        "client_ip": { "type": "string", "nullable": true, "example": "192.168.1.50" }
                    }
                },
                "ClosePeriodRequest": {
                    "type": "object",
                    "required": ["year", "month"],
                    "properties": {
                        "year": { "type": "integer", "example": 2026 },
                        "month": { "type": "integer", "minimum": 1, "maximum": 12, "example": 1 }
                    }
                },
                "ReopenPeriodRequest": {
                    "type": "object",
                    "required": ["year", "month"],
                    "properties": {
                        "year": { "type": "integer", "example": 2026 },
                        "month": { "type": "integer", "minimum": 1, "maximum": 12, "example": 1 },
                        "reason": { "type": "string", "nullable": true, "example": "Adjust year-end audit entries" }
                    }
                },
                "PeriodStatusResponse": {
                    "type": "object",
                    "required": ["year", "month", "is_closed"],
                    "properties": {
                        "year": { "type": "integer" },
                        "month": { "type": "integer" },
                        "is_closed": { "type": "boolean" },
                        "closed_at": { "type": "string", "nullable": true }
                    }
                },
                "TrialBalanceResponse": {
                    "type": "object",
                    "required": ["total_debit", "total_credit", "is_balanced", "rows"],
                    "properties": {
                        "total_debit": { "type": "string" },
                        "total_credit": { "type": "string" },
                        "is_balanced": { "type": "boolean" },
                        "rows": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/TrialBalanceRow" }
                        }
                    }
                },
                "TrialBalanceRow": {
                    "type": "object",
                    "required": ["account_code", "total_debit", "total_credit", "net_balance"],
                    "properties": {
                        "account_code": { "type": "string" },
                        "total_debit": { "type": "string" },
                        "total_credit": { "type": "string" },
                        "net_balance": { "type": "string" }
                    }
                },
                "BalanceSheetResponse": {
                    "type": "object",
                    "required": [
                        "as_of_date",
                        "current_assets",
                        "non_current_assets",
                        "accumulated_depreciation",
                        "asset_provisions",
                        "total_assets",
                        "liabilities",
                        "equity",
                        "total_resources",
                        "is_balanced"
                    ],
                    "properties": {
                        "as_of_date": { "type": "string", "format": "date" },
                        "current_assets": { "type": "string" },
                        "non_current_assets": { "type": "string" },
                        "accumulated_depreciation": { "type": "string", "description": "Contra-asset TK 214" },
                        "asset_provisions": { "type": "string", "description": "Contra-asset TK 229" },
                        "total_assets": { "type": "string", "description": "(current + non_current) - (depreciation + provisions)" },
                        "liabilities": { "type": "string" },
                        "equity": { "type": "string" },
                        "total_resources": { "type": "string", "description": "liabilities + equity" },
                        "is_balanced": { "type": "boolean" }
                    }
                },
                "IncomeStatementResponse": {
                    "type": "object",
                    "required": [
                        "from_date",
                        "to_date",
                        "gross_revenue",
                        "revenue_deductions",
                        "net_revenue",
                        "cost_of_goods_sold",
                        "gross_profit",
                        "financial_income",
                        "financial_expenses",
                        "selling_expenses",
                        "administrative_expenses",
                        "operating_profit",
                        "other_income",
                        "other_expenses",
                        "net_profit_before_tax",
                        "corporate_tax",
                        "net_profit_after_tax"
                    ],
                    "properties": {
                        "from_date": { "type": "string", "format": "date" },
                        "to_date": { "type": "string", "format": "date" },
                        "gross_revenue": { "type": "string" },
                        "revenue_deductions": { "type": "string" },
                        "net_revenue": { "type": "string" },
                        "cost_of_goods_sold": { "type": "string" },
                        "gross_profit": { "type": "string" },
                        "financial_income": { "type": "string" },
                        "financial_expenses": { "type": "string" },
                        "selling_expenses": { "type": "string" },
                        "administrative_expenses": { "type": "string" },
                        "operating_profit": { "type": "string" },
                        "other_income": { "type": "string" },
                        "other_expenses": { "type": "string" },
                        "net_profit_before_tax": { "type": "string" },
                        "corporate_tax": { "type": "string" },
                        "net_profit_after_tax": { "type": "string" }
                    }
                },
                "BackupRequest": {
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string", "nullable": true, "example": "backup_20260115.db" }
                    }
                },
                "BackupResponse": {
                    "type": "object",
                    "required": ["status", "backup_path", "file_size_bytes", "created_at"],
                    "properties": {
                        "status": { "type": "string", "example": "success" },
                        "backup_path": { "type": "string", "example": "./backups/accounting_20260115_120000.db" },
                        "file_size_bytes": { "type": "integer", "example": 32768 },
                        "created_at": { "type": "string", "format": "date-time" }
                    }
                },
                "ErrorResponse": {
                    "type": "object",
                    "required": ["error", "message"],
                    "properties": {
                        "error": {
                            "type": "string",
                            "enum": [
                                "UNBALANCED_VOUCHER",
                                "PERIOD_CLOSED",
                                "BAD_REQUEST",
                                "NOT_FOUND",
                                "INTERNAL_ERROR"
                            ],
                            "example": "PERIOD_CLOSED"
                        },
                        "message": {
                            "type": "string",
                            "example": "Accounting period 2026-01 is closed for postings"
                        }
                    }
                }
            }
        }
    })
}

/// Returns the embedded standalone HTML page initializing Swagger UI.
/// Connects to `/api-docs/openapi.json` without requiring local filesystem UI assets.
pub fn swagger_ui_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>VAS Accounting Engine - Swagger UI</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  <style>
    body { margin: 0; padding: 0; background-color: #fafafa; }
    .topbar { display: none; }
  </style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js" crossorigin></script>
  <script>
    window.onload = () => {
      window.ui = SwaggerUIBundle({
        url: '/api-docs/openapi.json',
        dom_id: '#swagger-ui',
        deepLinking: true,
        presets: [
          SwaggerUIBundle.presets.apis,
          SwaggerUIBundle.SwaggerUIStandalonePreset
        ],
        layout: "BaseLayout"
      });
    };
  </script>
</body>
</html>"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_structure() {
        let spec = generate_openapi_spec();
        assert_eq!(spec["openapi"], "3.1.0");
        assert_eq!(spec["info"]["title"], "VAS Enterprise Accounting API");
        assert_eq!(spec["info"]["version"], "1.0.0");

        let paths = spec["paths"].as_object().unwrap();
        assert!(paths.contains_key("/api/v1/system/backup"));
        assert!(paths.contains_key("/api/v1/vouchers"));
        assert!(paths.contains_key("/api-docs/openapi.json"));
        assert!(paths.contains_key("/swagger-ui"));

        let schemas = spec["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("BackupRequest"));
        assert!(schemas.contains_key("BackupResponse"));
        assert!(schemas.contains_key("ErrorResponse"));
    }

    #[test]
    fn test_swagger_ui_html_content() {
        let html = swagger_ui_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("/api-docs/openapi.json"));
        assert!(html.contains("swagger-ui-bundle.js"));
    }
}
