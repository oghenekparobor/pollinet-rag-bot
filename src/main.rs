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

use anyhow::Result;
use pollinet_knowledge_bot::{bot, config};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    pretty_env_logger::init();
    
    log::info!("Starting Pollinet Knowledge Bot...");

    // Load configuration from environment
    let cfg = config::Config::from_env()?;
    
    // Validate configuration and connections
    cfg.validate().await?;

    // Run the bot
    bot::run_bot(cfg).await?;

    Ok(())
}
