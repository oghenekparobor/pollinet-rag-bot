/// Pollinet Knowledge Bot
/// 
/// A Telegram bot that uses RAG (Retrieval-Augmented Generation) to answer
/// questions about Pollinet using official documentation and knowledge base.
/// 
/// The bot:
/// - Responds to mentions and keyword "Pollinet" in group chats
/// - Uses vector database (Qdrant) for semantic search
/// - Generates contextual answers using GPT-4o-mini
/// - Maintains conversation history for better context
/// - Never hallucinates - only answers from retrieved context
/// - Includes HTTP server for Twitter sync endpoints

use anyhow::Result;
use pollinet_knowledge_bot::{bot, config, http_server, rag, scheduler};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    pretty_env_logger::init();
    
    log::info!("Starting Pollinet Knowledge Bot...");

    // Load configuration from environment
    let cfg = config::Config::from_env()?;
    
    // Validate configuration and connections
    cfg.validate().await?;

    // Initialize RAG system (needed for both bot and HTTP server)
    let rag_system = Arc::new(rag::RAGSystem::new(cfg.clone()).await?);
    rag_system.initialize_collection().await?;

    // Create HTTP server state if HTTP server is enabled
    let http_state = if cfg.http_port > 0 {
        Some(http_server::AppState {
            config: cfg.clone(),
            rag_system: rag_system.clone(),
            sync_status: Arc::new(tokio::sync::RwLock::new(
                http_server::SyncStatus::default(),
            )),
        })
    } else {
        None
    };

    // Run bot, HTTP server, and scheduler concurrently
    if let Some(state) = http_state {
        let port = cfg.http_port;
        log::info!("Starting HTTP server on port {}...", port);
        
        // Start scheduler if Twitter API is configured
        let scheduler_handle = if cfg.twitter_api_key.is_some() {
            let scheduler_config = cfg.clone();
            let scheduler_rag = rag_system.clone();
            Some(tokio::spawn(async move {
                // Run sync twice weekly (every 84 hours = 3.5 days)
                // This equals ~4 syncs per month, well within free tier limits
                // Free tier: 100 requests/month, Basic: 15,000 requests/month
                if let Err(e) = scheduler::start_scheduler(scheduler_config, scheduler_rag, 84).await {
                    log::error!("Scheduler error: {}", e);
                }
            }))
        } else {
            None
        };
        
        tokio::select! {
            bot_result = bot::run_bot_with_rag(cfg.clone(), rag_system.clone()) => {
                bot_result?;
            }
            server_result = http_server::start_server(state, port) => {
                server_result?;
            }
            _ = async {
                if let Some(handle) = scheduler_handle {
                    handle.await.ok();
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_secs(86400)).await;
                }
            } => {}
        }
    } else {
        // Run bot only (no HTTP server)
        bot::run_bot_with_rag(cfg, rag_system).await?;
    }

    Ok(())
}
