/// Twitter Sync Module
/// 
/// Handles fetching tweets from X (Twitter), filtering, deduplication,
/// and adding them to the knowledge base.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::rag::RAGSystem;

// Global lock to prevent concurrent syncs
lazy_static::lazy_static! {
    static ref SYNC_LOCK: Mutex<()> = Mutex::new(());
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterTweet {
    id: String,
    text: String,
    author_id: Option<String>,
    created_at: Option<String>,
    public_metrics: Option<TwitterMetrics>,
    author: Option<TwitterUser>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterMetrics {
    like_count: Option<u32>,
    retweet_count: Option<u32>,
    reply_count: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterUser {
    username: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterResponse {
    data: Option<Vec<TwitterTweet>>,
    meta: Option<TwitterMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterMeta {
    result_count: Option<u32>,
    next_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub tweets_added: usize,
    pub tweets_skipped: usize,
    pub last_sync: DateTime<Utc>,
}

/// Sync tweets from Twitter/X API
/// 
/// Prevents concurrent syncs using a global mutex to avoid rate limit issues
pub async fn sync_tweets(
    config: &Config,
    rag_system: Arc<RAGSystem>,
) -> Result<SyncResult> {
    // Acquire lock to prevent concurrent syncs
    let _lock = SYNC_LOCK.lock().await;
    
    let api_key = config
        .twitter_api_key
        .as_ref()
        .context("TWITTER_API_KEY not set")?;
    
    let api_secret = config.twitter_api_secret.as_ref();

    log::info!("Starting Twitter sync (fetching latest 12 tweets from @sol_pollinet)...");

    // Fetch tweets using Twitter API v2 - only from @sol_pollinet
    let tweets = fetch_tweets(api_key, api_secret, "sol_pollinet").await?;

    log::info!("Fetched {} tweets from @sol_pollinet", tweets.len());

    // Filter tweets
    let filtered = filter_tweets(tweets);
    log::info!("Filtered to {} relevant tweets", filtered.len());

    // Check for duplicates and add new ones
    let mut added = 0;
    let mut skipped = 0;

    for tweet in filtered {
        if is_duplicate(&tweet, rag_system.clone()).await? {
            skipped += 1;
            continue;
        }

        // Categorize tweet with readable categories
        let (category, category_label) = categorize_tweet(&tweet);

        // Format tweet content for better readability
        let formatted_content = format_tweet_content(&tweet, &category_label);

        // Create metadata with readable information
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "twitter".to_string());
        metadata.insert("category".to_string(), category.clone());
        metadata.insert("category_label".to_string(), category_label.clone());
        metadata.insert("tweet_id".to_string(), tweet.id.clone());
        metadata.insert(
            "author".to_string(),
            tweet
                .author
                .as_ref()
                .map(|a| a.username.clone())
                .unwrap_or_else(|| "sol_pollinet".to_string()),
        );
        if let Some(created_at) = &tweet.created_at {
            metadata.insert("created_at".to_string(), created_at.clone());
            // Parse and format date for better readability
            if let Ok(parsed_date) = chrono::DateTime::parse_from_rfc3339(created_at) {
                metadata.insert(
                    "date_formatted".to_string(),
                    parsed_date.format("%B %d, %Y").to_string(),
                );
            }
        }
        if let Some(metrics) = &tweet.public_metrics {
            if let Some(likes) = metrics.like_count {
                metadata.insert("likes".to_string(), likes.to_string());
            }
            if let Some(retweets) = metrics.retweet_count {
                metadata.insert("retweets".to_string(), retweets.to_string());
            }
        }

        // Add to knowledge base with formatted content
        let doc_id = format!("tweet_{}", tweet.id);
        rag_system
            .add_document(&doc_id, &formatted_content, metadata)
            .await?;

        added += 1;
        log::debug!(
            "Added tweet {} to category: {} ({})",
            tweet.id,
            category_label,
            category
        );
    }

    log::info!(
        "Sync complete: {} added, {} skipped",
        added,
        skipped
    );

    Ok(SyncResult {
        tweets_added: added,
        tweets_skipped: skipped,
        last_sync: Utc::now(),
    })
}

/// Fetch tweets from Twitter API v2
/// 
/// Uses Bearer Token authentication (app-only)
/// According to X API docs: https://docs.x.com/x-api/introduction
async fn fetch_tweets(
    bearer_token: &str,
    _api_secret: Option<&String>,
    username: &str,
) -> Result<Vec<TwitterTweet>> {
    let client = reqwest::Client::new();

    // Search for tweets ONLY from the specified user (sol_pollinet)
    let query = format!("from:{}", username);
    let url = "https://api.twitter.com/2/tweets/search/recent";

    log::debug!("Fetching tweets with query: {} (max: 12)", query);
    log::debug!("Using Bearer Token authentication (token length: {})", bearer_token.len());

    let response = client
        .get(url)
        .header(
            "Authorization",
            format!("Bearer {}", bearer_token.trim()),
        )
        .header("Content-Type", "application/json")
        .query(&[
            ("query", query.as_str()),
            ("max_results", "12"),
            ("tweet.fields", "created_at,author_id,public_metrics"),
            ("expansions", "author_id"),
            ("user.fields", "username,name"),
        ])
        .send()
        .await
        .context("Failed to fetch tweets from Twitter API")?;

    // Check rate limit headers before processing response
    let rate_limit_remaining = response
        .headers()
        .get("x-rate-limit-remaining")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok());
    
    let rate_limit_reset = response
        .headers()
        .get("x-rate-limit-reset")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        
        // Provide helpful error messages based on status code
        let error_msg = match status.as_u16() {
            401 => {
                format!(
                    "Unauthorized (401): Invalid or missing Bearer Token.\n\
                    Please ensure:\n\
                    1. You have a valid Bearer Token from X Developer Portal\n\
                    2. Set it in .env as TWITTER_BEARER_TOKEN or TWITTER_API_KEY\n\
                    3. The token has not expired\n\
                    4. Your app has access to the Twitter API v2 endpoints\n\
                    \n\
                    Get your Bearer Token: https://developer.twitter.com/en/portal/dashboard\n\
                    API Response: {}",
                    text
                )
            }
            403 => {
                format!(
                    "Forbidden (403): Your Bearer Token doesn't have permission to access this endpoint.\n\
                    Make sure your app has access to Twitter API v2.\n\
                    API Response: {}",
                    text
                )
            }
            429 => {
                let reset_info = if let Some(reset_timestamp) = rate_limit_reset {
                    let reset_time = DateTime::<Utc>::from_timestamp(reset_timestamp, 0)
                        .unwrap_or_else(|| Utc::now());
                    let wait_seconds = (reset_timestamp - Utc::now().timestamp()).max(0);
                    format!(
                        "\n\nRate limit will reset at: {} (in approximately {} seconds / {} minutes)\n\
                        Please wait before retrying the sync.",
                        reset_time.format("%Y-%m-%d %H:%M:%S UTC"),
                        wait_seconds,
                        wait_seconds / 60
                    )
                } else {
                    "\n\nPlease wait 15 minutes before retrying.".to_string()
                };
                
                format!(
                    "Rate Limited (429): Too many requests.\n\
                    Twitter API has rate limits based on your tier:\n\
                    - Free tier: 100 requests/month\n\
                    - Basic tier: 15,000 requests/month\n\
                    \n\
                    This sync was blocked to prevent exceeding your limit.{}",
                    reset_info
                )
            }
            _ => {
                format!(
                    "Twitter API error: {} - {}",
                    status,
                    text
                )
            }
        };
        
        anyhow::bail!("{}", error_msg);
    }

    // Log rate limit information if available
    if let Some(remaining) = rate_limit_remaining {
        log::info!("Twitter API rate limit: {} requests remaining", remaining);
        if remaining < 5 {
            log::warn!("⚠️  Low rate limit remaining! Consider upgrading your Twitter API tier.");
        }
    }

    let twitter_response: TwitterResponse = response
        .json()
        .await
        .context("Failed to parse Twitter API response")?;

    let tweets = twitter_response.data.unwrap_or_default();

    // Note: In a full implementation, we'd parse the includes.users array
    // from the response to map author information to tweets
    // For now, tweets may have author_id but not full author details

    Ok(tweets)
}

/// Filter tweets based on quality and relevance
fn filter_tweets(tweets: Vec<TwitterTweet>) -> Vec<TwitterTweet> {
    tweets
        .into_iter()
        .filter(|tweet| {
            // Remove retweets (they start with "RT @")
            if tweet.text.starts_with("RT @") {
                return false;
            }

            // Remove very short tweets (likely spam)
            if tweet.text.len() < 50 {
                return false;
            }

            // Keep tweets from official account regardless of likes
            let is_official = tweet
                .author
                .as_ref()
                .map(|a| a.username == "sol_pollinet")
                .unwrap_or(false);

            if is_official {
                return true;
            }

            // For other tweets, require minimum engagement
            let likes = tweet
                .public_metrics
                .as_ref()
                .and_then(|m| m.like_count)
                .unwrap_or(0);

            likes >= 10
        })
        .collect()
}

/// Check if a tweet is already in the knowledge base
async fn is_duplicate(tweet: &TwitterTweet, rag_system: Arc<RAGSystem>) -> Result<bool> {
    // Try to retrieve chunks that might contain this tweet
    // We'll search by the tweet text to see if similar content exists
    let chunks = rag_system.retrieve_relevant_chunks(&tweet.text).await?;
    
    // Check if any chunk has the same tweet_id in metadata
    // For now, we'll do a simple text similarity check
    // In a production system, you'd query the database directly for tweet_id
    
    // Simple heuristic: if we find very similar content, it's likely a duplicate
    if !chunks.is_empty() {
        // Check if any chunk contains the tweet ID or very similar text
        for chunk in chunks {
            if chunk.contains(&tweet.id) || 
               (chunk.len() > 50 && tweet.text.len() > 50 && 
                chunk.chars().take(50).eq(tweet.text.chars().take(50))) {
                return Ok(true);
            }
        }
    }
    
    Ok(false)
}

/// Categorize a tweet based on its content and source
/// Returns (category_key, category_label) for better readability
fn categorize_tweet(tweet: &TwitterTweet) -> (String, String) {
    let text_lower = tweet.text.to_lowercase();
    let is_official = tweet
        .author
        .as_ref()
        .map(|a| a.username == "sol_pollinet")
        .unwrap_or(false);

    // Categorize based on content keywords
    if text_lower.contains("announcement")
        || text_lower.contains("announcing")
        || text_lower.contains("we're excited")
        || text_lower.contains("introducing")
    {
        return ("pollinet_announcement".to_string(), "Announcement".to_string());
    }

    if text_lower.contains("update")
        || text_lower.contains("updated")
        || text_lower.contains("updates")
        || text_lower.contains("changelog")
        || text_lower.contains("version")
    {
        return ("pollinet_updates".to_string(), "Update".to_string());
    }

    if text_lower.contains("news")
        || text_lower.contains("headline")
        || text_lower.contains("breaking")
        || text_lower.contains("report")
    {
        return ("pollinet_news".to_string(), "News".to_string());
    }

    if text_lower.contains("talk")
        || text_lower.contains("speaking")
        || text_lower.contains("presentation")
        || text_lower.contains("conference")
        || text_lower.contains("event")
        || text_lower.contains("webinar")
    {
        return ("pollinet_talks".to_string(), "Talk/Event".to_string());
    }

    if text_lower.contains("partnership")
        || text_lower.contains("collaboration")
        || text_lower.contains("integrated")
        || text_lower.contains("working with")
    {
        return ("pollinet_partnerships".to_string(), "Partnership".to_string());
    }

    if text_lower.contains("tutorial")
        || text_lower.contains("guide")
        || text_lower.contains("how to")
        || text_lower.contains("documentation")
        || text_lower.contains("docs")
    {
        return ("pollinet_information".to_string(), "Information/Guide".to_string());
    }

    // Default category for general information
    if is_official {
        return ("pollinet_information".to_string(), "Information".to_string());
    }

    ("pollinet_information".to_string(), "Information".to_string())
}

/// Format tweet content for better readability and understanding
fn format_tweet_content(tweet: &TwitterTweet, category_label: &str) -> String {
    let mut formatted = String::new();

    // Add category header
    formatted.push_str(&format!("📢 Pollinet {}: ", category_label));
    formatted.push_str("\n\n");

    // Add the tweet content
    formatted.push_str(&tweet.text);
    formatted.push_str("\n\n");

    // Add context information
    formatted.push_str("---\n");
    formatted.push_str("Source: @sol_pollinet (Official Pollinet Account)\n");

    // Add date if available
    if let Some(created_at) = &tweet.created_at {
        if let Ok(parsed_date) = chrono::DateTime::parse_from_rfc3339(created_at) {
            formatted.push_str(&format!(
                "Date: {}\n",
                parsed_date.format("%B %d, %Y at %I:%M %p UTC")
            ));
        }
    }

    // Add engagement metrics if available
    if let Some(metrics) = &tweet.public_metrics {
        let mut engagement = Vec::new();
        if let Some(likes) = metrics.like_count {
            if likes > 0 {
                engagement.push(format!("{} likes", likes));
            }
        }
        if let Some(retweets) = metrics.retweet_count {
            if retweets > 0 {
                engagement.push(format!("{} retweets", retweets));
            }
        }
        if !engagement.is_empty() {
            formatted.push_str(&format!("Engagement: {}\n", engagement.join(", ")));
        }
    }

    formatted.push_str("---\n");

    formatted
}

