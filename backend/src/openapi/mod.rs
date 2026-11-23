//! OpenAPI 3.1.0 specification generator and Swagger UI asset provider.

pub mod components;
pub mod paths_core;
pub mod paths_mgmt;
pub mod paths_reports;

#[cfg(test)]
mod tests;

use serde_json::{Value, json};

/// Generates a complete OpenAPI 3.1.0 specification for the VAS Accounting Engine.
/// Documents all statutory endpoints, request/response models, and error structures
/// under VAS Circular 99/2025/TT-BTC and Circular 91/2026/TT-BTC.
pub fn generate_openapi_spec() -> Value {
    let mut spec = json!({
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
        "paths": {}
    });

    let paths_obj = spec
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .unwrap();

    if let Some(core_paths) = paths_core::paths_core_json().as_object() {
        for (k, v) in core_paths {
            paths_obj.insert(k.clone(), v.clone());
        }
    }

    if let Some(mgmt_paths) = paths_mgmt::paths_mgmt_json().as_object() {
        for (k, v) in mgmt_paths {
            paths_obj.insert(k.clone(), v.clone());
        }
    }

    if let Some(reports_paths) = paths_reports::paths_reports_json().as_object() {
        for (k, v) in reports_paths {
            paths_obj.insert(k.clone(), v.clone());
        }
    }

    let spec_obj = spec.as_object_mut().unwrap();
    if let Some(comp) = components::components_json().get("components").cloned() {
        spec_obj.insert("components".to_string(), comp);
    }

    spec
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
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-standalone-preset.js" crossorigin></script>
  <script>
    window.onload = () => {
      window.ui = SwaggerUIBundle({
        url: '/api-docs/openapi.json',
        dom_id: '#swagger-ui',
        presets: [
          SwaggerUIBundle.presets.apis,
          SwaggerUIStandalonePreset
        ],
        layout: "BaseLayout",
        deepLinking: true,
        showExtensions: true,
        showCommonExtensions: true,
        defaultModelsExpandDepth: 2,
        defaultModelExpandDepth: 2,
        docExpansion: "list",
        filter: true
      });
    };
  </script>
</body>
</html>"#
}
