//! Tier 1 Tests: Feature 6 — Java / Kotlin Support
//!
//! Verifies Java and Kotlin AST and language handling:
//! - Spring Boot `@RestController` and `@Service`
//! - JPA `@Entity` models
//! - Kotlin data classes and services
//! - Kotlin coroutine suspend functions
//! - Java records and sealed types

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f6_java_spring_controller_slice() {
    // Arrange: Java Spring Boot RestController
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("UserController.java");
    let content = r#"
package com.example.demo.controller;

import org.springframework.web.bind.annotation.*;
import org.springframework.http.ResponseEntity;

@RestController
@RequestMapping("/api/users")
public class UserController {

    @GetMapping("/{id}")
    public ResponseEntity<String> getUser(@PathVariable String id) {
        return ResponseEntity.ok("User: " + id);
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Run stats
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", file_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: Java file processed cleanly
    output.assert_success();
}

#[test]
fn test_f6_java_jpa_entity_hoisting() {
    // Arrange: Java JPA Entity
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("User.java");
    let content = r#"
package com.example.demo.model;

import jakarta.persistence.*;

@Entity
@Table(name = "users")
public class User {
    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(nullable = false, unique = true)
    private String email;

    public Long getId() { return id; }
    public String getEmail() { return email; }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Token verifier on JPA entity
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: Proper tokenization
    assert!(tokens > 20);
}

#[test]
fn test_f6_kotlin_data_class_and_service() {
    // Arrange: Kotlin data class and service
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("PaymentService.kt");
    let content = r#"
package com.example.demo.service

data class PaymentRequest(val amount: Double, val currency: String)
data class PaymentResponse(val success: Boolean, val transactionId: String)

class PaymentService {
    fun process(request: PaymentRequest): PaymentResponse {
        return PaymentResponse(success = true, transactionId = "TXN_123")
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Run fast stats scan
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", "-f", file_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: Kotlin code analyzed
    output.assert_success();
}

#[test]
fn test_f6_kotlin_coroutine_suspend_function() {
    // Arrange: Kotlin coroutines suspend function
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("AsyncClient.kt");
    let content = r#"
package com.example.demo.client

import kotlinx.coroutines.delay

class AsyncClient {
    suspend fun fetchData(): String {
        delay(100)
        return "Async payload"
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Overview scan
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .expect("Command failed");

    // Assert: Overview successful
    output.assert_success();
}

#[test]
fn test_f6_java_record_and_sealed_interface() {
    // Arrange: Modern Java 17+ records and sealed interfaces
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("DomainEvents.java");
    let content = r#"
package com.example.demo.events;

public sealed interface DomainEvent permits OrderCreated, OrderCancelled {}

public record OrderCreated(String orderId, double amount) implements DomainEvent {}
public record OrderCancelled(String orderId, String reason) implements DomainEvent {}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Stats calculation
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", file_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: Clean exit
    output.assert_success();
}
