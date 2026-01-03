/// Telegram bot module
/// 
/// This module sets up and runs the Telegram bot using the teloxide framework.
/// It connects all the pieces: configuration, RAG system, handlers, and conversation management.

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use teloxide::{prelude::*, types::Me, utils::command::BotCommands};
use tokio::time::sleep;
use reqwest;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};

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

    // Detect if running on Railway or cloud platform
    let is_railway = std::env::var("RAILWAY_ENVIRONMENT").is_ok() 
        || std::env::var("RAILWAY_PROJECT_ID").is_ok();
    let is_cloud = is_railway 
        || std::env::var("DYNO").is_ok() // Heroku
        || std::env::var("FLY_APP_NAME").is_ok(); // Fly.io
    
    if is_cloud {
        log::info!("Detected cloud hosting environment - using optimized configuration");
    }
    
    // Create bot instance with custom client optimized for cloud hosting
    // Railway and other cloud platforms may have network restrictions
    log::info!("Creating HTTP client with extended timeouts (60s request, 30s connect)...");
    
    // Build client with cloud-optimized settings
    let mut client_builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(60)) // 60 second timeout for slow connections
        .connect_timeout(Duration::from_secs(30)) // 30 second connection timeout
        .tcp_keepalive(Duration::from_secs(60)) // Keep connection alive
        .pool_idle_timeout(Duration::from_secs(90)) // Keep connections in pool
        .pool_max_idle_per_host(10); // Allow connection pooling
    
    // For cloud platforms, add additional settings
    if is_cloud {
        log::info!("Applying cloud-specific network optimizations...");
        // Note: Railway's network may have issues with HTTP/2 (frame size errors)
        // We avoid forcing HTTP/2 by not calling http2_prior_knowledge()
        // This allows reqwest to negotiate HTTP/1.1 which is more compatible
        client_builder = client_builder
            .danger_accept_invalid_certs(false); // Keep security
        // Note: We don't call http2_prior_knowledge() to avoid HTTP/2 issues on Railway
    }
    
    let client = client_builder
        .build()
        .context("Failed to create HTTP client")?;
    
    // Test connection first (only log, don't fail)
    if is_cloud {
        log::info!("Testing outbound connection to Telegram API...");
        match client.get("https://api.telegram.org").send().await {
            Ok(resp) => {
                log::info!("✓ Outbound connection test successful (status: {})", resp.status());
            }
            Err(e) => {
                log::warn!("⚠ Outbound connection test failed: {}", e);
                log::warn!("This might indicate Railway network restrictions.");
                log::warn!("The bot will still attempt to connect with retries...");
            }
        }
    }
    
    log::info!("Creating bot instance with custom client...");
    let bot = Bot::with_client(&config.telegram_token, client);

    // Check if we're using webhooks - if so, we can skip get_me() initially
    // Telegram will verify the bot when we set the webhook
    let is_webhook_mode = config.webhook_url.is_some();
    
    // Get bot info with retry logic for network issues
    // For webhook mode, skip this - we'll get it when setting the webhook (which also needs outbound connection)
    let me = if is_webhook_mode {
        log::info!("Webhook mode detected - skipping initial bot info retrieval");
        log::info!("Bot info will be retrieved when setting webhook (requires same outbound connection)");
        // Try a quick single attempt, but don't fail if it doesn't work
        match bot.get_me().await {
            Ok(me) => {
                log::info!("Bot info retrieved: @{}", me.username());
                me
            }
            Err(e) => {
                log::warn!("Could not get bot info initially (this is expected if Railway blocks outbound connections)");
                log::warn!("Error: {}. Will attempt during webhook setup.", e);
                // We'll get it in run_webhook_server - for now return an error with helpful message
                anyhow::bail!(
                    "Cannot reach Telegram API. For Railway webhook mode:\n\
                    \n\
                    Railway may be blocking outbound connections. Try:\n\
                    1. Check Railway service settings - ensure outbound connections are allowed\n\
                    2. Verify Railway network policies allow connections to api.telegram.org\n\
                    3. Check if Railway requires a specific network configuration\n\
                    4. Try restarting the Railway service\n\
                    \n\
                    Note: Setting webhook also requires outbound connection, so if this fails,\n\
                    the webhook setup will also fail with the same network issue.\n\
                    \n\
                    Error: {}", e
                );
            }
        }
    } else {
        // For polling mode, we must get bot info
        retry_get_me(&bot).await
            .context("Failed to connect to Telegram API after multiple retries")?
    };
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

    // Build dispatcher (used for both modes)
    let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![
            rag_system.clone(),
            conversation_manager.clone(),
            me.clone()
        ])
        .enable_ctrlc_handler()
        .build();

    // Check if we should use webhooks or polling
    if let Some(webhook_url) = &config.webhook_url {
        log::info!("Using webhook mode");
        run_webhook_server(
            bot, 
            config.clone(), 
            webhook_url,
            rag_system, 
            conversation_manager, 
            me
        ).await?;
    } else {
        log::info!("Using polling mode (no webhook URL configured)");
        log::info!("Bot is running. Press Ctrl+C to stop.");
        dispatcher.dispatch().await;
    }

    Ok(())
}

