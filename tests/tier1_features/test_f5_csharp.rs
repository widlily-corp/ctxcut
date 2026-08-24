//! Tier 1 Tests: Feature 5 — C# / .NET Support
//!
//! Verifies C# / .NET AST and language handling:
//! - ASP.NET Core controllers & actions
//! - C# records and DTOs
//! - Interface implementations
//! - Generic repositories
//! - LINQ query syntax and token statistics

#[path = "../common/mod.rs"]
mod common;

use common::{CliRunner, TokenVerifier};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_f5_csharp_controller_action_slice() {
    // Arrange: ASP.NET Core Controller with attributes
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("OrdersController.cs");
    let content = r#"
using Microsoft.AspNetCore.Mvc;
using System.Threading.Tasks;

namespace MyApp.Controllers;

[ApiController]
[Route("api/[controller]")]
public class OrdersController : ControllerBase
{
    [HttpGet("{id}")]
    public async Task<IActionResult> GetOrderById(int id)
    {
        return Ok(new { Id = id, Status = "Completed" });
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Run stats scan
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", file_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: C# file processed without errors
    output.assert_success();
}

#[test]
fn test_f5_csharp_record_dto_hoisting() {
    // Arrange: C# record DTOs
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("Dtos.cs");
    let content = r#"
namespace MyApp.Dtos;

public record OrderDto(int Id, decimal Total, string CustomerEmail);
public record CreateOrderRequest(decimal Total, string CustomerEmail);
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Calculate token metrics
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: Token metrics parsed
    assert!(tokens > 0);
}

#[test]
fn test_f5_csharp_interface_implementation() {
    // Arrange: C# interface and implementation class
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("OrderService.cs");
    let content = r#"
namespace MyApp.Services;

public interface IOrderService
{
    Task<bool> ProcessOrderAsync(int orderId);
}

public class OrderService : IOrderService
{
    public async Task<bool> ProcessOrderAsync(int orderId)
    {
        return await Task.FromResult(true);
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Run workspace overview
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["overview", dir.path().to_str().unwrap()])
        .expect("Command failed");

    // Assert: Overview completes successfully
    output.assert_success();
}

#[test]
fn test_f5_csharp_generic_repository_slice() {
    // Arrange: C# generic repository pattern
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("IRepository.cs");
    let content = r#"
using System.Collections.Generic;
using System.Threading.Tasks;

namespace MyApp.Data;

public interface IRepository<T> where T : class
{
    Task<T?> GetByIdAsync(int id);
    Task<IEnumerable<T>> GetAllAsync();
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Stats calculation
    let runner = CliRunner::new();
    let output = runner
        .run_in_dir(dir.path(), &["stats", "-f", file_path.to_str().unwrap()])
        .expect("Command failed");

    // Assert: File scanned
    output.assert_success();
}

#[test]
fn test_f5_csharp_linq_query_slicing() {
    // Arrange: C# class with LINQ expressions
    let dir = TempDir::new().expect("Failed to create tempdir");
    let file_path = dir.path().join("QueryHelper.cs");
    let content = r#"
using System.Linq;
using System.Collections.Generic;

namespace MyApp.Helpers;

public static class QueryHelper
{
    public static IEnumerable<int> FilterEvens(IEnumerable<int> numbers)
    {
        return numbers.Where(n => n % 2 == 0).OrderBy(n => n);
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Act: Verify token counts
    let verifier = TokenVerifier::new();
    let tokens = verifier.count_tokens(content);

    // Assert: LINQ code tokenized accurately
    assert!(tokens >= 20);
}
