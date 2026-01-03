/// Configuration module for managing environment variables and API keys
/// 
/// This module loads and validates all required configuration values from
/// environment variables (typically from a .env file).

use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    /// Telegram bot token from BotFather
    pub telegram_token: String,
    
    /// OpenAI API key for GPT-4o-mini
    pub openai_api_key: String,
    
    /// PostgreSQL database URL (e.g., "postgres://user:password@localhost/dbname")
    pub database_url: String,
    
    /// Table name for storing document embeddings
    pub embeddings_table: String,
    
    /// Embedding model to use (e.g., "text-embedding-ada-002")
    pub embedding_model: String,
    
    /// GPT model to use (e.g., "gpt-4o-mini")
    pub gpt_model: String,
    
    /// Maximum number of conversation messages to keep in memory
    pub max_conversation_history: usize,
    
    /// Number of document chunks to retrieve for context
    pub top_k_chunks: usize,
    
    /// Maximum chunks to include in fallback context (limits token cost)
    pub max_fallback_chunks: usize,
    
    /// Webhook URL for receiving updates (if using webhooks)
    /// If not set, will auto-detect from Railway/Fly.io environment variables
    pub webhook_url: Option<String>,
    
    /// Port for webhook HTTP server
    pub webhook_port: u16,
    
    /// Webhook secret token for security (optional)
    pub webhook_secret: Option<String>,
}

impl Config {
    /// Load configuration from environment variables
    /// 
    /// # Errors
    /// Returns an error if any required environment variable is missing
    pub fn from_env() -> Result<Self> {
        // Load .env file if it exists
        dotenv::dotenv().ok();
        
        Ok(Config {
            telegram_token: env::var("TELEGRAM_BOT_TOKEN")
                .context("TELEGRAM_BOT_TOKEN must be set")?,
            
            openai_api_key: env::var("OPENAI_API_KEY")
                .context("OPENAI_API_KEY must be set")?,
            
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set")?,
            
            embeddings_table: env::var("EMBEDDINGS_TABLE")
                .unwrap_or_else(|_| "document_embeddings".to_string()),
            
            embedding_model: env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-ada-002".to_string()),
            
            gpt_model: env::var("GPT_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            
            max_conversation_history: env::var("MAX_CONVERSATION_HISTORY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            
            top_k_chunks: env::var("TOP_K_CHUNKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            
            max_fallback_chunks: env::var("MAX_FALLBACK_CHUNKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            
            // Webhook configuration
            webhook_url: Self::detect_webhook_url(),
            webhook_port: env::var("WEBHOOK_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| {
                    // Default to PORT env var (Railway/Fly.io) or 8080
                    env::var("PORT")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(8080)
                }),
            webhook_secret: env::var("WEBHOOK_SECRET").ok(),
        })
    }
    
    /// Auto-detect webhook URL from cloud platform environment variables
    fn detect_webhook_url() -> Option<String> {
        // Check if explicitly set (highest priority)
        if let Ok(url) = env::var("WEBHOOK_URL") {
            if !url.is_empty() {
                // Ensure URL has https:// prefix if it's just a domain
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Some(format!("https://{}", url));
                }
                return Some(url);
            }
        }
        
        // Railway provides RAILWAY_PUBLIC_DOMAIN
        if let Ok(domain) = env::var("RAILWAY_PUBLIC_DOMAIN") {
            return Some(format!("https://{}", domain));
        }
        
        // Railway also provides RAILWAY_STATIC_URL for public networking
        if let Ok(url) = env::var("RAILWAY_STATIC_URL") {
            if !url.is_empty() {
                // Ensure it has https://
                if url.starts_with("https://") {
                    return Some(url);
                } else {
                    return Some(format!("https://{}", url));
                }
            }
        }
        
        // Fly.io provides FLY_APP_NAME
        if let Ok(app_name) = env::var("FLY_APP_NAME") {
            return Some(format!("https://{}.fly.dev", app_name));
        }
        
        // Heroku provides DYNO but no direct URL, would need to set WEBHOOK_URL
        // For local development, return None (will use polling)
        None
    }
    
    /// Validate that all required services are accessible
    pub async fn validate(&self) -> Result<()> {
        log::info!("Validating configuration...");
        
        // Check if using connection pooler (pgBouncer)
        let use_pooler = self.database_url.contains(":6543") || self.database_url.contains("pgbouncer=true");
        
        // Test Postgres connection
        let mut pool_options = sqlx::postgres::PgPoolOptions::new()
            .max_connections(20);
        
        // Disable prepared statements for connection poolers
        if use_pooler {
            log::info!("Detected connection pooler - disabling prepared statements");
            pool_options = pool_options.after_connect(|conn, _meta| {
                Box::pin(async move {
                    // Disable prepared statements for this connection
                    sqlx::query("SET statement_timeout = 0")
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            });
        }
        
        let pool = pool_options
            .connect(&self.database_url)
            .await
            .context("Failed to connect to PostgreSQL database")?;
        
        // Test query
        sqlx::query("SELECT 1")
            .fetch_one(&pool)
            .await
            .context("Database connection test query failed")?;
        
        log::info!("Configuration validated successfully");
        Ok(())
    }
}

