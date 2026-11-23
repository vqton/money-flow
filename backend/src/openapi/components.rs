use serde_json::{Value, json};

pub fn components_json() -> Value {
    json!({
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT",
                    "description": "JWT Bearer token containing user identity and role (Accountant, Controller, or Admin)"
                }
            },
            "schemas": {
                "AccountClass": {
                    "type": "string",
                    "enum": ["ASSET", "LIABILITY", "EQUITY", "REVENUE", "EXPENSE", "CLOSING"]
                },
                "NormalBalance": {
                    "type": "string",
                    "enum": ["DEBIT", "CREDIT"]
                },
                "Account": {
                    "type": "object",
                    "required": ["code", "name", "class", "is_active", "normal_balance"],
                    "properties": {
                        "code": { "type": "string", "example": "15201" },
                        "name": { "type": "string", "example": "Thep cuon ma kem Posco 1.2mm" },
                        "parent_code": { "type": "string", "nullable": true, "example": "1520" },
                        "class": { "$ref": "#/components/schemas/AccountClass" },
                        "is_active": { "type": "boolean", "example": true },
                        "normal_balance": { "$ref": "#/components/schemas/NormalBalance" }
                    }
                },
                "CreateAccountRequest": {
                    "type": "object",
                    "required": ["code", "name", "class", "normal_balance"],
                    "properties": {
                        "code": { "type": "string", "minLength": 3, "example": "15201" },
                        "name": { "type": "string", "example": "Thep cuon ma kem Posco 1.2mm" },
                        "parent_code": { "type": "string", "nullable": true, "example": "1520" },
                        "class": { "$ref": "#/components/schemas/AccountClass" },
                        "is_active": { "type": "boolean", "default": true, "example": true },
                        "normal_balance": { "$ref": "#/components/schemas/NormalBalance" }
                    }
                },
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
                "RestoreRequest": {
                    "type": "object",
                    "required": ["snapshot_path", "confirm_overwrite", "expected_sha256"],
                    "properties": {
                        "snapshot_path": { "type": "string", "example": "./backups/accounting_20260115_120000.db" },
                        "confirm_overwrite": { "type": "boolean", "example": true },
                        "expected_sha256": { "type": "string", "example": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" }
                    }
                },
                "RestoreResponse": {
                    "type": "object",
                    "required": ["status", "timestamp", "restored_from_snapshot", "sha256_checksum"],
                    "properties": {
                        "status": { "type": "string", "example": "RESTORED" },
                        "timestamp": { "type": "string", "format": "date-time" },
                        "restored_from_snapshot": { "type": "string", "example": "./backups/accounting_20260115_120000.db" },
                        "sha256_checksum": { "type": "string", "example": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" }
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
                                "VOUCHER_ALREADY_VOIDED",
                                "CANNOT_VOID_IN_CLOSED_PERIOD",
                                "BAD_REQUEST",
                                "NOT_FOUND",
                                "INTERNAL_ERROR",
                                "UNAUTHORIZED",
                                "FORBIDDEN"
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
