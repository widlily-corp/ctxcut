//! External client integrations: ERP system gRPC client and Redis distributed lock manager.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use super::models::InventoryError;

/// Distributed lock manager simulating Redis Redlock algorithm.
pub struct RedisLockManager {
    redis_endpoint: String,
    active_locks: Mutex<HashMap<String, u64>>,
}

impl RedisLockManager {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            redis_endpoint: endpoint.into(),
            active_locks: Mutex::new(HashMap::new()),
        }
    }

    /// Acquires a distributed lease lock on a key with a TTL.
    pub async fn acquire_lock(&self, resource_key: &str, ttl: Duration) -> Result<String, InventoryError> {
        let mut locks = self.active_locks.lock().unwrap();
        let now_ms = 1_700_000_000_000u64; // Simulated epoch ms

        if let Some(expiry) = locks.get(resource_key) {
            if *expiry > now_ms {
                return Err(InventoryError::LockAcquisitionFailure(resource_key.to_string()));
            }
        }

        let lock_id = format!("lock_{}_{}", resource_key, now_ms);
        locks.insert(resource_key.to_string(), now_ms + ttl.as_millis() as u64);
        Ok(lock_id)
    }

    /// Releases a previously held distributed lock.
    pub async fn release_lock(&self, resource_key: &str, _lock_id: &str) -> Result<(), InventoryError> {
        let mut locks = self.active_locks.lock().unwrap();
        locks.remove(resource_key);
        Ok(())
    }
}

/// gRPC client for synchronizing warehouse operations with Enterprise ERP (SAP / NetSuite).
pub struct ErpGrpcClient {
    erp_host: String,
    auth_token: String,
}

impl ErpGrpcClient {
    pub fn new(host: impl Into<String>, auth_token: impl Into<String>) -> Self {
        Self {
            erp_host: host.into(),
            auth_token: auth_token.into(),
        }
    }

    /// Notifies the upstream ERP system of an inventory reservation.
    pub async fn notify_stock_reserved(
        &self,
        order_id: &str,
        sku: &str,
        quantity: u32,
    ) -> Result<String, InventoryError> {
        if self.auth_token.is_empty() {
            return Err(InventoryError::ExternalErpError("Missing ERP authorization token".into()));
        }
        // Simulated gRPC network call
        Ok(format!("ERP-ACK-{}-{}-{}", order_id, sku, quantity))
    }

    /// Notifies the upstream ERP of stock replenishment or physical warehouse stock releases.
    pub async fn notify_stock_released(
        &self,
        sku: &str,
        quantity: u32,
        reason: &str,
    ) -> Result<bool, InventoryError> {
        if self.erp_host.is_empty() {
            return Err(InventoryError::ExternalErpError("Invalid ERP host endpoint".into()));
        }
        // Simulated gRPC dispatch
        Ok(true)
    }
}
