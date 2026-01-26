# NAIDIS CORE - Rust Backend Engine

**Parent**: [../AGENTS.md](../AGENTS.md)

## OVERVIEW

Binary-only Rust engine exposing JSON-RPC over HTTP (:21420) and stdio. Handles AI/RAG, content extraction, and external integrations.

## STRUCTURE

```
core/src/
├── rpc/            # HTTP server + stdio JSON-RPC
├── ai/             # RAG, LLM (Candle/Ollama), embeddings
├── integrations/   # Todoist, GCal, Readwise, Wallabag, Hoarder
├── utils/          # Calculator, datetime, emoji, snippets, vault
├── youtube/        # yt-dlp wrapper, transcript extraction
├── pdf/            # PDF parsing, OCR (Tesseract)
├── epub/           # EPUB to markdown
├── rss/            # Feed parsing
├── newsletter/     # IMAP email → markdown
├── reading/        # Read-later store
├── labels/         # Hierarchical labels
├── highlights/     # Highlight persistence
├── tts/            # Text-to-speech
├── git/            # Git operations wrapper
├── tasks/          # Task query
├── periodic/       # Daily/weekly notes
├── dataview/       # Dataview queries
├── nlp/            # NLP utilities
├── tables/         # Table operations
└── web_clip/       # Web page → markdown
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Add HTTP endpoint | `rpc/server.rs` | Add route + handler, mirror types in `rpc/types.rs` |
| Add integration | `integrations/` | Follow `todoist.rs` pattern |
| AI/RAG changes | `ai/rag.rs` | Pipeline orchestration |
| LLM changes | `ai/llm.rs` (Candle), `ai/ollama.rs` (Ollama) | Local inference |
| Content extraction | `youtube/`, `pdf/`, `epub/`, `web_clip/` | Each has `mod.rs` |
| Utility functions | `utils/` | Exposed via RPC |

## CONVENTIONS

### Error Handling
```rust
// App-level: use anyhow
use anyhow::Result;
pub async fn do_thing() -> Result<Output> { ... }

// Module errors: use thiserror
#[derive(thiserror::Error, Debug)]
pub enum LabelError { ... }
```

### State Management
```rust
// Axum shared state
pub struct AppState { ... }
let state = Arc::new(AppState::new());

// Heavy singletons (RAG pipeline, etc.)
static PIPELINE: OnceLock<RwLock<RagPipeline>> = OnceLock::new();
```

### Module Pattern
```rust
// Each feature: one mod.rs file with co-located tests
// core/src/labels/mod.rs
pub struct LabelStore { ... }
impl LabelStore { ... }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_label_create() { ... }
}
```

## ANTI-PATTERNS

- **NO `.unwrap()` in new code** - use `?` operator (existing code has tech debt)
- **NO panic-prone code** - return `Result`, handle errors gracefully
- **NO blocking in async** - use `tokio::task::spawn_blocking` for CPU work

## KEY DEPENDENCIES

| Crate | Purpose |
|-------|---------|
| `axum` | HTTP server |
| `tokio` | Async runtime |
| `candle-*` | Local LLM inference |
| `fastembed` | Embeddings |
| `ollama-rs` | Ollama client |
| `pdf-extract` | PDF parsing |
| `epub` | EPUB parsing |
| `feed-rs` | RSS parsing |
| `reqwest` | HTTP client |
| `scraper`/`readability` | Web scraping |

## COMMANDS

```bash
cargo build --release    # Production build (LTO enabled)
cargo test              # Run all tests
cargo run -- --mode http --port 21420    # HTTP mode
cargo run -- --mode stdio                # JSON-RPC stdio mode

# Docker
docker-compose up       # With optional Ollama sidecar
```

## NOTES

- Binary only (no lib.rs) - all public API via RPC
- Release profile: LTO + strip for minimal binary
- External deps: yt-dlp, tesseract-ocr, poppler-utils (for full features)
