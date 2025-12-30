/// Telegram bot module
/// 
/// This module sets up and runs the Telegram bot using the teloxide framework.
/// It connects all the pieces: configuration, RAG system, handlers, and conversation management.

use anyhow::Result;
use std::sync::Arc;
use teloxide::{prelude::*, types::Me, utils::command::BotCommands};

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

/// Initialize and run the Telegram bot
pub async fn run_bot(config: Config) -> Result<()> {
    log::info!("Initializing bot...");

    // Initialize the RAG system
    let rag_system = Arc::new(RAGSystem::new(config.clone()).await?);
    
    // Initialize the Qdrant collection
    rag_system.initialize_collection().await?;

    // Initialize conversation manager
    let conversation_manager = Arc::new(ConversationManager::new(
        config.max_conversation_history * 2, // Store both user and assistant messages
    ));

    // Create bot instance
    let bot = Bot::new(&config.telegram_token);

    // Get bot info
    let me = bot.get_me().await?;
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
    
    // Start the dispatcher
    dispatcher.dispatch().await;

    Ok(())
}

