//! Unit and integration tests for InventoryService.

#[cfg(test)]
mod tests {
    use super::external::{ErpGrpcClient, RedisLockManager};
    use super::inventory::InventoryService;
    use super::models::{ReservationItem, ReservationRequest, ReservationStatus};

    #[tokio::test]
    async fn test_reserve_stock_success() {
        let lock_manager = RedisLockManager::new("redis://127.0.0.1:6379");
        let erp_client = ErpGrpcClient::new("https://erp.internal.corp:50051");
        let service = InventoryService::new(lock_manager, erp_client);

        let request = ReservationRequest {
            order_id: "ord_rust_101".to_string(),
            customer_id: "cust_rust_202".to_string(),
            items: vec![ReservationItem {
                sku: "SKU-RUST-001".to_string(),
                quantity: 5,
            }],
            ttl_seconds: 300,
        };

        let result = service.reserve_stock(request).await;
        assert!(result.is_ok(), "Expected stock reservation to succeed");
        let reservation = result.unwrap();
        assert_eq!(reservation.order_id, "ord_rust_101");
        assert_eq!(reservation.status, ReservationStatus::Active);
    }

    #[tokio::test]
    async fn test_reserve_stock_insufficient_quantity() {
        let lock_manager = RedisLockManager::new("redis://127.0.0.1:6379");
        let erp_client = ErpGrpcClient::new("https://erp.internal.corp:50051");
        let service = InventoryService::new(lock_manager, erp_client);

        let request = ReservationRequest {
            order_id: "ord_rust_102".to_string(),
            customer_id: "cust_rust_202".to_string(),
            items: vec![ReservationItem {
                sku: "SKU-RUST-001".to_string(),
                quantity: 9999, // Exceeds available 120 units
            }],
            ttl_seconds: 300,
        };

        let result = service.reserve_stock(request).await;
        assert!(result.is_err(), "Expected error due to insufficient quantity");
    }
}
