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
    
    log::info!("Starting Pollinet Knowledge Bot...");

    // Load configuration from environment
    let cfg = config::Config::from_env()?;
    
    // Validate configuration and connections
    cfg.validate().await?;

    // Initialize RAG system
    let rag_system = Arc::new(rag::RAGSystem::new(cfg.clone()).await?);
    rag_system.initialize_collection().await?;

    // Run bot
    bot::run_bot_with_rag(cfg, rag_system).await?;

    Ok(())
}
