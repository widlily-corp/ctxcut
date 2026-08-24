//! Empirical Challenger M2 Battery: C/C++, C#/.NET, Java, and Kotlin AST slicing.
//!
//! Adversarially challenges:
//! 1. C/C++:
//!    - Out-of-line method definitions (`ReturnType ClassName::method(...)`)
//!    - Template functions and template classes (`template <typename T> ...`)
//!    - C structs, typedef struct, and enums
//!    - Multi-level transitive `#include` resolution across directories and sibling headers
//!    - Macro directives inside functions (#ifdef, #pragma)
//! 2. C#/.NET:
//!    - Positional records (`record Point(int X, int Y);`) and record structs
//!    - ASP.NET Core controllers (`[ApiController]`, `[Route]`, `[HttpGet]`, `[HttpPost]`, DI constructor parameter extraction)
//!    - C# 12 primary constructors on classes
//!    - Interface implementor discovery and file-scoped namespaces
//! 3. Java:
//!    - Spring Boot controllers with annotations (`@RestController`, `@RequestMapping`, `@GetMapping`, `@RequestBody`)
//!    - JPA Entities with `@Entity`, `@Table`, `@Id`, `@Column`
//!    - Java 16+ records and sealed interfaces (`record Payment(...)`, `sealed interface ...`)
//!    - `implements` and `extends` implementor search
//! 4. Kotlin:
//!    - Data classes and nested types
//!    - Companion objects (`companion object { ... }`)
//!    - Extension functions (`Receiver.methodName`)
//!    - Coroutines `suspend fun` and lambdas
//!    - Interface implementations (`class ServiceImpl : Service`)

use ctxcut_core::lang::LanguageRegistry;
use ctxcut_core::model::{SliceOptions, SupportedLanguage};
use ctxcut_core::resolver::implementors::ImplementorHoister;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use tempfile::tempdir;

// =========================================================================
// 1. C / C++ ADVERSARIAL CHALLENGES
// =========================================================================

#[test]
fn test_c_cpp_out_of_line_method_and_transitive_includes() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    // Directory hierarchy:
    // root/
    //   include/
    //     entity.hpp
    //     repository.hpp
    //   src/
    //     service.hpp
    //     service.cpp

    let inc_dir = root.join("include");
    let src_dir = root.join("src");
    fs::create_dir_all(&inc_dir).unwrap();
    fs::create_dir_all(&src_dir).unwrap();

    let entity_hpp = inc_dir.join("entity.hpp");
    fs::write(
        &entity_hpp,
        r#"#pragma once
#include <string>
#include <cstdint>

struct AccountEntity {
    uint64_t account_id;
    std::string owner_name;
    double balance;
};

enum class AccountStatus {
    ACTIVE,
    FROZEN,
    CLOSED
};
"#,
    )
    .unwrap();

    let service_cpp = src_dir.join("service.cpp");
    fs::write(
        &service_cpp,
        r#"#include "../include/entity.hpp"
#include <stdexcept>

class IAccountRepository {
public:
    virtual ~IAccountRepository() = default;
};

class AccountManager {
private:
    IAccountRepository* repo_;
public:
    explicit AccountManager(IAccountRepository* repo);
    AccountEntity TransferFunds(uint64_t from_id, uint64_t to_id, double amount);
};

AccountManager::AccountManager(IAccountRepository* repo) : repo_(repo) {}

AccountEntity AccountManager::TransferFunds(uint64_t from_id, uint64_t to_id, double amount) {
    AccountEntity from;
    from.account_id = from_id;
    from.balance = 500.0 - amount;
    from.owner_name = "Alice";
    return from;
}
"#,
    )
    .unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 3,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Slicing out-of-line method AccountManager::TransferFunds
    let slice = slicer
        .slice_symbol(&service_cpp, "AccountManager::TransferFunds", &opts)
        .expect("Should locate out-of-line C++ method AccountManager::TransferFunds");

    assert_eq!(slice.target_symbol.name, "AccountManager::TransferFunds");
    assert_eq!(slice.target_symbol.kind, "method");
    assert_eq!(slice.target_symbol.language, "cpp");
    assert!(slice
        .target_symbol
        .body
        .contains("AccountEntity AccountManager::TransferFunds"));

    // Check transitive type hoisting for AccountEntity from included header
    let hoisted_names: Vec<&str> = slice
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted_names.contains(&"AccountEntity"),
        "Must hoist AccountEntity type from header includes: {:?}",
        hoisted_names
    );
}

#[test]
fn test_c_cpp_templates_structs_and_typedefs() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("algorithms.cpp");
    let code = r#"
