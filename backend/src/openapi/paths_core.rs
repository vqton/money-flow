use serde_json::{Value, json};

pub fn paths_core_json() -> Value {
    json!({
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
                    "security": [
                        { "bearerAuth": [] }
                    ],
                    "parameters": [
                        {
                            "name": "Idempotency-Key",
                            "in": "header",
                            "required": false,
                            "description": "Optional unique idempotency key (1..=255 printable ASCII chars) to safely retry voucher creation without duplicate ledger posting.",
                            "schema": {
                                "type": "string",
                                "minLength": 1,
                                "maxLength": 255,
                                "example": "7c9e6679-7425-40de-944b-e07fc1f90ae7"
                            }
                        }
                    ],
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
                            "description": "Malformed request, invalid or overlong Idempotency-Key header, duplicate voucher number, or validation error",
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
                        "409": {
                            "description": "Idempotency conflict: Idempotency-Key already used with a different request payload",
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
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
            "/api/v1/vouchers/{id}": {
                "get": {
                    "tags": ["Vouchers"],
                    "summary": "Get Voucher by ID",
                    "description": "Retrieves details of a single journal voucher by its UUID.",
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
                        "401": {
                            "description": "Unauthorized - Missing, malformed, or expired Bearer JWT token",
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
            "/api/v1/vouchers/{id}/approve": {
                "post": {
                    "tags": ["Vouchers", "Approval"],
                    "summary": "Approve Draft Voucher",
                    "description": "Approves a DRAFT journal voucher, transitioning it to POSTED status and committing it to the general ledger. Enforces Segregation of Duties: only Controller/Admin can approve, and creator cannot approve own voucher.",
                    "security": [
                        { "bearerAuth": [] }
                    ],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" },
                            "description": "UUID of the draft journal voucher to approve"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Voucher successfully approved and posted",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/VoucherResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Voucher is already approved or voided",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "401": {
                            "description": "Missing or invalid bearer token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                                }
                            }
                        },
                        "403": {
                            "description": "Forbidden: Requires Controller role or creator cannot approve own voucher",
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
                            "description": "Cannot approve voucher in a closed accounting period",
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
                    "security": [
                        { "bearerAuth": [] }
                    ],
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
