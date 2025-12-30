/// Message handlers module
/// 
/// This module handles:
/// - Message routing logic
/// - Determining when bot should respond (mentions, keywords)
/// - Managing conversation history per chat
/// - Coordinating between Telegram and RAG system

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{prelude::*, types::{Me, ParseMode}};
use tokio::sync::RwLock;

use crate::rag::{ConversationMessage, RAGSystem};

/// Manages conversation history for multiple chats
pub struct ConversationManager {
    /// Maps chat_id to conversation history
    conversations: Arc<RwLock<HashMap<i64, Vec<ConversationMessage>>>>,
    max_history: usize,
}

impl ConversationManager {
    pub fn new(max_history: usize) -> Self {
        Self {
            conversations: Arc::new(RwLock::new(HashMap::new())),
            max_history,
        }
    }

    /// Add a user message to conversation history
    pub async fn add_user_message(&self, chat_id: i64, message: String) {
        let mut conversations = self.conversations.write().await;
        let history = conversations.entry(chat_id).or_insert_with(Vec::new);
        
        history.push(ConversationMessage {
            role: "user".to_string(),
            content: message,
        });

        // Trim history if it exceeds max
        if history.len() > self.max_history {
            let start = history.len() - self.max_history;
            *history = history[start..].to_vec();
        }
    }

    /// Add an assistant message to conversation history
    pub async fn add_assistant_message(&self, chat_id: i64, message: String) {
        let mut conversations = self.conversations.write().await;
        let history = conversations.entry(chat_id).or_insert_with(Vec::new);
        
        history.push(ConversationMessage {
            role: "assistant".to_string(),
            content: message,
        });

        // Trim history if it exceeds max
        if history.len() > self.max_history {
            let start = history.len() - self.max_history;
            *history = history[start..].to_vec();
        }
    }

