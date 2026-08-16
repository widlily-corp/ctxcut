//! Data models and entities for the realistic inventory service.

use std::fmt;

/// Warehouse location entity representing physical bin storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarehouseLocation {
    pub location_id: String,
    pub aisle: String,
    pub rack: u32,
    pub shelf: u32,
    pub bin: String,
    pub max_capacity_units: u32,
}

/// Catalog product entity.
#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    pub id: String,
    pub sku: String,
    pub title: String,
    pub description: String,
    pub unit_price_cents: u64,
    pub available_quantity: u32,
    pub reserved_quantity: u32,
    pub reorder_threshold: u32,
    pub location: WarehouseLocation,
    pub is_active: bool,
}

/// Stock reservation record tracking held inventory for pending orders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockReservation {
    pub reservation_id: String,
    pub order_id: String,
    pub sku: String,
    pub quantity: u32,
    pub status: ReservationStatus,
    pub expires_at: i64,
    pub created_at: i64,
}

/// Status of an active or expired stock reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationStatus {
    Pending,
    Confirmed,
    Released,
    Expired,
}

impl fmt::Display for ReservationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReservationStatus::Pending => write!(f, "PENDING"),
            ReservationStatus::Confirmed => write!(f, "CONFIRMED"),
            ReservationStatus::Released => write!(f, "RELEASED"),
            ReservationStatus::Expired => write!(f, "EXPIRED"),
        }
    }
}

/// Inventory catalog audit report summary.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogAuditSummary {
    pub total_products: usize,
    pub total_stock_units: u64,
    pub total_reserved_units: u64,
    pub low_stock_skus: Vec<String>,
    pub valuation_cents: u64,
}

/// Reservation request payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationRequest {
    pub order_id: String,
    pub items: Vec<ReservationItem>,
    pub ttl_seconds: u64,
}

/// Individual item within a reservation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationItem {
    pub sku: String,
    pub quantity: u32,
}

/// Domain errors occurring in inventory operations.
#[derive(Debug, PartialEq, Eq)]
pub enum InventoryError {
    ProductNotFound(String),
    InsufficientStock {
        sku: String,
        requested: u32,
        available: u32,
    },
    ReservationNotFound(String),
    InvalidReservationState {
        reservation_id: String,
        current_status: String,
    },
    LockAcquisitionFailure(String),
    ExternalErpError(String),
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InventoryError::ProductNotFound(sku) => write!(f, "Product with SKU '{}' not found", sku),
            InventoryError::InsufficientStock { sku, requested, available } => {
                write!(f, "Insufficient stock for SKU '{}': requested {}, available {}", sku, requested, available)
            }
            InventoryError::ReservationNotFound(id) => write!(f, "Reservation '{}' not found", id),
            InventoryError::InvalidReservationState { reservation_id, current_status } => {
                write!(f, "Reservation '{}' is in invalid state '{}'", reservation_id, current_status)
            }
            InventoryError::LockAcquisitionFailure(res) => write!(f, "Failed to acquire lock for '{}'", res),
            InventoryError::ExternalErpError(msg) => write!(f, "External ERP error: {}", msg),
        }
    }
}

impl std::error::Error for InventoryError {}
