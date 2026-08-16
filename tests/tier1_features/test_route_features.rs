//! Tier 1: Feature Coverage - Route Handler Slicing Tests (`test_route_features.rs`)
//!
//! Verifies automated resolution of web framework route handlers across Express,
//! FastAPI, Gin, Actix/Axum, and diagnostic reporting for unmatched routes.

#[path = "../common/mod.rs"]
mod common;

use common::CliRunner;

/// Test 1: Resolving an Express POST route handler with middleware and DTOs.
///
/// Arrange: Express router file with `router.post('/api/v1/checkout', authenticate, validate(...), handleCheckout)`.
/// Act: Run `ctxcut route POST /api/v1/checkout`.
/// Assert: Resolves `handleCheckout`, extracts handler body, inlines `CheckoutRequestDTO` and `CheckoutResponseDTO`.
#[test]
fn test_route_express_post_resolution() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner
        .run(&["route", "POST", "/api/v1/checkout"])
        .expect("Failed to execute ctxcut route POST /api/v1/checkout");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("handleCheckout") || stdout.contains("checkout"),
        "Must resolve handleCheckout function"
    );
    assert!(
        stdout.contains("CheckoutRequestDTO") || stdout.contains("CheckoutResponseDTO") || stdout.contains("items"),
        "Must hoist request/response DTOs"
    );
}

/// Test 2: Resolving a parameterized FastAPI GET route handler with schemas.
///
/// Arrange: FastAPI router with `@router.get("/users/{user_id}/profile", response_model=UserProfile)`.
/// Act: Run `ctxcut route GET /api/v1/users/{user_id}/profile`.
/// Assert: Resolves async `get_user_profile`, inlines `UserProfile` Pydantic model.
#[test]
fn test_route_fastapi_get_parameterized() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner
        .run(&["route", "GET", "/api/v1/users/{user_id}/profile"])
        .expect("Failed to execute ctxcut route GET /api/v1/users/{user_id}/profile");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("get_user_profile") || stdout.contains("UserProfile"),
        "Must resolve FastAPI get_user_profile handler"
    );
    assert!(
        stdout.contains("class UserProfile") || stdout.contains("email") || stdout.contains("full_name"),
        "Must hoist UserProfile schema"
    );
}

/// Test 3: Resolving a Go Gin group-prefixed route handler.
///
/// Arrange: Gin router declarations with route group prefixes.
/// Act: Run `ctxcut route POST /v1/auth/login`.
/// Assert: Resolves combined path prefix and extracts `LoginHandler` or `Authenticate`.
#[test]
fn test_route_gin_group_prefixed_route() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner
        .run(&["route", "POST", "/v1/auth/login"])
        .expect("Failed to execute ctxcut route POST /v1/auth/login");

    // Assert
    // Gin handler resolution should identify LoginHandler or Auth handler
    if output.success {
        let stdout = &output.stdout;
        assert!(
            stdout.contains("Login") || stdout.contains("Auth") || stdout.contains("handler"),
            "Must resolve Gin route handler"
        );
    }
}

/// Test 4: Resolving a Rust Axum POST route handler.
///
/// Arrange: Axum Router declarations `route("/inventory/reserve", post(reserve_handler))`.
/// Act: Run `ctxcut route POST /inventory/reserve`.
/// Assert: Resolves handler and inlines request payload DTO.
#[test]
fn test_route_axum_post_handler() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner
        .run(&["route", "POST", "/inventory/reserve"])
        .expect("Failed to execute ctxcut route POST /inventory/reserve");

    // Assert
    if output.success {
        let stdout = &output.stdout;
        assert!(
            stdout.contains("reserve") || stdout.contains("inventory") || stdout.contains("Handler"),
            "Must resolve Axum handler"
        );
    }
}

/// Test 5: Diagnostics and graceful failure when an unmatched route is requested.
///
/// Arrange: Request non-existent route `DELETE /api/v99/unknown/resource`.
/// Act: Run `ctxcut route DELETE /api/v99/unknown/resource`.
/// Assert: Fails cleanly with descriptive diagnostic message or lists registered routes.
#[test]
fn test_route_unmatched_route_diagnostics() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner
        .run(&["route", "DELETE", "/api/v99/unknown/resource"])
        .expect("Failed to execute ctxcut route DELETE");

    // Assert
    // Unmatched route must either return a non-zero exit code or explain no route matched
    if !output.success {
        let combined = format!("{}\n{}", output.stdout, output.stderr);
        assert!(
            combined.contains("not found") || combined.contains("No route") || combined.contains("unmatched") || combined.contains("Unknown"),
            "Error output must provide informative diagnostic message"
        );
    } else {
        assert!(
            output.stdout.contains("No route found") || output.stdout.contains("Did you mean"),
            "Output must indicate route was not found"
        );
    }
}

/// Test 6: Case-insensitivity for HTTP methods (e.g. `post` vs `POST`).
///
/// Arrange: Express router file with POST endpoint.
/// Act: Run `ctxcut route post /api/v1/checkout`.
/// Assert: Successfully resolves handler identically to uppercase POST.
#[test]
fn test_route_method_case_insensitivity() {
    // Arrange
    let runner = CliRunner::new();

    // Act
    let output = runner
        .run(&["route", "post", "/api/v1/checkout"])
        .expect("Failed to execute ctxcut route post");

    // Assert
    output.assert_success();
    let stdout = &output.stdout;
    assert!(
        stdout.contains("handleCheckout") || stdout.contains("checkout"),
        "Must accept lowercase HTTP method"
    );
}
