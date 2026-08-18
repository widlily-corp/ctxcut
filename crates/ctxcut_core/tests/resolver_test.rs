//! Comprehensive Unit & Integration Tests for ForeignSymbolLocator & Resolvers (Milestone 2).

use ctxcut_core::model::SliceOptions;
use ctxcut_core::resolver::{DefaultForeignSymbolLocator, ForeignSymbolLocator, ImportResolver};
use std::path::Path;

#[test]
fn test_resolve_import_ts_relative_and_extension() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file =
        Path::new("../../tests/fixtures/typescript/realistic_order_service/order_service.ts");

    // Relative specifier without extension
    let resolved = locator.resolve_import_path(current_file, "./models");
    assert!(resolved.is_some(), "Expected ./models to resolve");
    let resolved_path = resolved.unwrap();
    assert!(
        resolved_path.to_string_lossy().ends_with("models.ts"),
        "Resolved path should be models.ts: {resolved_path:?}"
    );

    // Relative specifier with .ts extension
    let resolved_ts = locator.resolve_import_path(current_file, "./gateways.ts");
    assert!(resolved_ts.is_some(), "Expected ./gateways.ts to resolve");
    assert!(resolved_ts
        .unwrap()
        .to_string_lossy()
        .ends_with("gateways.ts"));
}

#[test]
fn test_resolve_import_ts_directory_index() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file = Path::new("../../tests/fixtures/typescript/barrel_imports/service.ts");

    let resolved = locator.resolve_import_path(current_file, "./index");
    assert!(resolved.is_some(), "Expected ./index to resolve");
    assert!(resolved.unwrap().to_string_lossy().ends_with("index.ts"));
}

#[test]
fn test_resolve_import_ts_barrel_reexport_hop() {
    let current_file = Path::new("../../tests/fixtures/typescript/barrel_imports/index.ts");
    let resolved = ImportResolver::resolve_module_path(current_file, "./sub");
    assert!(resolved.is_some(), "Expected ./sub to resolve to index.ts");
    let path = resolved.unwrap();
    assert!(path.to_string_lossy().contains("sub"));
}

#[test]
fn test_resolve_import_py_relative_levels() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file =
        Path::new("../../tests/fixtures/python/realistic_payment_service/payment_service.py");

    // Dot relative: .schemas
    let resolved = locator.resolve_import_path(current_file, ".schemas");
    assert!(resolved.is_some(), "Expected .schemas to resolve");
    let p = resolved.unwrap();
    assert!(p.to_string_lossy().ends_with("schemas.py"));

    // Dot relative: .clients
    let resolved_clients = locator.resolve_import_path(current_file, ".clients");
    assert!(resolved_clients.is_some(), "Expected .clients to resolve");
    assert!(resolved_clients
        .unwrap()
        .to_string_lossy()
        .ends_with("clients.py"));
}

#[test]
fn test_resolve_import_py_init_package() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file =
        Path::new("../../tests/fixtures/python/realistic_payment_service/payment_service.py");

    let resolved = locator.resolve_import_path(current_file, ".");
    assert!(resolved.is_some(), "Expected package root to resolve");
}

#[test]
fn test_resolve_import_go_same_package() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file = Path::new("../../tests/fixtures/go/realistic_auth_service/service.go");

    let resolved = locator.resolve_import_path(current_file, "models.go");
    assert!(resolved.is_some(), "Expected models.go to resolve");
    assert!(resolved.unwrap().to_string_lossy().ends_with("models.go"));
}

#[test]
fn test_resolve_import_rust_mod_hierarchy() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file =
        Path::new("../../tests/fixtures/rust/realistic_inventory_service/inventory.rs");

    let resolved = locator.resolve_import_path(current_file, "models");
    assert!(
        resolved.is_some(),
        "Expected models to resolve in Rust service"
    );
    assert!(resolved.unwrap().to_string_lossy().ends_with("models.rs"));
}

#[test]
fn test_foreign_signature_extraction_ts() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file =
        Path::new("../../tests/fixtures/typescript/realistic_order_service/gateways.ts");

    let stub = locator
        .locate_foreign_signature(target_file, "StripeGateway.chargeCard")
        .expect("Failed to query signature")
        .expect("Expected StripeGateway.chargeCard to be found");

    assert_eq!(stub.name, "chargeCard");
    assert!(stub.signature.contains("chargeCard("));
    assert!(stub.signature.contains("amountCents: number"));
    assert!(stub.signature.ends_with(';'));
    // Zero body leakage: should not contain implementation code
    assert!(!stub.signature.contains("if (!this.apiKey)"));
    assert!(!stub.signature.contains("throw new Error"));
}

#[test]
fn test_foreign_signature_extraction_py() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file = Path::new("../../tests/fixtures/python/realistic_payment_service/clients.py");

    let stub = locator
        .locate_foreign_signature(target_file, "BankingGatewayClient.authorize_charge")
        .expect("Failed to query signature")
        .expect("Expected BankingGatewayClient.authorize_charge to be found");

    assert_eq!(stub.name, "authorize_charge");
    assert!(stub.signature.contains("def authorize_charge("));
    assert!(stub.signature.ends_with(": ..."));
    // Zero body leakage
    assert!(!stub.signature.contains("endpoint = f\"{self.base_url}"));
    assert!(!stub.signature.contains("raw_payload = json.dumps"));
}

#[test]
fn test_foreign_signature_extraction_go() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file = Path::new("../../tests/fixtures/go/realistic_auth_service/jwt_helper.go");

    let stub = locator
        .locate_foreign_signature(target_file, "GenerateToken")
        .expect("Failed to query signature")
        .expect("Expected GenerateToken to be found");

    assert_eq!(stub.name, "GenerateToken");
    assert!(stub.signature.contains("func GenerateToken("));
    assert!(!stub.signature.contains("claims := Claims{"));
}

