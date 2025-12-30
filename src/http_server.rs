/// HTTP Server Module
/// 
/// Provides HTTP endpoints for triggering Twitter sync and viewing sync status.

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::rag::RAGSystem;
use crate::twitter_sync::{sync_tweets, SyncResult};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub rag_system: Arc<RAGSystem>,
    pub sync_status: Arc<RwLock<SyncStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_sync: Option<DateTime<Utc>>,
    pub last_result: Option<SyncResult>,
    pub total_syncs: u32,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            last_sync: None,
            last_result: None,
            total_syncs: 0,
        }
    }
}

/// Create and configure the HTTP server router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/sync-twitter", post(sync_twitter_endpoint))
        .route("/sync-status", get(sync_status_endpoint))
        .route("/knowledge-stats", get(knowledge_stats_endpoint))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "status": "ok",
        "service": "pollinet_knowledge_bot",
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// Sync Twitter endpoint
async fn sync_twitter_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Check authentication if sync_api_secret is set
    if let Some(secret) = &state.config.sync_api_secret {
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        if !auth_header.starts_with("Bearer ") {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let token = auth_header.strip_prefix("Bearer ").unwrap();
        if token != secret.as_str() {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // Check if Twitter API is configured
    if state.config.twitter_api_key.is_none() {
        return Ok(Json(serde_json::json!({
            "error": "Twitter API not configured",
            "message": "Set TWITTER_API_KEY in environment variables"
        })));
    }

    log::info!("Twitter sync triggered via HTTP endpoint");

    // Perform sync
    match sync_tweets(&state.config, state.rag_system.clone()).await {
        Ok(result) => {
            // Update sync status
            let mut status = state.sync_status.write().await;
            status.last_sync = Some(Utc::now());
            status.last_result = Some(SyncResult {
                tweets_added: result.tweets_added,
                tweets_skipped: result.tweets_skipped,
                last_sync: result.last_sync,
            });
            status.total_syncs += 1;

            Ok(Json(serde_json::json!({
                "status": "success",
                "tweets_added": result.tweets_added,
                "tweets_skipped": result.tweets_skipped,
                "last_sync": result.last_sync.to_rfc3339()
            })))
        }
        Err(e) => {
            log::error!("Twitter sync failed: {}", e);
            Ok(Json(serde_json::json!({
                "status": "error",
                "error": e.to_string()
            })))
        }
    }
}

/// Get sync status endpoint
async fn sync_status_endpoint(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let status = state.sync_status.read().await;

    Ok(Json(serde_json::json!({
        "last_sync": status.last_sync.map(|d| d.to_rfc3339()),
        "total_syncs": status.total_syncs,
        "last_result": status.last_result.as_ref().map(|r| serde_json::json!({
            "tweets_added": r.tweets_added,
            "tweets_skipped": r.tweets_skipped,
            "last_sync": r.last_sync.to_rfc3339()
        }))
    })))
}

/// Get knowledge base statistics endpoint
async fn knowledge_stats_endpoint(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Query database for statistics
    // For now, return a placeholder - you can enhance this to query actual stats
    Ok(Json(serde_json::json!({
        "message": "Knowledge base statistics",
        "note": "Enhanced statistics can be added by querying the database"
    })))
}

/// Start the HTTP server
pub async fn start_server(state: AppState, port: u16) -> Result<()> {
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .context("Failed to bind HTTP server")?;

    log::info!("HTTP server listening on port {}", port);

    axum::serve(listener, app)
        .await
        .context("HTTP server error")?;

    Ok(())
}

