use axum::response::Json;
use utoipa;

use crate::models::ApiResponse;

#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Health check successful", body = ApiResponse<String>)
    ),
    tag = "health"
)]
pub async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("OK".to_string()))
}