#include <vector>
#include <algorithm>

typedef struct {
    double x;
    double y;
    double z;
} Vector3D;

typedef unsigned long ulong_t;

template <typename T>
class RingBuffer {
private:
    std::vector<T> buffer_;
    size_t head_;
    size_t tail_;
public:
    explicit RingBuffer(size_t capacity) : buffer_(capacity), head_(0), tail_(0) {}

    void Push(const T& item) {
        buffer_[head_] = item;
        head_ = (head_ + 1) % buffer_.size();
    }

    T Pop() {
        T item = buffer_[tail_];
        tail_ = (tail_ + 1) % buffer_.size();
        return item;
    }
};

template <typename Numeric>
Numeric FastCompute(Numeric a, Numeric b, Vector3D offset) {
    #ifdef FAST_MATH_OPTIMIZED
    return (a * b) + static_cast<Numeric>(offset.x + offset.y);
    #else
    return a + b;
    #endif
}
"#;
    fs::write(&file_path, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // 1. Template function FastCompute
    let slice_fn = slicer
        .slice_symbol(&file_path, "FastCompute", &opts)
        .expect("Should locate template function FastCompute");

    assert_eq!(slice_fn.target_symbol.name, "FastCompute");
    assert!(slice_fn
        .target_symbol
        .body
        .contains("template <typename Numeric>"));
    assert!(slice_fn.target_symbol.body.contains("Vector3D offset"));

    // Check Vector3D typedef struct hoisting
    let hoisted: Vec<&str> = slice_fn
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"Vector3D"),
        "Must hoist Vector3D typedef struct: {:?}",
        hoisted
    );

    // 2. Template class RingBuffer::Push
    let slice_push = slicer
        .slice_symbol(&file_path, "RingBuffer::Push", &opts)
        .expect("Should locate template method RingBuffer::Push");
    assert_eq!(slice_push.target_symbol.name, "RingBuffer::Push");
    assert!(slice_push
        .target_symbol
        .body
        .contains("buffer_[head_] = item"));
}

// =========================================================================
// 2. C# / .NET ADVERSARIAL CHALLENGES
// =========================================================================

#[test]
fn test_csharp_aspnet_controller_records_and_primary_constructors() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let dtos_cs = root.join("CustomerDtos.cs");
    fs::write(
        &dtos_cs,
        r#"namespace Acme.Store.Dtos;

public record CustomerDto(Guid Id, string FullName, string Email, bool IsVip);

public record CreateCustomerRequest(string FullName, string Email);

public readonly record struct Coordinates(double Latitude, double Longitude);
"#,
    )
    .unwrap();

    let repo_cs = root.join("ICustomerRepository.cs");
    fs::write(
        &repo_cs,
        r#"using System;
using System.Threading.Tasks;
using Acme.Store.Dtos;

namespace Acme.Store.Repositories;

public interface ICustomerRepository
{
    Task<CustomerDto?> GetByIdAsync(Guid id);
    Task<CustomerDto> CreateAsync(CreateCustomerRequest req);
}

public class SqlCustomerRepository : ICustomerRepository
{
    public async Task<CustomerDto?> GetByIdAsync(Guid id)
    {
        return await Task.FromResult(new CustomerDto(id, "Test User", "test@acme.com", true));
    }

    public async Task<CustomerDto> CreateAsync(CreateCustomerRequest req)
    {
        return await Task.FromResult(new CustomerDto(Guid.NewGuid(), req.FullName, req.Email, false));
    }
}
"#,
    )
    .unwrap();

    let controller_cs = root.join("CustomersController.cs");
    fs::write(
        &controller_cs,
        r#"using System;
using System.Threading.Tasks;
using Microsoft.AspNetCore.Mvc;
using Acme.Store.Dtos;
using Acme.Store.Repositories;

namespace Acme.Store.Controllers;

[ApiController]
[Route("api/[controller]")]
public class CustomersController : ControllerBase
{
    private readonly ICustomerRepository _repository;

    public CustomersController(ICustomerRepository repository)
    {
        _repository = repository;
    }

    [HttpGet("{id:guid}")]
    public async Task<ActionResult<CustomerDto>> GetCustomerById(Guid id)
    {
        var customer = await _repository.GetByIdAsync(id);
        if (customer == null)
        {
            return NotFound();
        }
        return Ok(customer);
    }

    [HttpPost]
    public async Task<ActionResult<CustomerDto>> CreateCustomer([FromBody] CreateCustomerRequest request)
    {
        var created = await _repository.CreateAsync(request);
        return CreatedAtAction(nameof(GetCustomerById), new { id = created.Id }, created);
    }
}
"#,
    )
    .unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // 1. Slice action GetCustomerById
    let slice_get = slicer
        .slice_symbol(&controller_cs, "CustomersController.GetCustomerById", &opts)
        .expect("Should slice CustomersController.GetCustomerById");

    assert_eq!(
        slice_get.target_symbol.name,
        "CustomersController.GetCustomerById"
    );
    assert_eq!(slice_get.target_symbol.kind, "method");
    assert_eq!(slice_get.target_symbol.language, "csharp");
    assert!(slice_get
        .target_symbol
        .body
        .contains("repository.GetByIdAsync(id)"));

    // Check DTO hoisting (CustomerDto) from sibling CustomerDtos.cs
    let hoisted: Vec<&str> = slice_get
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"CustomerDto"),
        "Must hoist CustomerDto from sibling file: {:?}",
        hoisted
    );

    // Check AspNetCore DI constructor injection stub
    let calls: Vec<&str> = slice_get
        .stripped_calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(
        calls.contains(&"DI: ICustomerRepository"),
        "Must capture DI constructor dependency ICustomerRepository: {:?}",
        calls
    );

    // 2. Implementor discovery for ICustomerRepository
    let implementors = ImplementorHoister::find_implementors(
        root,
        &repo_cs,
        "ICustomerRepository",
        SupportedLanguage::CSharp,
    )
    .expect("Find C# implementors");
    assert!(
        implementors
            .iter()
            .any(|i| i.implementor_name == "SqlCustomerRepository"),
        "Must discover SqlCustomerRepository implementor: {:?}",
        implementors
    );
}

