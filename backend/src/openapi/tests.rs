use super::*;

#[test]
fn test_openapi_spec_structure() {
    let spec = generate_openapi_spec();
    assert_eq!(spec["openapi"], "3.1.0");
    assert_eq!(spec["info"]["title"], "VAS Enterprise Accounting API");
    assert_eq!(spec["info"]["version"], "1.0.0");

    let paths = spec["paths"].as_object().unwrap();
    assert!(paths.contains_key("/api/v1/system/backup"));
    assert!(paths.contains_key("/api/v1/system/restore"));
    assert!(paths.contains_key("/api/v1/vouchers"));
    assert!(paths.contains_key("/api-docs/openapi.json"));
    assert!(paths.contains_key("/swagger-ui"));

    // Verify security schemes
    let sec_schemes = spec["components"]["securitySchemes"].as_object().unwrap();
    assert!(sec_schemes.contains_key("bearerAuth"));
    assert_eq!(sec_schemes["bearerAuth"]["type"], "http");
    assert_eq!(sec_schemes["bearerAuth"]["scheme"], "bearer");
    assert_eq!(sec_schemes["bearerAuth"]["bearerFormat"], "JWT");

    // Verify protected endpoints declare security and 401
    let voucher_post = &paths["/api/v1/vouchers"]["post"];
    assert!(voucher_post["security"].as_array().is_some());
    assert!(voucher_post["responses"]["401"].as_object().is_some());

    // Verify Idempotency-Key header parameter and 409 response on voucher POST
    let params = voucher_post["parameters"]
        .as_array()
        .expect("Voucher POST must declare parameters");
    assert!(
        params
            .iter()
            .any(|p| p["name"] == "Idempotency-Key" && p["in"] == "header")
    );
    assert!(voucher_post["responses"]["409"].as_object().is_some());

    // Verify privileged endpoints declare 403
    let void_post = &paths["/api/v1/vouchers/{id}/void"]["post"];
    assert!(void_post["security"].as_array().is_some());
    assert!(void_post["responses"]["401"].as_object().is_some());
    assert!(void_post["responses"]["403"].as_object().is_some());

    let audit_get = &paths["/api/v1/audit-logs"]["get"];
    assert!(audit_get["security"].as_array().is_some());
    assert!(audit_get["responses"]["401"].as_object().is_some());
    assert!(audit_get["responses"]["403"].as_object().is_some());

    // Verify unauthenticated endpoints do not declare security
    assert!(paths["/"]["get"]["security"].is_null());
    assert!(paths["/health"]["get"]["security"].is_null());
    assert!(paths["/api-docs/openapi.json"]["get"]["security"].is_null());
    assert!(paths["/swagger-ui"]["get"]["security"].is_null());

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