    /// Get conversation history for a chat
    pub async fn get_history(&self, chat_id: i64) -> Vec<ConversationMessage> {
        let conversations = self.conversations.read().await;
        conversations
            .get(&chat_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Clear conversation history for a chat
    pub async fn clear_history(&self, chat_id: i64) {
        let mut conversations = self.conversations.write().await;
        conversations.remove(&chat_id);
    }
}

/// Check if the bot should respond to a message
/// 
/// Bot responds when:
/// 1. It is mentioned/tagged in the message
/// 2. Message contains the keyword "Pollinet" (case-insensitive)
/// 3. It's a private chat (not a group)
/// 4. Message is a reply to the bot's message
pub fn should_respond(bot_username: &str, message: &Message, bot_id: teloxide::types::UserId) -> bool {
    // Always respond in private chats
    if message.chat.is_private() {
        return true;
    }

    // Check if this is a reply to the bot's message
    if let Some(reply_to) = message.reply_to_message() {
        if let Some(from) = reply_to.from() {
            if from.id == bot_id {
                log::debug!("Responding to reply to bot message");
                return true;
            }
        }
    }

    // In group chats, check for mentions or keywords
    if let Some(text) = message.text() {
        let text_lower = text.to_lowercase();
        let bot_username_lower = bot_username.to_lowercase();
        
        // Check for bot mention (e.g., @bot_name)
        if text_lower.contains(&format!("@{}", bot_username_lower)) {
            return true;
        }
        
        // Check for "Pollinet" keyword
        if text_lower.contains("pollinet") {
            return true;
        }
    }

    // Check if bot is mentioned in entities
    if let Some(entities) = message.entities() {
        for entity in entities {
            if matches!(entity.kind, teloxide::types::MessageEntityKind::Mention) {
                if let Some(text) = message.text() {
                    let start = entity.offset;
                    let end = start + entity.length;
                    if let Some(mention) = text.get(start..end) {
                        if mention.to_lowercase().contains(&bot_username.to_lowercase()) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Extract the actual query from a message by removing bot mentions
pub fn extract_query(bot_username: &str, text: &str) -> String {
    let bot_mention = format!("@{}", bot_username);
    
    // Remove bot mentions
    let query = text.replace(&bot_mention, "")
        .replace(&bot_mention.to_lowercase(), "");
    
    // Trim whitespace
    query.trim().to_string()
}

/// Handle edited messages - delegates to the main message handler
/// 
/// This allows users to edit their questions and get updated responses
pub async fn handle_edited_message(
    bot: Bot,
    msg: Message,
    me: Me,
    rag_system: Arc<RAGSystem>,
    conversation_manager: Arc<ConversationManager>,
) -> Result<()> {
    log::debug!("Handling edited message from chat {}", msg.chat.id);
    // Process edited messages the same way as new messages
    // The bot will respond with a fresh answer to the edited question
    handle_message(bot, msg, me, rag_system, conversation_manager).await
}

/// Main message handler
/// 
/// This function:
/// 1. Checks if bot should respond
/// 2. Extracts the query
/// 3. Retrieves conversation history
/// 4. Queries the RAG system
/// 5. Updates conversation history
/// 6. Sends the response
pub async fn handle_message(
    bot: Bot,
    msg: Message,
    me: Me,
    rag_system: Arc<RAGSystem>,
    conversation_manager: Arc<ConversationManager>,
) -> Result<()> {
    // Get the message text first for logging
    let text = match msg.text() {
        Some(t) => t,
        None => return Ok(()), // Ignore non-text messages
    };

    // Log all messages for debugging (you can remove this later)
    log::debug!(
        "Received message in chat {} (type: {:?}): {}",
        msg.chat.id,
        if msg.chat.is_private() { "private" } else { "group" },
        text
    );

    // Check if we should respond to this message
    if !should_respond(&me.username(), &msg, me.id) {
        log::debug!("Skipping message (no mention/keyword/reply)");
        return Ok(());
    }

    // Extract the actual query
    let query = extract_query(&me.username(), text);
    
    if query.is_empty() {
        log::debug!("Query is empty after removing mentions");
        return Ok(());
    }

    log::info!("Received query from chat {}: {}", msg.chat.id, query);

    // Send "typing" action to indicate bot is processing
    bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
        .await?;

    // Get conversation history
    let chat_id = msg.chat.id.0;
    let history = conversation_manager.get_history(chat_id).await;

    // Add user message to history
    conversation_manager
        .add_user_message(chat_id, query.clone())
        .await;

    // Query the RAG system
    let response = match rag_system.query(&query, &history).await {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Error querying RAG system: {}", e);
            "Sorry, I encountered an error while processing your request. Please try again.".to_string()
        }
    };

    // Add assistant response to history
    conversation_manager
        .add_assistant_message(chat_id, response.clone())
        .await;

    // Send the response with HTML formatting
    bot.send_message(msg.chat.id, response)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle the /start command
pub async fn handle_start_command(bot: Bot, msg: Message) -> Result<()> {
    let welcome_message = "👋 <b>Hello! I'm the Pollinet Knowledge Bot.</b>\n\n\
        I can answer questions about Pollinet based on the official documentation.\n\n\
        <b>How to use me:</b>\n\
        • In private chats: Just send me your question\n\
        • In group chats: Mention me or include 'Pollinet' in your message\n\
        • You can also reply to my messages\n\n\
        I only provide information from the Pollinet knowledge base. \
        If I don't have the answer, I'll let you know!\n\n\
        Try asking me something about Pollinet!";

    bot.send_message(msg.chat.id, welcome_message)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle the /help command
pub async fn handle_help_command(bot: Bot, msg: Message) -> Result<()> {
    let help_message = "ℹ️ <b>Pollinet Knowledge Bot Help</b>\n\n\
        <b>Commands:</b>\n\
        /start - Welcome message and introduction\n\
        /help - Show this help message\n\
        /clear - Clear conversation history\n\n\
        <b>How I work:</b>\n\
        • I use Retrieval-Augmented Generation (RAG) to answer questions\n\
        • I search through Pollinet documents to find relevant information\n\
        • I remember our conversation to provide contextual answers\n\
        • I never make up information - if I don't know, I'll tell you\n\n\
        <b>Tips:</b>\n\
        • Ask specific questions for better answers\n\
        • You can ask follow-up questions and I'll remember the context\n\
        • In groups, mention me, say 'Pollinet', or reply to my messages\n\
        • You can edit your messages and I'll respond to the edited version";

    bot.send_message(msg.chat.id, help_message)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle the /clear command to reset conversation history
pub async fn handle_clear_command(
    bot: Bot,
    msg: Message,
    conversation_manager: Arc<ConversationManager>,
) -> Result<()> {
    let chat_id = msg.chat.id.0;
    conversation_manager.clear_history(chat_id).await;

    bot.send_message(
        msg.chat.id,
        "✅ <b>Conversation history cleared!</b> Starting fresh.",
    )
    .parse_mode(ParseMode::Html)
    .await?;

    Ok(())
}