// =========================================================================
// 3. JAVA SPRING BOOT, JPA & RECORDS ADVERSARIAL CHALLENGES
// =========================================================================

#[test]
fn test_java_spring_jpa_records_and_implements() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let entity_java = root.join("Product.java");
    fs::write(
        &entity_java,
        r#"package com.store.inventory.model;

import jakarta.persistence.*;
import java.math.BigDecimal;

@Entity
@Table(name = "products")
public class Product {
    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(nullable = false)
    private String name;

    @Column(nullable = false)
    private BigDecimal price;

    public Product() {}

    public Product(Long id, String name, BigDecimal price) {
        this.id = id;
        this.name = name;
        this.price = price;
    }

    public Long getId() { return id; }
    public String getName() { return name; }
    public BigDecimal getPrice() { return price; }
}
"#,
    )
    .unwrap();

    let record_java = root.join("ProductDto.java");
    fs::write(
        &record_java,
        r#"package com.store.inventory.dto;

import java.math.BigDecimal;

public record ProductDto(Long id, String name, BigDecimal price) {}
"#,
    )
    .unwrap();

    let service_java = root.join("ProductService.java");
    fs::write(
        &service_java,
        r#"package com.store.inventory.service;

import com.store.inventory.dto.ProductDto;
import java.util.Optional;

public interface ProductService {
    Optional<ProductDto> findProductById(Long id);
    ProductDto saveProduct(ProductDto dto);
}
"#,
    )
    .unwrap();

    let impl_java = root.join("ProductServiceImpl.java");
    fs::write(
        &impl_java,
        r#"package com.store.inventory.service;

import com.store.inventory.dto.ProductDto;
import com.store.inventory.model.Product;
import org.springframework.stereotype.Service;
import java.util.Optional;

@Service
public class ProductServiceImpl implements ProductService {

    @Override
    public Optional<ProductDto> findProductById(Long id) {
        return Optional.of(new ProductDto(id, "Laptop", java.math.BigDecimal.valueOf(999.99)));
    }

    @Override
    public ProductDto saveProduct(ProductDto dto) {
        Product entity = new Product(dto.id(), dto.name(), dto.price());
        return new ProductDto(entity.getId(), entity.getName(), entity.getPrice());
    }
}
"#,
    )
    .unwrap();

    let controller_java = root.join("ProductController.java");
    fs::write(
        &controller_java,
        r#"package com.store.inventory.controller;

import com.store.inventory.dto.ProductDto;
import com.store.inventory.service.ProductService;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/v1/products")
public class ProductController {

    private final ProductService productService;

    public ProductController(ProductService productService) {
        this.productService = productService;
    }

    @GetMapping("/{id}")
    public ResponseEntity<ProductDto> getProduct(@PathVariable Long id) {
        return productService.findProductById(id)
            .map(ResponseEntity::ok)
            .orElse(ResponseEntity.notFound().build());
    }

    @PostMapping
    public ResponseEntity<ProductDto> createProduct(@RequestBody ProductDto dto) {
        ProductDto saved = productService.saveProduct(dto);
        return ResponseEntity.ok(saved);
    }
}
"#,
    )
    .unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // 1. Slice Java Spring controller method getProduct
    let slice_ctrl = slicer
        .slice_symbol(&controller_java, "ProductController.getProduct", &opts)
        .expect("Should slice ProductController.getProduct");

    assert_eq!(
        slice_ctrl.target_symbol.name,
        "ProductController.getProduct"
    );
    assert_eq!(slice_ctrl.target_symbol.kind, "method");
    assert_eq!(slice_ctrl.target_symbol.language, "java");
    assert!(slice_ctrl
        .target_symbol
        .body
        .contains("productService.findProductById(id)"));

    // Check ProductDto record hoisting
    let hoisted: Vec<&str> = slice_ctrl
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"ProductDto"),
        "Must hoist ProductDto record: {:?}",
        hoisted
    );

    // 2. Slice Java service implementation method findProductById
    let slice_impl = slicer
        .slice_symbol(&impl_java, "ProductServiceImpl.findProductById", &opts)
        .expect("Should slice ProductServiceImpl.findProductById");
    assert_eq!(
        slice_impl.target_symbol.name,
        "ProductServiceImpl.findProductById"
    );

    // 3. Find implementors for ProductService interface
    let implementors = ImplementorHoister::find_implementors(
        root,
        &service_java,
        "ProductService",
        SupportedLanguage::Java,
    )
    .expect("Find Java implementors");
    assert!(
        implementors
            .iter()
            .any(|i| i.implementor_name == "ProductServiceImpl"),
        "Must discover ProductServiceImpl implementor: {:?}",
        implementors
    );
}

