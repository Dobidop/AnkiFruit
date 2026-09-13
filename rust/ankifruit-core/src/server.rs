// AnkiFruit — localhost protobuf endpoint.
//
// Anki Desktop's webview reaches the backend by POSTing protobuf to
// `/_anki/<method>` on a loopback HTTP server (see `ts/lib/generated/post.ts`
// and `qt/aqt/mediasrv.py`). Mirroring that contract here means Anki's own
// generated TypeScript client works against this server essentially unchanged.
//
// Note that on iOS a loopback port is reachable by *any* app on the device, so
// every request must carry the bearer token minted at startup.

use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::Path;
use axum::extract::State;
use axum::http::header;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Router;

use crate::routes;

/// Anki sends and receives protobuf under this content type.
const PROTO_CONTENT_TYPE: &str = "application/binary";

pub struct AppState {
    pub backend: anki::backend::Backend,
    pub token: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/_anki/healthz", get(healthz))
        .route("/_anki/{method}", post(call_unqualified))
        .route("/_anki/{service}/{method}", post(call_qualified))
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, anki::version::buildhash())
}

/// Constant-time-ish bearer check. Returns `Err` with a ready-made response.
fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(), Response> {
    let presented = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();

    if constant_time_eq(presented.as_bytes(), state.token.as_bytes()) {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "invalid or missing token").into_response())
    }
}

/// Length-independent comparison, so a caller cannot probe the token by timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    diff == 0
}

async fn call_unqualified(
    State(state): State<Arc<AppState>>,
    Path(method): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = authorize(&headers, &state) {
        return resp;
    }
    match routes::lookup(&method) {
        Some((service, method)) => dispatch(&state, service, method, &body),
        None => (
            StatusCode::NOT_FOUND,
            format!("unknown method: {method}; try /_anki/<Service>/{method}"),
        )
            .into_response(),
    }
}

async fn call_qualified(
    State(state): State<Arc<AppState>>,
    Path((service, method)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(resp) = authorize(&headers, &state) {
        return resp;
    }
    match routes::lookup_qualified(&service, &method) {
        Some((service, method)) => dispatch(&state, service, method, &body),
        None => (
            StatusCode::NOT_FOUND,
            format!("unknown method: {service}/{method}"),
        )
            .into_response(),
    }
}

/// Runs one backend call. Errors come back as protobuf-encoded `BackendError`,
/// which we forward with the status the TypeScript client expects.
fn dispatch(state: &AppState, service: u32, method: u32, input: &[u8]) -> Response {
    match state.backend.run_service_method(service, method, input) {
        Ok(out) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, PROTO_CONTENT_TYPE)],
            out,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, PROTO_CONTENT_TYPE)],
            err,
        )
            .into_response(),
    }
}

/// Binds to an ephemeral loopback port and serves until `shutdown` resolves.
/// Returns the bound port so the caller can hand it to the webview.
pub async fn bind(
    state: Arc<AppState>,
    port: u16,
) -> anyhow::Result<(u16, tokio::net::TcpListener)> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();
    let _ = &state;
    Ok((port, listener))
}
