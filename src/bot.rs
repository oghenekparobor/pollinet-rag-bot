/// Telegram bot module
/// 
/// This module sets up and runs the Telegram bot using the teloxide framework.
/// It connects all the pieces: configuration, RAG system, handlers, and conversation management.

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use teloxide::{prelude::*, types::Me, utils::command::BotCommands};
use tokio::time::sleep;

use crate::config::Config;
use crate::handlers::{
    handle_clear_command, handle_edited_message, handle_help_command, handle_message, 
    handle_start_command, ConversationManager,
};
use crate::rag::RAGSystem;

/// Bot commands that users can use
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Supported commands:")]
pub enum Command {
    #[command(description = "Start the bot and see welcome message")]
    Start,
    #[command(description = "Show help information")]
    Help,
    #[command(description = "Clear conversation history")]
    Clear,
}

/// Initialize and run the Telegram bot with a pre-initialized RAG system
pub async fn run_bot_with_rag(config: Config, rag_system: Arc<RAGSystem>) -> Result<()> {
    log::info!("Initializing bot...");

    // Initialize conversation manager
    let conversation_manager = Arc::new(ConversationManager::new(
        config.max_conversation_history * 2, // Store both user and assistant messages
    ));

    // Create bot instance with custom client for better timeout handling
    let bot = Bot::new(&config.telegram_token);

    // Get bot info with retry logic for network issues
    let me = retry_get_me(&bot).await
        .context("Failed to connect to Telegram API after multiple retries")?;
    log::info!("Bot started: @{}", me.username());

    // Set up command handler
    let handler = dptree::entry()
        // Handle commands
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(
                    |bot: Bot, msg: Message, cmd: Command, conversation_manager: Arc<ConversationManager>| async move {
                        match cmd {
                            Command::Start => handle_start_command(bot, msg).await,
                            Command::Help => handle_help_command(bot, msg).await,
                            Command::Clear => handle_clear_command(bot, msg, conversation_manager).await,
                        }
                    },
                ),
        )
        // Handle regular messages
        .branch(
            Update::filter_message()
                .endpoint(
                    |bot: Bot, msg: Message, me: Me, rag_system: Arc<RAGSystem>, conversation_manager: Arc<ConversationManager>| async move {
                        if let Err(e) = handle_message(bot, msg, me, rag_system, conversation_manager).await {
                            log::error!("Error handling message: {:?}", e);
                        }
                        Ok(())
                    },
                ),
        )
        // Handle edited messages
        .branch(
            Update::filter_edited_message()
                .endpoint(
                    |bot: Bot, msg: Message, me: Me, rag_system: Arc<RAGSystem>, conversation_manager: Arc<ConversationManager>| async move {
                        if let Err(e) = handle_edited_message(bot, msg, me, rag_system, conversation_manager).await {
                            log::error!("Error handling edited message: {:?}", e);
                        }
                        Ok(())
                    },
                ),
        );

    // Create dispatcher
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![
            rag_system,
            conversation_manager,
            me.clone()
        ])
        .enable_ctrlc_handler()
        .build();

    log::info!("Bot is running. Press Ctrl+C to stop.");
    
    // Start the dispatcher - teloxide handles reconnections automatically
    // But we add better error logging for network issues
    dispatcher.dispatch().await;

    Ok(())
}

/// Retry getting bot info with exponential backoff
async fn retry_get_me(bot: &Bot) -> Result<Me> {
    let max_retries = 5;
    let mut delay = Duration::from_secs(2);

    for attempt in 1..=max_retries {
        match bot.get_me().await {
            Ok(me) => return Ok(me),
            Err(e) => {
                if attempt == max_retries {
                    anyhow::bail!("Failed to get bot info after {} attempts: {}", max_retries, e);
                }
                
                log::warn!(
                    "Failed to connect to Telegram API (attempt {}/{}): {}. Retrying in {:?}...",
                    attempt,
                    max_retries,
                    e,
                    delay
                );
                
                sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
        }
    }
    
    unreachable!()
}

/// Initialize and run the Telegram bot (creates its own RAG system)
pub async fn run_bot(config: Config) -> Result<()> {
    log::info!("Initializing bot...");

    // Initialize the RAG system
    let rag_system = Arc::new(RAGSystem::new(config.clone()).await?);
    
    // Initialize the Qdrant collection
    rag_system.initialize_collection().await?;

    // Run with the RAG system
    run_bot_with_rag(config, rag_system).await
}

