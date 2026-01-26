# NAIDIS AI MODULE

**Parent**: [../../AGENTS.md](../../AGENTS.md)

## OVERVIEW

Local-first AI pipeline: embeddings (fastembed), RAG (Candle/Ollama), semantic search. Designed for Obsidian vault indexing and querying.

## STRUCTURE

```
ai/
├── mod.rs      # Module exports, pipeline initialization
├── rag.rs      # RAG orchestration (retrieval + generation)
├── llm.rs      # Candle-based local inference (TinyLlama)
├── ollama.rs   # Ollama client wrapper
├── embeddings.rs   # Vector embeddings (fastembed)
└── search.rs   # Semantic search over indexed vault
```

## WHERE TO LOOK

| Task | File | Notes |
|------|------|-------|
| Change RAG prompts | `rag.rs` | Embedded templates |
| Add LLM model | `llm.rs` | Candle model loading |
| Ollama integration | `ollama.rs` | Streaming responses |
| Embedding model | `embeddings.rs` | fastembed config |
| Search tuning | `search.rs` | Vector similarity |

## CONVENTIONS

### Pipeline Singleton
```rust
// Heavy resources use OnceLock + RwLock
static RAG_PIPELINE: OnceLock<RwLock<RagPipeline>> = OnceLock::new();

pub fn get_pipeline() -> &'static RwLock<RagPipeline> {
    RAG_PIPELINE.get_or_init(|| RwLock::new(RagPipeline::new()))
}
```

### Async Inference
```rust
// All inference is async
pub async fn chat(&self, messages: Vec<Message>) -> Result<String> { ... }
pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> { ... }
```

## ANTI-PATTERNS

- **NO blocking inference** - always async or spawn_blocking
- **NO hardcoded API keys** - passed via request params
- Avoid `.unwrap()` on model operations - they can fail

## NOTES

- Default model: TinyLlama-1.1B (via Candle)
- Optional: Ollama for larger models
- Embeddings: fastembed with ONNX runtime
- Prompts are currently hardcoded - future: template files
