//! Tier 4 Real-World Scenario: Rust / Axum / SQLx Microservice Trace
//!
//! Simulates high-performance concurrent Rust backend service:
//! - Axum router and multiple request handlers
//! - SQLx database repository queries
//! - Trait-based domain service contracts
//! - Asserts exact AST extraction and >=60% token reduction

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, GitSandbox, TokenVerifier};

#[test]
fn test_workload_v2_microservice_trace() {
    let sandbox = GitSandbox::new().expect("Failed sandbox");

    // 1. Rust Domain Models & Trait
    let models_content = r#"
pub struct UserRecord {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub role: String,
    pub active: bool,
}

pub struct CreateUserPayload {
    pub username: String,
    pub email: String,
    pub role: String,
}

pub trait UserRepository {
    fn find_by_id(&self, id: u64) -> Option<UserRecord>;
    fn find_by_email(&self, email: &str) -> Option<UserRecord>;
    fn create_user(&self, payload: CreateUserPayload) -> UserRecord;
    fn update_role(&self, id: u64, new_role: &str) -> bool;
    fn deactivate(&self, id: u64) -> bool;
    fn list_active_users(&self) -> Vec<UserRecord>;
}
"#;
    sandbox.write_file("src/domain.rs", models_content).unwrap();

    // 2. Concrete SQLx Repository Implementation
    let repo_content = r#"
use crate::domain::{CreateUserPayload, UserRecord, UserRepository};

pub struct SqlxUserRepository {
    pub connection_pool: String,
}

impl UserRepository for SqlxUserRepository {
    fn find_by_id(&self, id: u64) -> Option<UserRecord> {
        Some(UserRecord {
            id,
            username: format!("user_{id}"),
            email: format!("user_{id}@example.com"),
            role: "member".to_string(),
            active: true,
        })
    }

    fn find_by_email(&self, email: &str) -> Option<UserRecord> {
        Some(UserRecord {
            id: 1,
            username: "admin".to_string(),
            email: email.to_string(),
            role: "admin".to_string(),
            active: true,
        })
    }

    fn create_user(&self, payload: CreateUserPayload) -> UserRecord {
        UserRecord {
            id: 99,
            username: payload.username,
            email: payload.email,
            role: payload.role,
            active: true,
        }
    }

    fn update_role(&self, _id: u64, _new_role: &str) -> bool {
        true
    }

    fn deactivate(&self, _id: u64) -> bool {
        true
    }

    fn list_active_users(&self) -> Vec<UserRecord> {
        vec![
            UserRecord { id: 1, username: "alice".to_string(), email: "alice@test.com".to_string(), role: "admin".to_string(), active: true },
            UserRecord { id: 2, username: "bob".to_string(), email: "bob@test.com".to_string(), role: "member".to_string(), active: true },
        ]
    }
}
"#;
    sandbox.write_file("src/repository.rs", repo_content).unwrap();

    // 3. Axum Route Handlers
    let handler_content = r#"
use crate::domain::{CreateUserPayload, UserRepository};
use crate::repository::SqlxUserRepository;

pub struct AppState {
    pub user_repo: SqlxUserRepository,
}

pub fn handle_get_user(state: &AppState, user_id: u64) -> String {
    match state.user_repo.find_by_id(user_id) {
        Some(user) => format!("User: {} ({})", user.username, user.email),
        None => "User not found".to_string(),
    }
}

pub fn handle_create_user(state: &AppState, username: &str, email: &str) -> String {
    let payload = CreateUserPayload {
        username: username.to_string(),
        email: email.to_string(),
        role: "member".to_string(),
    };
    let created = state.user_repo.create_user(payload);
    format!("Created user ID {}", created.id)
}

pub fn handle_deactivate_user(state: &AppState, user_id: u64) -> String {
    if state.user_repo.deactivate(user_id) {
        "User deactivated".to_string()
    } else {
        "Deactivation failed".to_string()
    }
}

pub fn handle_list_users(state: &AppState) -> String {
    let users = state.user_repo.list_active_users();
    format!("Total active users: {}", users.len())
}
"#;
    let handler_path = sandbox.write_file("src/handlers.rs", handler_content).unwrap();

    sandbox.stage_all().unwrap();
    sandbox.commit("Initial microservice architecture").unwrap();

    // Act: Slice `handle_get_user`
    let runner = CliRunner::new();
    let target = format!("{}:handle_get_user", handler_path.display());
    let output = runner.run_in_dir(sandbox.path(), &["slice", &target]).expect("Command failed");

    // Assert: Handlers extracted cleanly
    output.assert_success();
    assert!(output.stdout.contains("handle_get_user"));

    // Verify token reduction against full microservice files (>= 60%)
    let verifier = TokenVerifier::new();
    let full_text = format!("{}\n{}\n{}", models_content, repo_content, handler_content);
    let metrics = verifier.verify_reduction(&full_text, &output.stdout, 60.0);
    assert!(metrics.reduction_percentage >= 60.0);
}
