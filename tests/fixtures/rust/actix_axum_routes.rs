//! Axum and Actix web framework routing declarations, handlers, and DTOs.

use std::collections::HashMap;

/// Axum Json wrapper mock.
#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

/// Mock HTTP status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    Unauthorized = 401,
    NotFound = 404,
    InternalServerError = 500,
}

/// Axum Path extractor mock.
#[derive(Debug, Clone)]
pub struct Path<T>(pub T);

/// Axum State extractor mock.
#[derive(Debug, Clone)]
pub struct State<T>(pub T);

/// Axum route handler type mock.
pub struct MethodRouter;
pub fn get<F>(_handler: F) -> MethodRouter {
    MethodRouter
}
pub fn post<F>(_handler: F) -> MethodRouter {
    MethodRouter
}

/// Axum Router mock.
pub struct Router {
    routes: HashMap<String, MethodRouter>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        self.routes.insert(path.to_string(), method_router);
        self
    }
}

// Request and Response DTOs

#[derive(Debug, Clone, PartialEq)]
pub struct CheckoutRequest {
    pub customer_id: String,
    pub items: Vec<CheckoutItem>,
    pub payment_token: String,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckoutItem {
    pub sku: String,
    pub quantity: u32,
    pub unit_price_cents: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckoutResponse {
    pub order_id: String,
    pub status: String,
    pub total_amount_cents: u64,
    pub confirmation_code: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserProfileResponse {
    pub user_id: String,
    pub email: String,
    pub tier: String,
}

// Axum Route Handlers

/// Axum checkout handler processing checkout payloads.
pub async fn checkout_handler(
    Json(payload): Json<CheckoutRequest>,
) -> Result<(StatusCode, Json<CheckoutResponse>), (StatusCode, String)> {
    if payload.items.is_empty() {
        return Err((StatusCode::BadRequest, "Items list cannot be empty".into()));
    }

    let total: u64 = payload
        .items
        .iter()
        .map(|it| it.unit_price_cents * it.quantity as u64)
        .sum();

    let response = CheckoutResponse {
        order_id: format!("ord_axum_{}", payload.customer_id),
        status: "CONFIRMED".to_string(),
        total_amount_cents: total,
        confirmation_code: "CONF-98765".to_string(),
    };

    Ok((StatusCode::Created, Json(response)))
}

/// Axum user profile handler.
pub async fn get_user_profile_handler(
    Path(user_id): Path<String>,
) -> Result<Json<UserProfileResponse>, (StatusCode, String)> {
    if user_id.is_empty() {
        return Err((StatusCode::NotFound, "User not found".into()));
    }

    Ok(Json(UserProfileResponse {
        user_id: user_id.clone(),
        email: format!("{}@example.corp", user_id),
        tier: "ENTERPRISE".to_string(),
    }))
}

/// Create configured Axum router application.
pub fn build_axum_app() -> Router {
    Router::new()
        .route("/api/v1/checkout", post(checkout_handler))
        .route("/api/v1/users/:user_id", get(get_user_profile_handler))
}
