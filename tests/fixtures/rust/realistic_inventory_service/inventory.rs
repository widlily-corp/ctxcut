//! Core InventoryService microservice implementation.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use super::external::{ErpGrpcClient, RedisLockManager};
use super::models::{
    CatalogAuditSummary, InventoryError, Product, ReservationItem, ReservationRequest,
    ReservationStatus, StockReservation, WarehouseLocation,
};

/// Production inventory service managing catalog availability, locks, and ERP sync.
pub struct InventoryService {
    products: RwLock<HashMap<String, Product>>,
    reservations: RwLock<HashMap<String, StockReservation>>,
    lock_manager: RedisLockManager,
    erp_client: ErpGrpcClient,
}

impl InventoryService {
    /// Creates a new initialized InventoryService.
    pub fn new(
        lock_manager: RedisLockManager,
        erp_client: ErpGrpcClient,
    ) -> Self {
        let mut initial_products = HashMap::new();
        initial_products.insert(
            "SKU-RUST-001".to_string(),
            Product {
                id: "prod_1".to_string(),
                sku: "SKU-RUST-001".to_string(),
                title: "Rust Systems Guide".to_string(),
                description: "Comprehensive systems programming guide in Rust".to_string(),
                unit_price_cents: 4999,
                available_quantity: 120,
                reserved_quantity: 0,
                reorder_threshold: 20,
                location: WarehouseLocation {
                    location_id: "LOC-A-12".to_string(),
                    aisle: "A".to_string(),
                    rack: 1,
                    shelf: 2,
                    bin: "B-04".to_string(),
                    max_capacity_units: 500,
                },
                is_active: true,
            },
        );

        Self {
            products: RwLock::new(initial_products),
            reservations: RwLock::new(HashMap::new()),
            lock_manager,
            erp_client,
        }
    }

    /// Reserves stock for an order across all requested line items with distributed locking.
    pub async fn reserve_stock(
        &self,
        request: ReservationRequest,
    ) -> Result<Vec<StockReservation>, InventoryError> {
        let lock_key = format!("inv_lock_order_{}", request.order_id);
        let lock_id = self
            .lock_manager
            .acquire_lock(&lock_key, Duration::from_secs(10))
            .await?;

        let mut reservations = Vec::with_capacity(request.items.len());
        let mut products = self.products.write().unwrap();

        // 1. Validation phase: ensure all items exist and have sufficient stock
        for item in &request.items {
            let product = products
                .get(&item.sku)
                .ok_or_else(|| InventoryError::ProductNotFound(item.sku.clone()))?;

            if product.available_quantity < item.quantity {
                let _ = self.lock_manager.release_lock(&lock_key, &lock_id).await;
                return Err(InventoryError::InsufficientStock {
                    sku: item.sku.clone(),
                    requested: item.quantity,
                    available: product.available_quantity,
                });
            }
        }

        // 2. Mutation phase: deduct available and increment reserved
        let now_epoch = 1_700_000_000i64;
        let expires_at = now_epoch + request.ttl_seconds as i64;

        for item in &request.items {
            let product = products.get_mut(&item.sku).unwrap();
            product.available_quantity -= item.quantity;
            product.reserved_quantity += item.quantity;

            let res_id = format!("res_{}_{}_{}", request.order_id, item.sku, now_epoch);
            let reservation = StockReservation {
                reservation_id: res_id.clone(),
                order_id: request.order_id.clone(),
                sku: item.sku.clone(),
                quantity: item.quantity,
                status: ReservationStatus::Pending,
                expires_at,
                created_at: now_epoch,
            };

            // Notify ERP system asynchronously
            let _ = self
                .erp_client
                .notify_stock_reserved(&request.order_id, &item.sku, item.quantity)
                .await;

            reservations.push(reservation);
        }

        // 3. Persist reservations
        let mut res_map = self.reservations.write().unwrap();
        for r in &reservations {
            res_map.insert(r.reservation_id.clone(), r.clone());
        }

        let _ = self.lock_manager.release_lock(&lock_key, &lock_id).await;
        Ok(reservations)
    }

    /// Releases a previously held stock reservation back into available inventory.
    pub async fn release_stock(
        &self,
        reservation_id: &str,
        reason: &str,
    ) -> Result<StockReservation, InventoryError> {
        let mut res_map = self.reservations.write().unwrap();
        let reservation = res_map
            .get_mut(reservation_id)
            .ok_or_else(|| InventoryError::ReservationNotFound(reservation_id.to_string()))?;

        if reservation.status != ReservationStatus::Pending {
            return Err(InventoryError::InvalidReservationState {
                reservation_id: reservation_id.to_string(),
                current_status: reservation.status.to_string(),
            });
        }

        let mut products = self.products.write().unwrap();
        if let Some(product) = products.get_mut(&reservation.sku) {
            product.available_quantity += reservation.quantity;
            product.reserved_quantity = product.reserved_quantity.saturating_sub(reservation.quantity);
        }

        reservation.status = ReservationStatus::Released;
        let released_copy = reservation.clone();

        let _ = self
            .erp_client
            .notify_stock_released(&released_copy.sku, released_copy.quantity, reason)
            .await;

        Ok(released_copy)
    }

    /// Generates an analytical audit summary of the entire warehouse product catalog.
    pub fn audit_catalog(&self) -> CatalogAuditSummary {
        let products = self.products.read().unwrap();
        let total_products = products.len();
        let mut total_stock_units = 0u64;
        let mut total_reserved_units = 0u64;
        let mut low_stock_skus = Vec::new();
        let mut valuation_cents = 0u64;

        for product in products.values() {
            total_stock_units += product.available_quantity as u64;
            total_reserved_units += product.reserved_quantity as u64;
            valuation_cents += (product.available_quantity as u64) * product.unit_price_cents;

            if product.available_quantity <= product.reorder_threshold {
                low_stock_skus.push(product.sku.clone());
            }
        }

        CatalogAuditSummary {
            total_products,
            total_stock_units,
            total_reserved_units,
            low_stock_skus,
            valuation_cents,
        }
    }
}
