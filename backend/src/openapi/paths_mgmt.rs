use serde_json::{Value, json};

pub fn paths_mgmt_json() -> Value {
    json!({
            "/api/v1/accounts": {
                "get": {
                    "tags": ["Chart of Accounts"],
                    "summary": "List Chart of Accounts",
                    "description": "Lists accounts in the chart of accounts with optional filtering by class or active status.",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        {
                            "name": "class",
                            "in": "query",
                            "required": false,
                            "schema": { "$ref": "#/components/schemas/AccountClass" }
                        },
                        {
                            "name": "is_active",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "boolean" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of accounts",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/Account" }
                                    }
                                }
                            }
                        },
                        "401": {
                            "description": "Unauthorized",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "tags": ["Chart of Accounts"],
                    "summary": "Create Account",
                    "description": "Creates a new subaccount in the dynamic chart of accounts. Guarded by RequireController.",
                    "security": [{ "bearerAuth": [] }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/CreateAccountRequest" }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Account created successfully",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Account" }
                                }
                            }
                        },
                        "400": {
                            "description": "Bad Request - Validation or cycle error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "409": {
                            "description": "Conflict - Duplicate account code",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "422": {
                            "description": "Unprocessable Entity - Parent account inactive",
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
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
            "/api/v1/periods/close": {
                "post": {
                    "tags": ["Period Governance"],
                    "summary": "Close Accounting Period",
                    "description": "Performs month-end closing: calculates P&L, generates the closing voucher (transferring revenues/expenses to TK 911 and TK 4212), and locks the period.",
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "403": {
                            "description": "Forbidden - Controller role required",
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
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "403": {
                            "description": "Forbidden - Controller role required",
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
                    "description": "Retrieves immutable forensic audit trail records with multi-dimensional filtering by actor, action, resource, and date range. Restricted to Controller or Admin role.",
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "403": {
                            "description": "Forbidden - Controller or Admin role required",
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
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "403": {
                            "description": "Forbidden - Controller role required",
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
            "/api/v1/system/restore": {
                "post": {
                    "tags": ["System"],
                    "summary": "Disaster Recovery Database Restore",
                    "description": "Restores the accounting database from a validated SQLite snapshot. Requires Admin role and dual-secret confirmation (confirm_overwrite: true, expected_sha256).",
                    "security": [
                        { "bearerAuth": [] }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/RestoreRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Database restored successfully",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/RestoreResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Bad Request - Dual-secret confirmation failure, checksum mismatch, or integrity error",
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
                        "403": {
                            "description": "Forbidden - Admin role required",
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
    })
}
