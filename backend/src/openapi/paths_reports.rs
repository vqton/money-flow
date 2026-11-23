use serde_json::{Value, json};

pub fn paths_reports_json() -> Value {
    json!({
            "/api/v1/reports/trial-balance": {
                "get": {
                    "tags": ["Statutory Reports"],
                    "summary": "Statutory Trial Balance Report (Bảng cân đối số phát sinh)",
                    "description": "Aggregates all ledger postings across all accounts and verifies equilibrium (total debit == total credit).",
                    "security": [
                        { "bearerAuth": [] }
                    ],
                    "responses": {
                        "200": {
                            "description": "Trial balance report",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TrialBalanceResponse" }
                                }
                            }
                        },
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
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
            "/api/v1/reports/balance-sheet": {
                "get": {
                    "tags": ["Statutory Reports"],
                    "summary": "Statutory Balance Sheet Report (Bảng cân đối kế toán)",
                    "description": "Computes Balance Sheet as of cut-off date. Accurately deducts contra-asset accounts (TK 214 accumulated depreciation, TK 229 provisions) from total assets.",
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
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
            "/api/v1/reports/income-statement": {
                "get": {
                    "tags": ["Statutory Reports"],
                    "summary": "Statutory Income Statement Report (Báo cáo kết quả hoạt động kinh doanh)",
                    "description": "Computes statutory profit & loss metrics across the specified reporting window.",
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
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
            }
    })
}
