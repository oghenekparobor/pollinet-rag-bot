/// Example: Adding documents to the Pollinet knowledge base
/// 
/// This demonstrates:
/// 1. Initializing the RAG system
/// 2. Creating the Qdrant collection
/// 3. Adding documents with metadata
/// 4. Testing retrieval
/// 
/// Run with: cargo run --example add_documents

use pollinet_knowledge_bot::{config::Config, rag::RAGSystem};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    pretty_env_logger::init();

    println!("🚀 Initializing Pollinet Knowledge Bot...\n");

    // Load configuration
    let config = Config::from_env()?;
    println!("✅ Configuration loaded");

    // Initialize RAG system
    let rag = RAGSystem::new(config).await?;
    println!("✅ RAG system initialized");

    // Create/initialize collection
    rag.initialize_collection().await?;
    println!("✅ Collection initialized\n");

    // Example 1: Pollinet Overview (from official whitepaper)
    // println!("📄 Adding Pollinet Overview...");
    // let overview = r#"
    // Pollinet: Decentralized Bluetooth Mesh SDK for Offline Solana Transactions
    // 
    // Pollinet is a decentralized SDK and runtime enabling Solana transactions to be 
    // distributed opportunistically over Bluetooth Low Energy (BLE) mesh networks. 
    // Inspired by biological pollination, transactions ("pollen grains") are created offline, 
    // propagated across peer devices, and eventually submitted to the Solana blockchain 
    // by any gateway node with internet connectivity.
    //
    // Key Features:
    // - Offline-first: Transactions work without constant internet connectivity
    // - BLE Mesh Network: Peer-to-peer transaction relay across devices
    // - Store-and-Forward: Caching for resilient delivery in offline environments
    // - Nonce Accounts: Extended transaction lifespan beyond recent blockhash limits
    // - LZ4 Compression: 30-70% size reduction for efficient bandwidth usage
    // - Secure: Pre-signed transactions prevent tampering and ensure authenticity
    // - Decentralized: No dependency on centralized infrastructure
    // - Open Source: Released under Apache 2.0 License
    //
    // Perfect for rural areas, disaster scenarios, and censorship-prone environments.
    // "#;
    //
    // let mut metadata = HashMap::new();
    // metadata.insert("source".to_string(), "overview".to_string());
    // metadata.insert("category".to_string(), "general".to_string());
    // metadata.insert("version".to_string(), "2.0".to_string());
    //
    // let chunks = rag.add_document("pollinet_overview", overview, metadata).await?;
    // println!("   ✓ Added with {} chunks\n", chunks);

    // Example 2: Technology Details (from whitepaper)
    // println!("📄 Adding Technology Details...");
    // let technology = r#"
    // Pollinet Technology Architecture
    // 
    // Bluetooth Mesh Network:
    // - Devices advertise their presence and capabilities (e.g., "CAN_SUBMIT_SOLANA")
    // - Peers scan for nearby devices advertising the same service UUID
    // - Nodes connect as both Central and Peripheral for bi-directional relay
    // - Store-and-forward caching when no internet is available
    // - Clusters form locally (~30 meters range), bridges connect clusters
    // 
    // Transaction Distribution Protocol:
    // - Serialized Solana transactions using solana-sdk
    // - Metadata includes max fee and expiration
    // - Compression flag and fragmentation index for large payloads
    // - Duplicate detection via unique transaction IDs
    // - Opportunistic multi-gateway submission for reliability
    // 
    // Nonce Account Management:
    // - Extends transaction lifespan beyond recent blockhash limits
    // - Funded with small SOL balance, reused until exhausted
    // - AdvanceNonceAccount instruction always first
    // - Transactions pre-signed offline, gateways cannot modify
    // - Confirmation messages propagate updated nonce values back through mesh
    // 
    // Compression and Fragmentation:
    // - LZ4 compression provides 30-70% size reduction
    // - Optimized for typical Solana transactions
    // - Fast encoding/decoding for mobile devices
    // - BLE MTU limits handled via fragmentation (~500 bytes)
    // - Fragments: FRAGMENT_START, FRAGMENT_CONTINUE, FRAGMENT_END
    // 
    // SDK Components:
    // - TransactionBuilder: Creates and signs nonce transactions
    // - MeshTransport: Handles BLE scanning, advertising, and relay
    // - CompressionService: LZ4 compress/decompress logic
    // - FragmentHandler: Splits and reassembles messages
    // - SubmissionService: Submits transactions to Solana RPC
    // - ConfirmationRouter: Routes submission confirmations back to origin
    // 
    // Available SDKs:
    // - Rust (core reference implementation)
    // - Swift (iOS)
    // - Kotlin (Android)
    // - JavaScript/TypeScript (React Native)
    // "#;
    //
    // let mut metadata = HashMap::new();
    // metadata.insert("source".to_string(), "whitepaper".to_string());
    // metadata.insert("category".to_string(), "technical".to_string());
    // metadata.insert("section".to_string(), "technology".to_string());
    //
    // let chunks = rag.add_document("pollinet_technology", technology, metadata).await?;
    // println!("   ✓ Added with {} chunks\n", chunks);

    // Example 3: Tokenomics (from pollinet.xyz/tokenomics)
    // println!("📄 Adding Tokenomics Information...");
    // let tokenomics = r#"
    // Pollinet Tokenomics
    // 
    // Native Token: POLLEN
    // Total Supply: 1,000,000,000 POLLEN (Fixed)
    // 
    // Initial Raise: $100,000 via CyreneAI Launchpad
    // 
    // Token Model:
    // A transparent and sustainable token model designed to incentivize network growth 
    // and reward contributors to the Pollinet DePIN ecosystem.
    // 
    // Use Cases:
    // - Network Incentives: Reward nodes that act as gateways and relay transactions
    // - Governance: Vote on protocol upgrades and network parameters
    // - Staking: Participate in securing the mesh network
    // - Ecosystem Development: Fund improvements and integrations
    // - DePIN Rewards: Incentivize physical infrastructure providers
    // 
    // DePIN Focus:
    // Pollinet is part of the Decentralized Physical Infrastructure Network (DePIN) 
    // ecosystem, rewarding real-world device participation in the Bluetooth mesh network.
    // "#;
    //
    // let mut metadata = HashMap::new();
    // metadata.insert("source".to_string(), "tokenomics".to_string());
    // metadata.insert("category".to_string(), "economics".to_string());
    //
    // let chunks = rag.add_document("pollinet_tokenomics", tokenomics, metadata).await?;
    // println!("   ✓ Added with {} chunks\n", chunks);

    // Example 4: FAQ (based on whitepaper)
    // println!("📄 Adding FAQ...");
    // let faq = r#"
    // Pollinet Frequently Asked Questions
    //
    // Q: What is Pollinet?
    // A: Pollinet is a decentralized SDK for distributing Solana transactions over 
    // Bluetooth Low Energy (BLE) mesh networks. It enables offline transactions that 
    // eventually reach the blockchain when any peer has internet connectivity.
    //
    // Q: How does it work?
    // A: Transactions are created and signed offline using nonce accounts, then 
    // propagated across nearby devices via BLE mesh. When any device with internet 
    // connectivity receives the transaction, it submits it to Solana and broadcasts 
    // the confirmation back through the mesh.
    //
    // Q: Is it open source?
    // A: Yes, Pollinet is fully open source and released under the Apache 2.0 License.
    //
    // Q: What are the use cases?
    // A: Rural areas without reliable internet, disaster scenarios, censorship-resistant 
    // transactions, DePIN applications, mobile-first dApps, and any situation requiring 
    // offline-first blockchain transactions.
    //
    // Q: How is it secure?
    // A: Transactions are pre-signed with end-to-end integrity. No private keys are 
    // transmitted. Nonce accounts provide replay protection. Gateways cannot tamper 
    // with or forge transactions.
    //
    // Q: What blockchains does it support?
    // A: Currently supports Solana. Future extensions may include other blockchains.
    //
    // Q: Who can use Pollinet?
    // A: Any developer building Solana applications. SDKs are available in Rust, Swift, 
    // Kotlin, and JavaScript/TypeScript for easy integration.
    //
    // Q: Is it production-ready?
    // A: Pollinet is in active development. Check the GitHub repository for current status 
    // and roadmap.
    //
    // Q: What makes Pollinet different?
    // A: It's the first offline-first infrastructure for Solana, extending network reach 
    // beyond internet connectivity using BLE mesh technology. This enables true peer-to-peer 
    // transactions without relying on centralized infrastructure.
    //
    // Q: How can I contribute?
    // A: Visit the GitHub repository, join the community on Telegram or X (@sol_pollinet), 
    // or consider building applications using the Pollinet SDK.
    // "#;
    //
    // let mut metadata = HashMap::new();
    // metadata.insert("source".to_string(), "faq".to_string());
    // metadata.insert("category".to_string(), "support".to_string());
    //
    // let chunks = rag.add_document("pollinet_faq", faq, metadata).await?;
    // println!("   ✓ Added with {} chunks\n", chunks);

    // Example 5: Security and Future Extensions
    // println!("📄 Adding Security & Future Information...");
    // let security = r#"
    // Pollinet Security Model and Future Extensions
    // 
    // Security Features:
    // - End-to-End Integrity: Transactions are pre-signed, preventing tampering
    // - No Private Keys in Transit: Only signed transaction blobs are relayed
    // - Replay Protection: Nonce accounts prevent transaction duplication
    // - Confirmation Signatures: Gateways return Solana transaction signatures as proof
    // - Optional Encryption: Future versions may encrypt payloads to conceal metadata
    // 
    // Deduplication:
    // When multiple gateways attempt to submit the same transaction, only the first 
    // succeeds. The Solana network automatically advances the nonce account, and 
    // subsequent submissions fail with "Transaction nonce invalid: already used" error.
    // 
    // Coordination Mechanisms:
    // 1. Confirmation Broadcasting: Gateways broadcast confirmation messages over BLE 
    //    to inform peers that a transaction was finalized, including the new nonce value.
    // 2. Pre-Submission Nonce Check: Devices with internet can query nonce account 
    //    state before attempting submission.
    // 
    // Future Extensions:
    // - WiFi Direct Transport: Higher bandwidth and longer range relay
    // - LoRa Integration: Extreme-range mesh relays for wider coverage
    // - Cross-Chain Support: Distributing transactions for other blockchains
    // - Incentive Mechanisms: Rewards for acting as a gateway node
    // 
    // System Benefits:
    // - Resilient: Works in fully offline settings with eventual consistency
    // - Efficient: LZ4 compression and fragmentation reduce bandwidth overhead
    // - Extensible: SDK can integrate with any Solana-based wallet or application
    // "#;
    //
    // let mut metadata = HashMap::new();
    // metadata.insert("source".to_string(), "whitepaper".to_string());
    // metadata.insert("category".to_string(), "security".to_string());
    //
    // let chunks = rag.add_document("pollinet_security", security, metadata).await?;
    // println!("   ✓ Added with {} chunks\n", chunks);

    // New: Pollinet Official URLs and Resources
    println!("📄 Adding Pollinet URLs and Resources...");
    let urls = r#"
    Pollinet Official Resources and Links
    
    Official Website:
    - Main Website: https://pollinet.xyz/
    - The official Pollinet website with information about the project, features, and updates.
    
    GitHub Repository:
    - Repository: https://github.com/pollinet/pollinet
    - Open source SDK and runtime for offline Solana transactions over BLE mesh networks.
    - License: Apache 2.0
    - Languages: Rust (core), Swift (iOS), Kotlin (Android), JavaScript/TypeScript (React Native)
    
    Technical Documentation:
    - Whitepaper: https://pollinet.github.io/pollinet/
    - Complete technical whitepaper covering system architecture, Bluetooth mesh networking, 
      transaction distribution protocol, nonce account management, compression, security model, 
      and SDK architecture.
    
    Social Media and Community:
    - X (formerly Twitter): https://x.com/sol_pollinet
    - Follow @sol_pollinet on X for updates, announcements, and community discussions.
    
    - Telegram: https://t.me/pollinet_dot_xyz
    - Join the Pollinet Telegram community for discussions, support, and collaboration.
    
    All official Pollinet resources are available through these links. The project is fully 
    open source and community-driven.
    "#;

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "official".to_string());
    metadata.insert("category".to_string(), "resources".to_string());
    metadata.insert("type".to_string(), "urls".to_string());

    let chunks = rag.add_document("pollinet_urls", urls, metadata).await?;
    println!("   ✓ Added with {} chunks\n", chunks);

    // New: Pollinet Team Information
    println!("📄 Adding Pollinet Team Information...");
    let team = r#"
    Pollinet Team and Leadership
    
    Founder:
    - Name: Oghenekparobor
    - X (Twitter): https://x.com/oghenekparobor_
    - Role: Founder of Pollinet
    - The visionary behind Pollinet, leading the development of the decentralized SDK 
      for offline Solana transactions over Bluetooth Low Energy mesh networks.
    
    Pollinet is an open source project with contributions from the community. The project 
    is released under the Apache 2.0 License, encouraging collaboration and innovation 
    from developers worldwide.
    
    For inquiries, contributions, or partnerships, reach out through the official channels:
    - GitHub: https://github.com/pollinet/pollinet
    - Telegram: https://t.me/pollinet_dot_xyz
    - X: https://x.com/sol_pollinet
    "#;

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "official".to_string());
    metadata.insert("category".to_string(), "team".to_string());
    metadata.insert("type".to_string(), "team_info".to_string());

    let chunks = rag.add_document("pollinet_team", team, metadata).await?;
    println!("   ✓ Added with {} chunks\n", chunks);

    // Test retrieval
    println!("🔍 Testing retrieval...\n");
    
    let test_queries = vec![
        "What is the Pollinet website?",
        "Where can I find the Pollinet GitHub repository?",
        "Who is the founder of Pollinet?",
        "What is the Pollinet Telegram link?",
        "Where is the Pollinet whitepaper?",
        "What is the Pollinet X/Twitter account?",
    ];

    for query in test_queries {
        println!("Query: {}", query);
        let chunks = rag.retrieve_relevant_chunks(query).await?;
        println!("   ✓ Retrieved {} relevant chunks", chunks.len());
        if !chunks.is_empty() {
            println!("   Preview: {}...", &chunks[0].chars().take(100).collect::<String>());
        }
        println!();
    }

    println!("✅ All documents added successfully!");
    println!("\nYou can now run the bot and ask questions about Pollinet.");
    println!("Examples:");
    println!("  - 'What is the Pollinet website?'");
    println!("  - 'Where is the Pollinet GitHub repository?'");
    println!("  - 'Who is the founder of Pollinet?'");
    println!("  - 'What is the Pollinet Telegram link?'");

    Ok(())
}
