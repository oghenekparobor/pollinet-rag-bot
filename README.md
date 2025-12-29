# Pollinet Knowledge Bot 🤖

A Telegram bot built in Rust that uses Retrieval-Augmented Generation (RAG) to answer questions about Pollinet based on official documentation and knowledge base.

## Features ✨

- **Smart Response Detection**: Responds when mentioned or when "Pollinet" keyword is detected in group chats
- **RAG Pipeline**: Uses semantic search with Qdrant vector database to find relevant information
- **GPT-4o-mini Integration**: Generates contextual, accurate answers using OpenAI's API
- **Conversation Memory**: Maintains conversation history for better contextual understanding
- **No Hallucination**: Only answers from retrieved context; admits when information is not available
- **Modular Architecture**: Clean, maintainable codebase with separation of concerns

## Architecture 🏗️

The project is structured into modular components:

- **`main.rs`**: Application entry point and initialization
- **`config.rs`**: Configuration management and environment variables
- **`bot.rs`**: Telegram bot setup and event loop using teloxide
- **`handlers.rs`**: Message routing, conversation management, and command handlers
- **`rag.rs`**: RAG pipeline including embedding generation, retrieval, and response generation

### How It Works

1. **Document Ingestion**: Documents are chunked and embedded using OpenAI's embedding model
2. **Storage**: Embeddings are stored in Qdrant vector database with metadata
3. **Query Processing**: User questions are embedded and used to retrieve relevant chunks
4. **Context Building**: Retrieved chunks + conversation history form the context
5. **Response Generation**: GPT-4o-mini generates accurate answers based solely on the context
6. **Memory**: Conversation is stored for follow-up questions

## Prerequisites 📋

