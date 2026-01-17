# Naidis Core

Rust JSON-RPC server for AI/PDF/YouTube/RSS processing.

## Features

- **YouTube**: Transcript extraction via yt-dlp
- **PDF**: Text/table extraction, OCR
- **AI**: Local LLM inference, embedding, semantic search
- **RSS**: Feed parsing and article extraction
- **External**: Wallabag/Hoarder integration

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Start JSON-RPC server
naidis-core --host 127.0.0.1 --port 9123
```

## API

JSON-RPC 2.0 over HTTP.

### Methods

| Method | Description |
|--------|-------------|
| `system.version` | Get server version |
| `system.ping` | Health check |
| `youtube.extract_transcript` | Extract YouTube transcript |
| `pdf.extract_text` | Extract text from PDF |
| `ai.chat` | Chat with local LLM |
| `rss.parse_feed` | Parse RSS/Atom feed |

## License

MIT
