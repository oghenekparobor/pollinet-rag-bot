#!/bin/bash
# Deployment helper script for Pollinet Telegram Bot

set -e

echo "🚀 Pollinet Bot Deployment Helper"
echo "=================================="
echo ""

# Check if .env exists
if [ ! -f .env ]; then
    echo "❌ Error: .env file not found!"
    echo "   Please create .env from env.example"
    echo "   cp env.example .env"
    exit 1
fi

# Check required variables
echo "🔍 Checking environment variables..."
required_vars=("TELEGRAM_BOT_TOKEN" "OPENAI_API_KEY" "DATABASE_URL")
for var in "${required_vars[@]}"; do
    if ! grep -q "^$var=" .env; then
        echo "❌ Error: $var not set in .env"
        exit 1
    fi
done
echo "✅ Environment variables OK"
echo ""

# Check if documents are loaded
echo "🔍 Checking if documents are loaded..."
if command -v psql &> /dev/null; then
    DB_URL=$(grep "^DATABASE_URL=" .env | cut -d'=' -f2-)
    COUNT=$(psql "$DB_URL" -t -c "SELECT COUNT(*) FROM document_embeddings;" 2>/dev/null || echo "0")
    
    if [ "$COUNT" -gt "0" ]; then
        echo "✅ Documents loaded: $COUNT chunks found"
    else
        echo "⚠️  Warning: No documents found in database"
        echo "   Run: cargo run --example add_documents"
        read -p "   Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
else
    echo "⚠️  psql not found, skipping document check"
fi
echo ""

# Build
echo "🔨 Building release binary..."
cargo build --release
echo "✅ Build complete"
echo ""

# Deployment options
echo "📦 Deployment Options:"
echo "1) Run locally (foreground)"
echo "2) Run locally (background with nohup)"
echo "3) Build Docker image"
echo "4) Deploy to Railway"
echo "5) Deploy to Fly.io"
echo ""
read -p "Choose option (1-5): " choice

case $choice in
    1)
        echo "🚀 Starting bot in foreground (Ctrl+C to stop)..."
        RUST_LOG=info ./target/release/pollinet_knowledge_bot
        ;;
    2)
        echo "🚀 Starting bot in background..."
        nohup ./target/release/pollinet_knowledge_bot > bot.log 2>&1 &
        PID=$!
        echo "✅ Bot started with PID: $PID"
        echo "   View logs: tail -f bot.log"
        echo "   Stop bot: kill $PID"
        ;;
    3)
        echo "🐳 Building Docker image..."
        docker build -t pollinet-bot .
        echo "✅ Image built: pollinet-bot"
        echo "   Run with: docker run --env-file .env pollinet-bot"
        ;;
    4)
        if ! command -v railway &> /dev/null; then
            echo "❌ Railway CLI not installed"
            echo "   Install: npm i -g @railway/cli"
            exit 1
        fi
        echo "🚂 Deploying to Railway..."
        railway up
        echo "✅ Deployed! Check: railway logs -f"
        ;;
    5)
        if ! command -v flyctl &> /dev/null; then
            echo "❌ Fly CLI not installed"
            echo "   Install: curl -L https://fly.io/install.sh | sh"
            exit 1
        fi
        echo "✈️  Deploying to Fly.io..."
        flyctl deploy
        echo "✅ Deployed! Check: flyctl logs"
        ;;
    *)
        echo "Invalid option"
        exit 1
        ;;
esac

echo ""
echo "🎉 Done!"