#[test]
fn test_foreign_signature_extraction_rust() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file =
        Path::new("../../tests/fixtures/rust/realistic_inventory_service/external.rs");

    let stub = locator
        .locate_foreign_signature(target_file, "RedisLockManager::acquire_lock")
        .expect("Failed to query signature")
        .expect("Expected RedisLockManager::acquire_lock to be found");

    assert_eq!(stub.name, "acquire_lock");
    assert!(stub.signature.contains("pub async fn acquire_lock("));
    assert!(stub.signature.ends_with(';'));
    // Zero body leakage
    assert!(!stub.signature.contains("let mut locks = self.active_locks"));
}

#[test]
fn test_foreign_type_hoisting_ts() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file =
        Path::new("../../tests/fixtures/typescript/realistic_order_service/models.ts");

    let types = locator
        .hoist_foreign_types(
            target_file,
            &["OrderCreationRequest", "Customer", "OrderStatus"],
        )
        .expect("Failed to hoist types");

    assert_eq!(types.len(), 3);
    let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"OrderCreationRequest"));
    assert!(names.contains(&"Customer"));
    assert!(names.contains(&"OrderStatus"));

    for t in &types {
        assert!(!t.definition.is_empty());
    }
}

#[test]
fn test_foreign_type_hoisting_py() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file = Path::new("../../tests/fixtures/python/realistic_payment_service/schemas.py");

    let types = locator
        .hoist_foreign_types(target_file, &["ChargeRequest", "ChargeResult", "Currency"])
        .expect("Failed to hoist Python types");

    assert_eq!(types.len(), 3);
    let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"ChargeRequest"));
    assert!(names.contains(&"ChargeResult"));
    assert!(names.contains(&"Currency"));
}

#[test]
fn test_foreign_type_hoisting_go() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file = Path::new("../../tests/fixtures/go/realistic_auth_service/models.go");

    let types = locator
        .hoist_foreign_types(target_file, &["User", "Role", "Session"])
        .expect("Failed to hoist Go types");

    assert_eq!(types.len(), 3);
    let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"User"));
    assert!(names.contains(&"Role"));
    assert!(names.contains(&"Session"));
}

#[test]
fn test_foreign_type_hoisting_rust() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file = Path::new("../../tests/fixtures/rust/realistic_inventory_service/models.rs");

    let types = locator
        .hoist_foreign_types(
            target_file,
            &["WarehouseLocation", "StockReservation", "ReservationStatus"],
        )
        .expect("Failed to hoist Rust types");

    assert_eq!(types.len(), 3);
    let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"WarehouseLocation"));
    assert!(names.contains(&"StockReservation"));
    assert!(names.contains(&"ReservationStatus"));
}

#[test]
fn test_resolver_depth_0_isolation() {
    let slicer = ctxcut_core::ContextSlicer::new();
    let target_file =
        Path::new("../../tests/fixtures/typescript/realistic_order_service/order_service.ts");
    let opts = SliceOptions {
        depth: 0,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = slicer
        .slice_symbol(target_file, "OrderService.processOrder", &opts)
        .expect("Failed to slice at depth 0");

    // At depth 0, foreign types from models.ts should not be hoisted
    let hoisted_names: Vec<&str> = result
        .hoisted_types
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert!(
        !hoisted_names.contains(&"OrderCreationRequest"),
        "Depth 0 should not hoist foreign types"
    );
}

#[test]
fn test_resolver_circular_import_safety() {
    let slicer = ctxcut_core::ContextSlicer::new();
    let target_file = Path::new("../../tests/fixtures/typescript/circular_types.ts");
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Circular types: User -> Post -> Comment -> User
    let result = slicer
        .slice_symbol(target_file, "formatUser", &opts)
        .expect("Circular types slicing should succeed without infinite recursion");

    assert!(!result.hoisted_types.is_empty());
}

#[test]
fn test_resolver_missing_file_resilience() {
    let locator = DefaultForeignSymbolLocator::new();
    let current_file = Path::new("../../tests/fixtures/typescript/non_existent.ts");

    let resolved = locator.resolve_import_path(current_file, "./missing_module");
    assert!(
        resolved.is_none(),
        "Non-existent module should return None gracefully"
    );

    let target_file = Path::new("../../tests/fixtures/typescript/non_existent.ts");
    let stub = locator
        .locate_foreign_signature(target_file, "missing_func")
        .expect("Non-existent file should return Ok(None)");
    assert!(stub.is_none());

    let types = locator
        .hoist_foreign_types(target_file, &["MissingType"])
        .expect("Non-existent file should return Ok(empty)");
    assert!(types.is_empty());
}

#[test]
fn test_default_foreign_symbol_locator_caching() {
    let locator = DefaultForeignSymbolLocator::new();
    let target_file =
        Path::new("../../tests/fixtures/typescript/realistic_order_service/models.ts");

    let types1 = locator
        .hoist_foreign_types(target_file, &["Customer"])
        .expect("First query");
    assert_eq!(types1.len(), 1);

    // Second query uses cache
    let types2 = locator
        .hoist_foreign_types(target_file, &["Customer"])
        .expect("Second cached query");
    assert_eq!(types2.len(), 1);

    locator.clear_cache();
    let types3 = locator
        .hoist_foreign_types(target_file, &["Customer"])
        .expect("Query after clear_cache");
    assert_eq!(types3.len(), 1);
}