- **Rust**: 1.70 or later
- **PostgreSQL**: With pgvector extension (or use free cloud service like Supabase)
- **OpenAI API Key**: For embeddings and GPT-4o-mini
- **Telegram Bot Token**: From [@BotFather](https://t.me/botfather)

## Installation 🚀

### 1. Clone the Repository

```bash
git clone <your-repo-url>
cd pollinet_knowledge_bot
```

### 2. Set Up PostgreSQL with pgvector

**Option A: Use Supabase (Easiest - Free Tier)**
1. Go to https://supabase.com and create a project
2. Get your connection string from Project Settings → Database
3. Enable the `vector` extension in Database → Extensions

**Option B: Local Docker**
```bash
docker run -d --name pollinet-postgres \
    -e POSTGRES_PASSWORD=password \
    -e POSTGRES_DB=pollinet_bot \
    -p 5432:5432 \
    pgvector/pgvector:pg16
```

**Option C: Local Installation**
```bash
# macOS
brew install postgresql@16 pgvector
brew services start postgresql@16

# Create database
createdb pollinet_bot
psql pollinet_bot -c "CREATE EXTENSION vector;"
```

See [POSTGRES_SETUP.md](POSTGRES_SETUP.md) for detailed instructions.

### 3. Configure Environment Variables

Copy the example environment file:

```bash
cp env.example .env
```

Edit `.env` and fill in your credentials:

```env
TELEGRAM_BOT_TOKEN=your_telegram_bot_token_here
OPENAI_API_KEY=your_openai_api_key_here
DATABASE_URL=postgres://postgres:password@localhost/pollinet_bot
```

### 4. Build and Run

```bash
# Build the project
cargo build --release

# Run the bot
cargo run --release
```

Or for development with logging:

```bash
RUST_LOG=info cargo run
```

## Creating a Telegram Bot 🤖

1. Open Telegram and search for [@BotFather](https://t.me/botfather)
2. Send `/newbot` and follow the instructions
3. Choose a name and username for your bot
4. Copy the API token provided by BotFather
5. Add the token to your `.env` file

## Adding Documents to Knowledge Base 📚

Create a simple script or add documents programmatically. Here's an example:

```rust
use pollinet_knowledge_bot::{config::Config, rag::RAGSystem};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config
    let config = Config::from_env()?;
    
    // Initialize RAG system
    let rag = RAGSystem::new(config).await?;
    rag.initialize_collection().await?;
    
    // Example: Add Pollinet whitepaper
    let whitepaper = r#"
    Pollinet is a decentralized network protocol...
    [Your document content here]
    "#;
    
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "whitepaper".to_string());
    metadata.insert("date".to_string(), "2024".to_string());
    
    rag.add_document("pollinet_whitepaper", whitepaper, metadata).await?;
    
    println!("Document added successfully!");
    Ok(())
}
```

Save this as `examples/add_documents.rs` and run:

```bash
cargo run --example add_documents
```

## Usage Examples 💬

### In Private Chat

Simply send any question:

```
User: What is Pollinet?
Bot: Pollinet is a decentralized network protocol that...

User: How does it work?
Bot: [Provides answer based on context and conversation history]
```

### In Group Chat

Mention the bot or use the keyword "Pollinet":

```
User: @pollinet_bot what are the key features?
Bot: The key features of Pollinet include...

User: Tell me about Pollinet tokenomics
Bot: [Provides answer about tokenomics]
```

### Commands

- `/start` - Welcome message and introduction
- `/help` - Show help information
- `/clear` - Clear conversation history

### Example Conversation with Memory

```
User: What is Pollinet?
Bot: Pollinet is a decentralized protocol for...

User: What are its main use cases?
Bot: Based on the information about Pollinet I just shared, 
     its main use cases include...

User: How does the first use case work?
Bot: [Remembers which use case was discussed first and explains it]
```

### When Information is Not Available

```
User: What is Pollinet's stock price?
Bot: I don't have that information yet.
```

The bot will **never** make up information!

## Project Structure 📁

```
pollinet_knowledge_bot/
├── Cargo.toml              # Dependencies and project metadata
├── Cargo.lock              # Locked dependencies
├── env.example             # Example environment configuration
├── README.md               # This file
└── src/
    ├── main.rs            # Application entry point
    ├── config.rs          # Configuration management
    ├── bot.rs             # Telegram bot setup
    ├── handlers.rs        # Message and command handlers
    └── rag.rs             # RAG pipeline implementation
```

## Configuration Options ⚙️

All configuration is done via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `TELEGRAM_BOT_TOKEN` | Bot token from BotFather | **Required** |
| `OPENAI_API_KEY` | OpenAI API key | **Required** |
| `DATABASE_URL` | PostgreSQL connection string | **Required** |
| `EMBEDDINGS_TABLE` | Table name for embeddings | `document_embeddings` |
| `EMBEDDING_MODEL` | OpenAI embedding model | `text-embedding-ada-002` |
| `GPT_MODEL` | OpenAI chat model | `gpt-4o-mini` |
| `MAX_CONVERSATION_HISTORY` | Max messages to remember | `10` |
| `TOP_K_CHUNKS` | Number of chunks to retrieve | `5` |
| `RUST_LOG` | Logging level | `info` |

## Error Handling 🛡️

The bot includes comprehensive error handling:

- API failures are logged and graceful error messages are sent to users
- Invalid configurations are caught at startup
- Connection issues with Qdrant or OpenAI are handled gracefully
- All errors are logged for debugging

## Logging 📝

Set the `RUST_LOG` environment variable to control logging:

```bash
# Only errors
RUST_LOG=error cargo run

# Info and above (recommended)
RUST_LOG=info cargo run

# Debug mode (verbose)
RUST_LOG=debug cargo run

# Everything
RUST_LOG=trace cargo run
```

## Development 🔧

### Running Tests

```bash
cargo test
```

### Formatting Code

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Building for Production

```bash
cargo build --release
```

The optimized binary will be in `target/release/pollinet_knowledge_bot`.

## Deployment 🚀

### Using systemd (Linux)

Create a service file `/etc/systemd/system/pollinet-bot.service`:

```ini
[Unit]
Description=Pollinet Knowledge Bot
After=network.target

[Service]
Type=simple
User=pollinet
WorkingDirectory=/opt/pollinet_knowledge_bot
EnvironmentFile=/opt/pollinet_knowledge_bot/.env
ExecStart=/opt/pollinet_knowledge_bot/target/release/pollinet_knowledge_bot
Restart=always

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable pollinet-bot
sudo systemctl start pollinet-bot
sudo systemctl status pollinet-bot
```

### Using Docker

Create a `Dockerfile`:

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/pollinet_knowledge_bot /usr/local/bin/
CMD ["pollinet_knowledge_bot"]
```

Build and run:

```bash
docker build -t pollinet-bot .
docker run -d --env-file .env --name pollinet-bot pollinet-bot
```

## Troubleshooting 🔍

### Bot doesn't respond in groups

- Make sure the bot has permission to read messages in the group
- Check that messages contain either a mention or the keyword "Pollinet"
- Verify the bot username matches what you're mentioning

### "I don't have that information yet" responses

- Ensure documents have been added to the knowledge base
- Check that Qdrant is running and accessible
- Verify the collection exists: `curl http://localhost:6334/collections`

### OpenAI API errors

- Verify your API key is correct
- Check your OpenAI account has credits
- Ensure you have access to the models (`text-embedding-ada-002` and `gpt-4o-mini`)

### Qdrant connection issues

- Verify Qdrant is running: `curl http://localhost:6334/health`
- Check the `QDRANT_URL` in your `.env` file
- Ensure the port is not blocked by firewall

## Contributing 🤝

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License 📄

[Add your license here]

## Support 💬

For issues and questions:

- Open an issue on GitHub
- Contact the development team
- Check existing documentation

## Acknowledgments 🙏

Built with:

- [teloxide](https://github.com/teloxide/teloxide) - Elegant Telegram bots framework
- [Qdrant](https://qdrant.tech/) - Vector database for semantic search
- [OpenAI](https://openai.com/) - Embeddings and GPT models
- [tokio](https://tokio.rs/) - Async runtime for Rust

---

Made with ❤️ for the Pollinet community