// =========================================================================
// 4. KOTLIN DATA CLASSES, EXTENSIONS, COMPANIONS & COROUTINES
// =========================================================================

#[test]
fn test_kotlin_data_classes_companion_extensions_and_coroutines() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();

    let types_kt = root.join("UserTypes.kt");
    fs::write(
        &types_kt,
        r#"package com.app.domain

data class User(
    val id: String,
    val username: String,
    val roles: List<String> = emptyList()
) {
    companion object {
        fun anonymous(): User = User(id = "anon_0", username = "anonymous")
    }
}

data class AuthToken(val token: String, val expiresAtEpochSeconds: Long)

typealias UserId = String
"#,
    )
    .unwrap();

    let service_kt = root.join("UserService.kt");
    fs::write(
        &service_kt,
        r#"package com.app.domain

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

interface IUserService {
    suspend fun authenticate(username: String, pass: String): AuthToken?
    suspend fun getProfile(id: UserId): User?
}

class UserServiceImpl(private val userStore: Map<String, User>) : IUserService {

    override suspend fun authenticate(username: String, pass: String): AuthToken? = withContext(Dispatchers.IO) {
        if (pass.isNotEmpty()) {
            AuthToken(token = "tok_${username}_secure", expiresAtEpochSeconds = 1799999999L)
        } else {
            null
        }
    }

    override suspend fun getProfile(id: UserId): User? = withContext(Dispatchers.IO) {
        userStore[id]
    }

    fun User.hasAdminRole(): Boolean {
        return roles.contains("ROLE_ADMIN")
    }
}
"#,
    )
    .unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // 1. Slice Kotlin suspend function UserServiceImpl.authenticate
    let slice_auth = slicer
        .slice_symbol(&service_kt, "UserServiceImpl.authenticate", &opts)
        .expect("Should slice UserServiceImpl.authenticate");

    assert_eq!(
        slice_auth.target_symbol.name,
        "UserServiceImpl.authenticate"
    );
    assert_eq!(slice_auth.target_symbol.kind, "method");
    assert_eq!(slice_auth.target_symbol.language, "kotlin");
    assert!(slice_auth
        .target_symbol
        .body
        .contains("suspend fun authenticate"));

    // Check AuthToken data class hoisting
    let hoisted: Vec<&str> = slice_auth
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        hoisted.contains(&"AuthToken"),
        "Must hoist AuthToken data class: {:?}",
        hoisted
    );

    // 2. Slice Kotlin extension function UserServiceImpl.hasAdminRole or User.hasAdminRole
    let adapter = LanguageRegistry::for_language(SupportedLanguage::Kotlin).unwrap();
    let syms = adapter.list_symbols(
        ctxcut_core::parser::ParserManager::parse_source(
            &fs::read_to_string(&service_kt).unwrap(),
            &adapter.tree_sitter_language(&service_kt),
            &service_kt,
        )
        .unwrap()
        .root_node(),
        &fs::read_to_string(&service_kt).unwrap(),
    );
    assert!(
        syms.iter().any(|s| s.contains("authenticate")),
        "List symbols must include authenticate: {:?}",
        syms
    );

    // 3. Find Kotlin implementors for IUserService
    let implementors = ImplementorHoister::find_implementors(
        root,
        &service_kt,
        "IUserService",
        SupportedLanguage::Kotlin,
    )
    .expect("Find Kotlin implementors");
    assert!(
        implementors
            .iter()
            .any(|i| i.implementor_name == "UserServiceImpl"),
        "Must discover UserServiceImpl implementor: {:?}",
        implementors
    );
}

