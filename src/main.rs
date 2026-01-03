/// Pollinet Knowledge Bot
/// 
/// A Telegram bot that uses RAG (Retrieval-Augmented Generation) to answer
/// questions about Pollinet using official documentation and knowledge base.
/// 
/// The bot:
/// - Responds to mentions and keyword "Pollinet" in group chats
/// - Uses PostgreSQL with pgvector for semantic search
/// - Generates contextual answers using GPT-4o-mini
/// - Maintains conversation history for better context
/// - Never hallucinates - only answers from retrieved context

use anyhow::Result;
use pollinet_knowledge_bot::{bot, config, rag};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    pretty_env_logger::init();
    
    // Set up panic handler to log panics
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("💥 PANIC: {:?}", panic_info);
        eprintln!("PANIC: {:?}", panic_info);
    }));
    
    log::info!("Starting Pollinet Knowledge Bot...");
    log::info!("Environment: PORT={:?}", std::env::var("PORT").ok());

    // Load configuration from environment
    let cfg = match config::Config::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("Failed to load configuration: {}", e);
            eprintln!("Configuration error: {}", e);
            return Err(e);
        }
    };
    
    // Validate configuration and connections
    if let Err(e) = cfg.validate().await {
        log::error!("Configuration validation failed: {}", e);
        eprintln!("Validation error: {}", e);
        return Err(e);
    }

    // Initialize RAG system
    let rag_system = match rag::RAGSystem::new(cfg.clone()).await {
        Ok(rag) => Arc::new(rag),
        Err(e) => {
            log::error!("Failed to initialize RAG system: {}", e);
            eprintln!("RAG initialization error: {}", e);
            return Err(e);
        }
    };
    
    if let Err(e) = rag_system.initialize_collection().await {
        log::error!("Failed to initialize collection: {}", e);
        eprintln!("Collection initialization error: {}", e);
        return Err(e);
    }

    log::info!("✅ All systems initialized, starting bot...");
    
    // Run bot (this should block forever for webhook mode)
    if let Err(e) = bot::run_bot_with_rag(cfg, rag_system).await {
        log::error!("Bot error: {}", e);
        eprintln!("Bot error: {}", e);
        return Err(e);
    }

    Ok(())
}
