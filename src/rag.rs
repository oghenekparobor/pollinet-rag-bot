/// RAG (Retrieval-Augmented Generation) module with PostgreSQL + pgvector
/// 
/// This module handles:
/// - Document chunking and embedding
/// - Vector storage in PostgreSQL with pgvector extension
/// - Semantic retrieval of relevant chunks
/// - Prompt building with context and conversation history
/// - GPT-4o-mini integration for response generation

use anyhow::{Context, Result};
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

use crate::config::Config;

/// Represents a chunk of a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
}

/// Represents a message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

/// OpenAI API request/response structures
#[derive(Debug, Serialize)]
struct OpenAIEmbeddingRequest {
    input: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<ConversationMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatChoice {
    message: ConversationMessage,
}

/// Main RAG system structure
pub struct RAGSystem {
    config: Config,
    db_pool: PgPool,
    http_client: reqwest::Client,
}

impl RAGSystem {
    /// Initialize the RAG system
    pub async fn new(config: Config) -> Result<Self> {
        // Check if using connection pooler (pgBouncer)
        let use_pooler = config.database_url.contains(":6543") || config.database_url.contains("pgbouncer=true");
        
        let mut pool_options = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10);
        
        // Disable prepared statements for connection poolers
        if use_pooler {
            log::info!("Using connection pooler - disabling prepared statements");
            pool_options = pool_options.after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("SET statement_timeout = 0")
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            });
        }
        
        let db_pool = pool_options
            .connect(&config.database_url)
            .await
            .context("Failed to connect to PostgreSQL")?;

        let http_client = reqwest::Client::new();

        Ok(Self {
            config,
            db_pool,
            http_client,
        })
    }

    /// Initialize the database table if it doesn't exist
    pub async fn initialize_collection(&self) -> Result<()> {
        log::info!("Initializing database table...");

        // Enable pgvector extension
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.db_pool)
            .await
            .context("Failed to create vector extension")?;

        // Create table for embeddings (1536 dimensions for text-embedding-ada-002)
        let create_table_query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                embedding vector(1536),
                metadata JSONB,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            self.config.embeddings_table
        );

        sqlx::query(&create_table_query)
            .execute(&self.db_pool)
            .await
            .context("Failed to create embeddings table")?;

        // Create index for vector similarity search
        let create_index_query = format!(
            r#"
            CREATE INDEX IF NOT EXISTS {}_embedding_idx 
            ON {} USING ivfflat (embedding vector_cosine_ops)
            WITH (lists = 100)
            "#,
            self.config.embeddings_table, self.config.embeddings_table
        );

        sqlx::query(&create_index_query)
            .execute(&self.db_pool)
            .await
            .context("Failed to create vector index")?;

        log::info!("Database table initialized successfully");
        Ok(())
    }

    /// Generate embeddings for text using OpenAI API
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = OpenAIEmbeddingRequest {
            input: text.to_string(),
            model: self.config.embedding_model.clone(),
        };

        let response = self
            .http_client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.config.openai_api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send embedding request")?;

        // Check HTTP status
        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            return Err(anyhow::anyhow!(
                "OpenAI API error (status {}): {}",
                status,
                error_text
            ));
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read response body")?;

        let response: OpenAIEmbeddingResponse = serde_json::from_str(&response_text)
            .context(format!(
                "Failed to parse embedding response. Response was: {}",
                response_text
            ))?;

        response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .context("No embedding returned")
    }

    /// Split text into chunks for embedding
    /// Simple chunking by character count with overlap
    fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let text = text.trim();
        
        if text.len() <= chunk_size {
            chunks.push(text.to_string());
            return chunks;
        }

        let mut start = 0;
        while start < text.len() {
            let end = (start + chunk_size).min(text.len());
            let chunk = &text[start..end];
            chunks.push(chunk.to_string());
            
            if end == text.len() {
                break;
            }
            
            start += chunk_size - overlap;
        }

        chunks
    }

    /// Add a document to the knowledge base
    /// 
    /// # Arguments
    /// * `document_name` - Name/identifier for the document
    /// * `content` - Full text content of the document
    /// * `metadata` - Additional metadata (e.g., source, date, etc.)
    pub async fn add_document(
        &self,
        document_name: &str,
        content: &str,
        metadata: HashMap<String, String>,
    ) -> Result<usize> {
        log::info!("Adding document: {}", document_name);

        // Chunk the document (1000 chars with 200 char overlap)
        let chunks = Self::chunk_text(content, 1000, 200);
        log::info!("Split into {} chunks", chunks.len());

        for (idx, chunk_text) in chunks.iter().enumerate() {
            // Generate embedding for this chunk
            let embedding = self.generate_embedding(chunk_text).await?;

            // Create point ID
            let point_id = format!("{}_{}", document_name, idx);

            // Prepare metadata
            let mut chunk_metadata = metadata.clone();
            chunk_metadata.insert("document".to_string(), document_name.to_string());
            chunk_metadata.insert("chunk_index".to_string(), idx.to_string());

            // Convert metadata to JSON
            let metadata_json = serde_json::to_value(&chunk_metadata)
                .context("Failed to serialize metadata")?;

            // Insert into database
            let insert_query = format!(
                r#"
                INSERT INTO {} (id, content, embedding, metadata)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (id) DO UPDATE 
                SET content = $2, embedding = $3, metadata = $4
                "#,
                self.config.embeddings_table
            );

            sqlx::query(&insert_query)
                .bind(&point_id)
                .bind(chunk_text)
                .bind(Vector::from(embedding))
                .bind(metadata_json)
                .execute(&self.db_pool)
                .await
                .context("Failed to insert embedding")?;
        }

        log::info!("Document added successfully with {} chunks", chunks.len());
        Ok(chunks.len())
    }

    /// Retrieve relevant document chunks for a query
    /// 
    /// # Arguments
    /// * `query` - User's question or query
    /// 
    /// # Returns
    /// Vector of relevant text chunks
    pub async fn retrieve_relevant_chunks(&self, query: &str) -> Result<Vec<String>> {
        log::info!("Retrieving relevant chunks for query: {}", query);

        // Generate embedding for the query
        let query_embedding = self.generate_embedding(query).await?;

        // Search for similar vectors using cosine similarity
        let search_query = format!(
            r#"
            SELECT content
            FROM {}
            ORDER BY embedding <=> $1
            LIMIT $2
            "#,
            self.config.embeddings_table
        );

        let rows = sqlx::query(&search_query)
            .bind(Vector::from(query_embedding))
            .bind(self.config.top_k_chunks as i64)
            .fetch_all(&self.db_pool)
            .await
            .context("Failed to search for similar vectors")?;

        let chunks: Vec<String> = rows
            .into_iter()
            .map(|row| row.get::<String, _>("content"))
            .collect();

        log::info!("Retrieved {} relevant chunks", chunks.len());
        Ok(chunks)
    }

    /// Generate a response using GPT-4o-mini with retrieved context
    /// 
    /// # Arguments
    /// * `query` - User's question
    /// * `context_chunks` - Retrieved relevant document chunks
    /// * `conversation_history` - Previous messages in the conversation
    /// 
    /// # Returns
    /// Generated response from GPT-4o-mini
    pub async fn generate_response(
        &self,
        query: &str,
        context_chunks: &[String],
        conversation_history: &[ConversationMessage],
    ) -> Result<String> {
        log::info!("Generating response using GPT-4o-mini");

        // Build context from retrieved chunks
        let context = if context_chunks.is_empty() {
            "No relevant information found in the knowledge base.".to_string()
        } else {
            context_chunks
                .iter()
                .enumerate()
                .map(|(i, chunk)| format!("[Context {}]\n{}", i + 1, chunk))
                .collect::<Vec<_>>()
                .join("\n\n")
        };

        // Build system message with instructions
        let system_message = ConversationMessage {
            role: "system".to_string(),
            content: format!(
                "You are a helpful knowledge base assistant for Pollinet. \
                Your role is to answer questions ONLY using the provided context from Pollinet documents. \
                \
                IMPORTANT RULES:\n\
                1. Answer questions using ONLY the information from the Context sections below.\n\
                2. If the answer is not in the provided context, respond EXACTLY with: \
                \"I don't have that information yet.\"\n\
                3. Never make assumptions or provide information not explicitly stated in the context.\n\
                4. Be concise and accurate.\n\
                5. You can use information from previous conversation to provide better context, \
                but only if it's based on the provided knowledge.\n\
                \n\
                Context from Pollinet documents:\n\
                {}\n\
                ---",
                context
            ),
        };

        // Build messages array: system + history + current query
        let mut messages = vec![system_message];
        
        // Add conversation history (limited to max_conversation_history)
        let history_start = conversation_history.len()
            .saturating_sub(self.config.max_conversation_history);
        messages.extend_from_slice(&conversation_history[history_start..]);
        
        // Add current query
        messages.push(ConversationMessage {
            role: "user".to_string(),
            content: query.to_string(),
        });

        // Call OpenAI API
        let request = OpenAIChatRequest {
            model: self.config.gpt_model.clone(),
            messages,
            temperature: 0.3, // Low temperature for factual responses
            max_tokens: 500,
        };

        let response = self
            .http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.openai_api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send chat completion request")?;

        let response: OpenAIChatResponse = response
            .json()
            .await
            .context("Failed to parse chat completion response")?;

        let answer = response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("No response from GPT")?;

        log::info!("Response generated successfully");
        Ok(answer)
    }

    /// Retrieve ALL documents from database (for comprehensive fallback context)
    async fn retrieve_all_documents(&self) -> Result<Vec<String>> {
        log::info!("Retrieving all documents for comprehensive context (limit: {})", 
                   self.config.max_fallback_chunks);

        let query = format!(
            "SELECT content FROM {} ORDER BY created_at LIMIT $1",
            self.config.embeddings_table
        );

        let rows = sqlx::query(&query)
            .bind(self.config.max_fallback_chunks as i64)
            .fetch_all(&self.db_pool)
            .await
            .context("Failed to retrieve all documents")?;

        let chunks: Vec<String> = rows
            .into_iter()
            .map(|row| row.get::<String, _>("content"))
            .collect();

        log::info!("Retrieved {} total chunks for context", chunks.len());
        Ok(chunks)
    }

    /// Generate a fallback response using ChatGPT with full knowledge base context
    /// Used when no relevant information is found via similarity search
    async fn generate_fallback_response(
        &self,
        query: &str,
        conversation_history: &[ConversationMessage],
    ) -> Result<String> {
        log::info!("Generating fallback response using ChatGPT with full Pollinet knowledge base");

        // Retrieve all documents for comprehensive context
        let all_chunks = self.retrieve_all_documents().await?;
        
        // Build comprehensive context
        let full_context = if all_chunks.is_empty() {
            "No documents in knowledge base yet.".to_string()
        } else {
            all_chunks.join("\n\n---\n\n")
        };

        // Build system message with full Pollinet knowledge base
        let system_message = ConversationMessage {
            role: "system".to_string(),
            content: format!(
                "You are a helpful assistant for Pollinet, a decentralized SDK enabling \
                offline Solana transactions via Bluetooth Low Energy (BLE) mesh networks. \
                \n\n\
                COMPLETE POLLINET KNOWLEDGE BASE:\n\
                {}\n\
                ---\n\n\
                When answering questions:\n\
                1. First try to answer using the knowledge base above\n\
                2. If the question is about Pollinet, blockchain, Solana, Web3, DePIN, or related crypto topics, \
                   answer using the knowledge base or your understanding of these topics\n\
                3. If the question is COMPLETELY UNRELATED (weather, cooking, sports, entertainment, general trivia, etc.), \
                   respond EXACTLY with: 'I'm sorry, but I only answer questions related to Pollinet, blockchain, \
                   Solana, and Web3 technologies. Please ask me something about Pollinet!'\n\
                4. If you're unsure whether a question is related, err on the side of answering if there's \
                   any connection to blockchain/crypto/technology\n\
                5. Keep responses concise and accurate",
                full_context
            ),
        };

        // Build messages array
        let mut messages = vec![system_message];
        
        // Add conversation history
        let history_start = conversation_history.len()
            .saturating_sub(self.config.max_conversation_history);
        messages.extend_from_slice(&conversation_history[history_start..]);
        
        // Add current query
        messages.push(ConversationMessage {
            role: "user".to_string(),
            content: query.to_string(),
        });

        // Call OpenAI API
        let request = OpenAIChatRequest {
            model: self.config.gpt_model.clone(),
            messages,
            temperature: 0.7, // Higher temperature for general responses
            max_tokens: 500,
        };

        let response = self
            .http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.openai_api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send chat completion request")?;

        let response: OpenAIChatResponse = response
            .json()
            .await
            .context("Failed to parse chat completion response")?;

        let answer = response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .context("No response from GPT")?;

        Ok(answer)
    }

    /// Main query method that combines retrieval and generation
    /// 
    /// # Arguments
    /// * `query` - User's question
    /// * `conversation_history` - Previous messages
    /// 
    /// # Returns
    /// Generated answer based on retrieved context or fallback to general ChatGPT
    pub async fn query(
        &self,
        query: &str,
        conversation_history: &[ConversationMessage],
    ) -> Result<String> {
        // Step 1: Retrieve relevant chunks
        let chunks = self.retrieve_relevant_chunks(query).await?;

        // Step 2: Check if we have relevant context
        if chunks.is_empty() {
            log::info!("No relevant chunks found, using ChatGPT fallback with full knowledge base");
            
            // Use ChatGPT with full knowledge base as fallback
            let fallback_response = self
                .generate_fallback_response(query, conversation_history)
                .await?;
            
            return Ok(fallback_response);
        }

        // Step 3: Generate response with context from knowledge base
        let response = self
            .generate_response(query, &chunks, conversation_history)
            .await?;

        // Check if GPT said it doesn't know
        if response.contains("I don't have that information yet") {
            log::info!("GPT couldn't answer from context, using ChatGPT fallback with full knowledge base");
            
            // Use ChatGPT with full knowledge base as fallback
            let fallback_response = self
                .generate_fallback_response(query, conversation_history)
                .await?;
            
            return Ok(fallback_response);
        }

        Ok(response)
    }
}
