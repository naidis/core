# Contributing to naidis-core

Thank you for your interest in contributing to naidis-core! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive in all interactions.

## Getting Started

### Prerequisites

- Rust 1.83 or later
- Cargo (comes with Rust)
- Git

Optional dependencies for specific features:
- `yt-dlp` - for YouTube transcript extraction
- `tesseract` - for OCR functionality
- `ollama` - for local LLM inference (optional, falls back to built-in TinyLlama)

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/naidis.git
   cd naidis/core
   ```
3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/ORIGINAL_OWNER/naidis.git
   ```

## Development Setup

```bash
# Navigate to core directory
cd core

# Build the project
cargo build

# Run tests
cargo test

# Run the server in HTTP mode
cargo run -- --mode http --port 21420

# Run with debug logging
RUST_LOG=debug cargo run -- --mode http --port 21420
```

### Docker (Alternative)

```bash
docker-compose up
```

## Project Structure

```
core/
├── src/
│   ├── main.rs           # Entry point
│   ├── lib.rs            # Library exports
│   ├── ai/               # AI/ML functionality
│   │   ├── mod.rs
│   │   ├── llm.rs        # LLM inference (Candle)
│   │   ├── ollama.rs     # Ollama integration
│   │   ├── rag.rs        # RAG pipeline
│   │   └── embeddings.rs # Vector embeddings
│   ├── rpc/              # JSON-RPC API
│   │   ├── mod.rs
│   │   ├── server.rs     # Main RPC handler (~100 endpoints)
│   │   └── routes/       # Route modules (WIP migration)
│   ├── integrations/     # External service integrations
│   │   ├── todoist.rs
│   │   └── gcal.rs
│   ├── youtube/          # YouTube processing
│   ├── pdf/              # PDF extraction
│   ├── utils/            # Utility functions
│   └── config.rs         # Configuration
├── Cargo.toml
└── Cargo.lock
```

## Coding Standards

### Rust Conventions

- **Edition**: Rust 2021
- **Formatting**: Run `cargo fmt` before committing
- **Linting**: Run `cargo clippy` and address warnings
- **Error Handling**: 
  - Use `anyhow::Result` for application errors
  - Use `thiserror` for library errors that need to be matched
  - Prefer `?` operator over `.unwrap()` in production code

### Code Style

```rust
// Good: Use `?` for error propagation
fn process_data(input: &str) -> anyhow::Result<Output> {
    let parsed = parse_input(input)?;
    let result = transform(parsed)?;
    Ok(result)
}

// Avoid: .unwrap() in production code
fn process_data(input: &str) -> Output {
    let parsed = parse_input(input).unwrap(); // ❌ Avoid this
    transform(parsed).unwrap()
}
```

### Documentation

- Add doc comments (`///`) for public functions and types
- Include examples in doc comments where helpful
- Keep comments in English

### Module Organization

- One `mod.rs` per feature directory
- Co-locate tests with implementation (`#[cfg(test)] mod tests`)
- Keep files under 500 lines when possible (split if larger)

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests for a specific module
cargo test youtube::
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_works() {
        let input = "test input";
        let result = process(input);
        assert_eq!(result, expected_output);
    }

    #[tokio::test]
    async fn test_async_feature() {
        let result = async_process().await;
        assert!(result.is_ok());
    }
}
```

### Test Coverage

- Aim for tests on public API functions
- Include edge cases and error conditions
- Integration tests go in `tests/` directory

## Submitting Changes

### Branch Naming

Use descriptive branch names:
- `feature/youtube-chapter-detection`
- `fix/pdf-table-extraction`
- `refactor/rpc-server-modularization`
- `docs/api-documentation`

### Commit Messages

Follow conventional commit format:

```
type(scope): short description

Longer description if needed.

Fixes #123
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

Examples:
- `feat(ai): add streaming response support for LLM`
- `fix(pdf): handle encrypted PDF files gracefully`
- `refactor(rpc): extract YouTube routes to separate module`

### Pull Request Process

1. Create a feature branch from `main`
2. Make your changes with clear commits
3. Ensure all tests pass: `cargo test`
4. Ensure code is formatted: `cargo fmt`
5. Ensure no clippy warnings: `cargo clippy`
6. Push to your fork and create a Pull Request
7. Fill out the PR template with:
   - Description of changes
   - Related issue numbers
   - Testing performed
   - Screenshots (for UI-related changes)

### PR Checklist

- [ ] Code compiles without warnings (`cargo build`)
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt --check`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated if needed
- [ ] Commit messages follow conventions
- [ ] PR description explains the "why"

## Reporting Issues

### Bug Reports

Include:
1. **Description**: Clear summary of the bug
2. **Steps to Reproduce**: Minimal steps to trigger the issue
3. **Expected Behavior**: What should happen
4. **Actual Behavior**: What actually happens
5. **Environment**: OS, Rust version, relevant dependencies
6. **Logs**: Error messages or stack traces

### Feature Requests

Include:
1. **Problem Statement**: What problem does this solve?
2. **Proposed Solution**: How should it work?
3. **Alternatives Considered**: Other approaches you've thought about
4. **Use Cases**: Concrete examples of how this would be used

## Architecture Decisions

If you're making significant architectural changes:

1. Open an issue first to discuss the approach
2. For large changes, consider writing an RFC-style document
3. Get consensus before implementing

## Questions?

- Open a GitHub issue for questions about contributing
- Check existing issues and PRs for similar topics
- Join our Discord community (link in README)

---

Thank you for contributing to naidis-core! Your efforts help make this project better for everyone.