/// Retry getting bot info with exponential backoff
async fn retry_get_me(bot: &Bot) -> Result<Me> {
    let max_retries = 5;
    let mut delay = Duration::from_secs(2);

    log::info!("Attempting to connect to Telegram API...");

    for attempt in 1..=max_retries {
        match bot.get_me().await {
            Ok(me) => {
                log::info!("Successfully connected to Telegram API on attempt {}", attempt);
                return Ok(me);
            }
            Err(e) => {
                let error_str = format!("{}", e);
                
                if attempt == max_retries {
                    log::error!(
                        "Failed to connect to Telegram API after {} attempts.",
                        max_retries
                    );
                    log::error!("Last error: {}", e);
                    log::error!("\nTroubleshooting tips:");
                    log::error!("1. Check your internet connection");
                    log::error!("2. Verify Telegram API is accessible: https://api.telegram.org");
                    log::error!("3. Check firewall/proxy settings");
                    log::error!("4. If using VPN, try disabling it");
                    log::error!("5. Check if your hosting provider blocks outbound connections");
                    anyhow::bail!(
                        "Failed to connect to Telegram API after {} attempts: {}",
                        max_retries,
                        e
                    );
                }
                
                // Provide more specific guidance based on error type
                // Check if running on Railway/cloud
                let is_railway = std::env::var("RAILWAY_ENVIRONMENT").is_ok() 
                    || std::env::var("RAILWAY_PROJECT_ID").is_ok();
                
                if error_str.contains("TimedOut") || error_str.contains("timeout") {
                    if is_railway {
                        log::warn!(
                            "Connection timeout on Railway (attempt {}/{}).\n\
                            Railway-specific troubleshooting:\n\
                            1. Check Railway logs for network errors\n\
                            2. Verify outbound connections are allowed\n\
                            3. Check if Railway service has network restrictions\n\
                            4. Try restarting the Railway service\n\
                            Retrying in {:?}...",
                            attempt,
                            max_retries,
                            delay
                        );
                    } else {
                        log::warn!(
                            "Connection timeout (attempt {}/{}). This usually indicates:\n\
                            - Slow or unstable network connection\n\
                            - Firewall blocking outbound connections\n\
                            - VPN or proxy issues\n\
                            Retrying in {:?}...",
                            attempt,
                            max_retries,
                            delay
                        );
                    }
                } else if error_str.contains("Connect") || error_str.contains("connection") {
                    if is_railway {
                        log::warn!(
                            "Connection error on Railway (attempt {}/{}).\n\
                            This might indicate:\n\
                            - Railway network restrictions\n\
                            - Outbound connection blocking\n\
                            - DNS resolution issues on Railway\n\
                            Retrying in {:?}...",
                            attempt,
                            max_retries,
                            delay
                        );
                    } else {
                        log::warn!(
                            "Connection error (attempt {}/{}). This usually indicates:\n\
                            - Network connectivity issues\n\
                            - DNS resolution problems\n\
                            - Telegram API temporarily unavailable\n\
                            Retrying in {:?}...",
                            attempt,
                            max_retries,
                            delay
                        );
                    }
                } else {
                    log::warn!(
                        "Failed to connect to Telegram API (attempt {}/{}): {}\n\
                        Retrying in {:?}...",
                        attempt,
                        max_retries,
                        e,
                        delay
                    );
                }
                
                sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
        }
    }
    
    unreachable!()
}

/// Run bot with webhook server
async fn run_webhook_server(
    bot: Bot,
    config: Config,
    webhook_url: &str,
    rag_system: Arc<RAGSystem>,
    conversation_manager: Arc<ConversationManager>,
    _me: Me, // May be invalid if initial get_me() failed - we'll get fresh one here
) -> Result<()> {
    let webhook_path = format!("{}/webhook", webhook_url);
    let addr = format!("0.0.0.0:{}", config.webhook_port);
    
    log::info!("Setting webhook URL: {}", webhook_path);
    
    // Verify bot info now (setting webhook also needs outbound connection)
    // Always get fresh bot info here to verify connectivity
    log::info!("Verifying bot token and network connectivity...");
    let verified_me = match bot.get_me().await {
        Ok(new_me) => {
            log::info!("✓ Bot verified: @{}", new_me.username());
            new_me
        }
        Err(e) => {
            log::error!("✗ Failed to verify bot token - cannot reach Telegram API");
            log::error!("This indicates Railway is blocking outbound connections to api.telegram.org");
            anyhow::bail!(
                "Cannot set webhook - Railway is blocking outbound connections.\n\
                \n\
                Solutions:\n\
                1. Check Railway service network settings - ensure outbound connections are enabled\n\
                2. Verify Railway allows connections to api.telegram.org (port 443)\n\
                3. Check Railway firewall/security group settings\n\
                4. Try using Railway's public networking feature if available\n\
                5. Consider using a different hosting provider if Railway blocks outbound connections\n\
                \n\
                Error: {}", e
            );
        }
    };
    
    // Set webhook with Telegram
    let mut set_webhook = bot.set_webhook(webhook_path.parse()?);
    
    // Add secret token if configured
    if let Some(secret) = &config.webhook_secret {
        set_webhook = set_webhook.secret_token(secret.clone());
    }
    
    set_webhook
        .await
        .context("Failed to set webhook with Telegram")?;
    
    log::info!("Webhook set successfully");
    
    // Create a channel to send updates from webhook handler to processing task
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Update>();
    
    // Spawn task to process updates - manually route to handlers
    let bot_clone = bot.clone();
    let rag_clone = rag_system.clone();
    let conv_clone = conversation_manager.clone();
    let me_clone = verified_me.clone();
    tokio::spawn(async move {
        log::info!("🔄 Webhook update processor started");
        while let Some(update) = rx.recv().await {
            log::info!("🔄 Processing update from webhook: ID={:?}", update.id);
            if let Err(e) = process_webhook_update(
                bot_clone.clone(),
                update,
                rag_clone.clone(),
                conv_clone.clone(),
                me_clone.clone(),
            ).await {
                log::error!("❌ Error processing webhook update: {:?}", e);
            } else {
                log::info!("✓ Update processed successfully");
            }
        }
    });
    
    // Create shared state for the HTTP server
    let state = AppState {
        update_tx: tx,
    };
    
    // Build the router
    let app = Router::new()
        .route("/webhook", post(webhook_handler))
        .route("/health", get(health_check))
        .with_state(state);
    
    log::info!("Starting webhook server on {}", addr);
    log::info!("Health check available at: http://{}/health", addr);
    log::info!("Webhook endpoint: http://{}/webhook", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("Failed to bind to {}", addr))?;
    
    // Start the HTTP server
    axum::serve(listener, app)
        .await
        .context("Webhook server error")?;
    
    Ok(())
}

/// Application state shared across HTTP handlers
#[derive(Clone)]
struct AppState {
    update_tx: tokio::sync::mpsc::UnboundedSender<Update>,
}

/// Handle incoming webhook updates from Telegram
async fn webhook_handler(
    State(state): State<AppState>,
    body: axum::body::Body,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    log::info!("📥 Received webhook update from Telegram");
    
    // Read the body
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to read webhook body: {}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Failed to read request body"})),
            ));
        }
    };
    
    log::debug!("Webhook body size: {} bytes", bytes.len());
    
    // Parse the update
    let update: Update = match serde_json::from_slice::<Update>(bytes.as_ref()) {
        Ok(update) => {
            log::info!("✓ Successfully parsed update ID: {:?}", update.id);
            update
        }
        Err(e) => {
            log::error!("Failed to parse webhook update: {}", e);
            let preview_len = bytes.len().min(500);
            log::error!("Raw body (first {} chars): {}", preview_len, String::from_utf8_lossy(&bytes[..preview_len]));
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid update format"})),
            ));
        }
    };
    
    // Send update to processing channel
    if let Err(e) = state.update_tx.send(update) {
        log::error!("Failed to send update to processing channel: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to queue update"})),
        ));
    }
    
    log::info!("✓ Update queued for processing");
    Ok(StatusCode::OK)
}