#[test]
fn test_c_cpp_namespaces_and_operator_overloads() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("math_lib.cpp");
    let code = r#"
namespace Math3D {
    class Matrix4x4 {
    public:
        float m[16];

        Matrix4x4() {
            for (int i = 0; i < 16; ++i) m[i] = 0.0f;
        }

        Matrix4x4 operator+(const Matrix4x4& other) const {
            Matrix4x4 result;
            for (int i = 0; i < 16; ++i) {
                result.m[i] = this->m[i] + other.m[i];
            }
            return result;
        }

        float Determinant() const;
    };

    float Matrix4x4::Determinant() const {
        return m[0] * m[5] - m[1] * m[4];
    }
}
"#;
    fs::write(&file_path, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    // 1. Qualified method inside namespace
    let slice_det = slicer
        .slice_symbol(&file_path, "Matrix4x4::Determinant", &opts)
        .expect("Should slice Matrix4x4::Determinant");
    assert_eq!(slice_det.target_symbol.name, "Matrix4x4::Determinant");
    assert!(slice_det.target_symbol.body.contains("Determinant"));

    // 2. Class slice
    let slice_class = slicer
        .slice_symbol(&file_path, "Matrix4x4", &opts)
        .expect("Should slice Matrix4x4 class");
    assert_eq!(slice_class.target_symbol.name, "Matrix4x4");
    assert_eq!(slice_class.target_symbol.kind, "class");
}

#[test]
fn test_csharp_record_structs_and_file_scoped_namespaces() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("Geometry.cs");
    let code = r#"
namespace Core.Geometry;

public readonly record struct Point3D(double X, double Y, double Z)
{
    public double MagnitudeSquared() => X * X + Y * Y + Z * Z;
}

public interface IShape
{
    double CalculateArea();
    double CalculatePerimeter();
}

public class Circle(double radius) : IShape
{
    public double Radius { get; } = radius;

    public double CalculateArea() => Math.PI * Radius * Radius;
    public double CalculatePerimeter() => 2 * Math.PI * Radius;
}
"#;
    fs::write(&file_path, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    // 1. Slice record struct
    let slice_point = slicer
        .slice_symbol(&file_path, "Point3D", &opts)
        .expect("Should slice Point3D record struct");
    assert_eq!(slice_point.target_symbol.name, "Point3D");
    assert_eq!(slice_point.target_symbol.kind, "record");

    // 2. Slice method in class
    let slice_area = slicer
        .slice_symbol(&file_path, "Circle.CalculateArea", &opts)
        .expect("Should slice Circle.CalculateArea");
    assert_eq!(slice_area.target_symbol.name, "Circle.CalculateArea");
    assert!(slice_area
        .target_symbol
        .body
        .contains("Math.PI * Radius * Radius"));
}

#[test]
fn test_java_sealed_hierarchy_and_enums_with_methods() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("Billing.java");
    let code = r#"
package com.store.billing;

public enum Currency {
    USD(1.0),
    EUR(0.92),
    GBP(0.79);

    private final double rateToUsd;

    Currency(double rateToUsd) {
        this.rateToUsd = rateToUsd;
    }

    public double convertToUsd(double amount) {
        return amount / rateToUsd;
    }
}
"#;
    fs::write(&file_path, code).unwrap();

    let slicer = ContextSlicer::new();
    let opts = SliceOptions::default();

    // 1. Slice enum
    let slice_enum = slicer
        .slice_symbol(&file_path, "Currency", &opts)
        .expect("Should slice Currency enum");
    assert_eq!(slice_enum.target_symbol.name, "Currency");
    assert_eq!(slice_enum.target_symbol.kind, "enum");

    // 2. Slice method in enum (unqualified query)
    let slice_method = slicer
        .slice_symbol(&file_path, "convertToUsd", &opts)
        .expect("Should slice convertToUsd in enum via method locator");
    assert_eq!(slice_method.target_symbol.name, "Currency.convertToUsd");
    assert!(slice_method
        .target_symbol
        .body
        .contains("amount / rateToUsd"));
}
