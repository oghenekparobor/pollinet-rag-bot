/// Scheduler Module
/// 
/// Handles automatic periodic syncing of Twitter content.

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::config::Config;
use crate::rag::RAGSystem;
use crate::twitter_sync::sync_tweets;

/// Start the automatic sync scheduler
/// 
/// Runs Twitter sync at regular intervals (default: twice weekly = 84 hours)
pub async fn start_scheduler(
    config: Config,
    rag_system: Arc<RAGSystem>,
    interval_hours: u64,
) -> Result<()> {
    let mut interval = time::interval(Duration::from_secs(interval_hours * 3600));
    
    let days = interval_hours as f64 / 24.0;
    let times_per_month = (30.0 / days).round() as u32;
    
    log::info!(
        "Starting Twitter sync scheduler (interval: {} hours = {:.1} days)",
        interval_hours,
        days
    );
    
    log::info!(
        "Sync will run approximately {} times per month",
        times_per_month
    );
    
    log::info!(
        "Note: Twitter API has rate limits. Free tier: 100 requests/month, Basic: 15,000/month"
    );

    // Skip the first tick (immediate execution)
    interval.tick().await;

    loop {
        interval.tick().await;

        log::info!("Automatic Twitter sync triggered");

        match sync_tweets(&config, rag_system.clone()).await {
            Ok(result) => {
                log::info!(
                    "Automatic sync complete: {} added, {} skipped",
                    result.tweets_added,
                    result.tweets_skipped
                );
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("429") || error_str.contains("Rate Limited") {
                    log::warn!(
                        "Rate limit hit during automatic sync. Will retry at next scheduled interval."
                    );
                } else {
                    log::error!("Automatic sync failed: {}", e);
                }
            }
        }
    }
}