/// Process webhook update by manually routing to appropriate handlers
async fn process_webhook_update(
    bot: Bot,
    update: Update,
    rag_system: Arc<RAGSystem>,
    conversation_manager: Arc<ConversationManager>,
    me: Me,
) -> Result<()> {
    log::debug!("Processing update: ID={:?}, kind={:?}", update.id, update.kind);
    
    // Handle different update types using pattern matching
    match update.kind {
        teloxide::types::UpdateKind::Message(msg) => {
            log::info!("📨 Received message update");
            // Check if it's a command
            if let Some(cmd) = msg.text().and_then(|t| {
                if t.starts_with('/') {
                    t.split_whitespace().next().map(|s| s.trim_start_matches('/'))
                } else {
                    None
                }
            }) {
                match cmd.to_lowercase().as_str() {
                    "start" => handle_start_command(bot, msg).await?,
                    "help" => handle_help_command(bot, msg).await?,
                    "clear" => handle_clear_command(bot, msg, conversation_manager).await?,
                    _ => {
                        // Try to handle as regular message
                        if let Err(e) = handle_message(bot, msg, me, rag_system, conversation_manager).await {
                            log::error!("Error handling message: {:?}", e);
                        }
                    }
                }
            } else {
                // Regular message
                if let Err(e) = handle_message(bot, msg, me, rag_system, conversation_manager).await {
                    log::error!("Error handling message: {:?}", e);
                }
            }
        }
        teloxide::types::UpdateKind::EditedMessage(msg) => {
            log::info!("✏️ Received edited message update");
            // Handle edited messages
            if let Err(e) = handle_edited_message(bot, msg, me, rag_system, conversation_manager).await {
                log::error!("Error handling edited message: {:?}", e);
            }
        }
        other => {
            log::debug!("Ignoring update type: {:?} for update ID: {:?}", other, update.id);
        }
    }
    
    Ok(())
}

/// Health check endpoint
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "pollinet_knowledge_bot"
    }))
}

/// Initialize and run the Telegram bot (creates its own RAG system)
pub async fn run_bot(config: Config) -> Result<()> {
    log::info!("Initializing bot...");

    // Initialize the RAG system
    let rag_system = Arc::new(RAGSystem::new(config.clone()).await?);
    
    // Initialize the database collection
    rag_system.initialize_collection().await?;

    // Run with the RAG system
    run_bot_with_rag(config, rag_system).await
}

