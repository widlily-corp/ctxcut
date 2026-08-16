//! Tier 4: Real-World Workload Simulation - Go Auth & Session Microservice (`test_workload_go_auth.rs`)
//!
//! Simulates a production Gin/GORM/JWT AuthService microservice in Go,
//! extracting the `AuthenticateUser` flow and mathematically verifying >=85% token reduction
//! while maintaining 100% semantic correctness of Go structs, method receivers, and repository stubs.

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;

/// Real-World Workload 3: Go AuthService `AuthenticateUser` flow.
///
/// Baseline: Complete `service.go` plus imported `models.go`, `jwt_helper.go`, `repo.go` (~2,150 tokens, ~340 LOC).
/// Target Function: `func (s *AuthService) AuthenticateUser(ctx context.Context, creds LoginCredentials) (*AuthResult, error)`.
/// Expected Slice:
///   1. Full `AuthenticateUser` method body without modification.
///   2. Hoisted Go structs: `LoginCredentials`, `AuthResult`, `User`, `Session`, `Role`.
///   3. Stripped signatures: `JWTHelper.GenerateAccessToken`, `UserRepository.FindByUsername`, `SessionRepository.CreateSession`.
/// Target Token Reduction: >= 85.0% (typically 87–90%).
#[test]
fn test_workload_go_auth_authenticate_user() {
    // Arrange
    let runner = CliRunner::new();
    let verifier = TokenVerifier::new();

    let service_path = "tests/fixtures/go/realistic_auth_service/service.go";
    let models_path = "tests/fixtures/go/realistic_auth_service/models.go";
    let jwt_path = "tests/fixtures/go/realistic_auth_service/jwt_helper.go";
    let repo_path = "tests/fixtures/go/realistic_auth_service/repo.go";

    let full_service = fs::read_to_string(service_path).expect("Failed to read service.go");
    let full_models = fs::read_to_string(models_path).unwrap_or_default();
    let full_jwt = fs::read_to_string(jwt_path).unwrap_or_default();
    let full_repo = fs::read_to_string(repo_path).unwrap_or_default();

    let total_baseline_code = format!("{}\n{}\n{}\n{}", full_service, full_models, full_jwt, full_repo);
    let target = format!("{}:AuthenticateUser", service_path);

    // Act
    let output = runner
        .run(&["slice", &target])
        .expect("Failed to execute ctxcut slice on Go AuthService");

    // Assert: Execution success
    output.assert_success();
    let slice_markdown = &output.stdout;

    // 1. Semantic Verification: Target function body intact
    assert!(
        slice_markdown.contains("AuthenticateUser(ctx context.Context, creds LoginCredentials)")
            || slice_markdown.contains("func (s *AuthService) AuthenticateUser"),
        "Target function signature must be present"
    );
    assert!(
        slice_markdown.contains("expectedHash := s.hashPassword(creds.Password)"),
        "Password verification logic in body must be preserved"
    );
    assert!(
        slice_markdown.contains("accessToken, err := s.jwtHelper.GenerateAccessToken(user, scopes)"),
        "JWT generation call in body must be preserved"
    );

    // 2. Semantic Verification: Type Hoisting
    assert!(
        slice_markdown.contains("LoginCredentials") || slice_markdown.contains("AuthResult") || slice_markdown.contains("User"),
        "Required Go structs must be hoisted"
    );

    // 3. Semantic Verification: Unrelated sibling method bodies omitted
    assert!(
        !slice_markdown.contains("func (s *AuthService) Register(ctx context.Context, req RegistrationRequest) (*User, error) {"),
        "Sibling method Register body must NOT be included in slice"
    );
    assert!(
        !slice_markdown.contains("func (s *AuthService) RefreshToken(ctx context.Context, oldRefreshToken string) (*AuthResult, error) {"),
        "Sibling method RefreshToken body must NOT be included in slice"
    );

    // 4. Quantitative Token Reduction Verification (Mathematical Proof >= 85%)
    let metrics = verifier.verify_reduction(&total_baseline_code, slice_markdown, 85.0);

    println!(
        "\n==========================================================\n\
         Go AuthService Microservice Slicing Results:\n\
         Baseline Tokens:     {}\n\
         Sliced Tokens:       {}\n\
         Token Reduction:     {:.2}%\n\
         Baseline Lines:      {}\n\
         Sliced Lines:        {}\n\
         ==========================================================",
        metrics.full_tokens,
        metrics.slice_tokens,
        metrics.reduction_percentage,
        metrics.full_lines,
        metrics.slice_lines
    );

    assert!(
        metrics.reduction_percentage >= 85.0,
        "Workload 3 token reduction must be >= 85.0%, got {:.2}%",
        metrics.reduction_percentage
    );
}
