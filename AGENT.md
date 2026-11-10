# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`varnishls` is a Language Server Protocol (LSP) implementation for Varnish Configuration Language (VCL). It supports Varnish 6.0+ (Community and Enterprise), but not Fastly Varnish.

## Commands

```bash
# Build
make build          # cargo build
cargo build

# Test
make test           # cargo test
cargo test          # run all tests
cargo test <name>   # run a single test by name

# Lint
cargo clippy --no-deps -- -D warnings

# Tree-sitter grammars
make tree-sitter-vcl   # build + test VCL grammar
make tree-sitter-vtc   # build + test VTC grammar
```

Pre-commit hooks (lefthook) run `cargo test`, `cargo clippy`, and `npx tree-sitter test` for both grammars in parallel.

## Architecture

The server is built on `tower-lsp` (async LSP framework) with `tokio`. Documents are stored in a `DashMap<Url, Document>` for concurrent access without global locking.

### Key modules

**`backend.rs`** — LSP backend: handles all LSP lifecycle (`did_open`, `did_change`, `completion`, `goto_definition`, etc.) and cross-document definition resolution via `get_all_definitions_across_all_documents()`.

**`document.rs`** — Core document model. Holds the `tree-sitter` AST, semantic tokens, linting diagnostics, and include-resolution logic. Most analysis happens here.

**`varnish_builtins.rs`** — The type system. Defines a `Type` enum (Obj, Func, Backend, String, etc.) and the `HasTypeProperties` trait used for autocomplete and hover.

**`vcc.rs`** — Parser for `.vcc` files (VMOD definition files shipped alongside Varnish modules).

**`vmod.rs`** — ELF binary introspection for compiled `.so` VMOD libraries. Uses `goblin` to extract the embedded JSON spec.

**`formatter.rs`** — Wraps `topiary-core` with tree-sitter queries from `src/formatter_queries/` to format VCL.

**`config.rs`** — Reads `.varnishls.toml` from the workspace root. Controls VMOD/VCC/VCL paths and per-rule lint severity.

**`safe_regex.rs`** — Validates regexes used in VCL rules against ReDoS patterns.

**`main.rs`** — CLI entry point with subcommands: `lsp`, `lint`, `format`, `inspect-vmod`, `inspect-vcc`.

### Vendored grammars

`vendor/tree-sitter-vcl/` and `vendor/tree-sitter-vtc/` are npm projects containing tree-sitter grammars. They must be built separately (see `make tree-sitter-vcl`).

### Document lifecycle

1. Editor opens a `.vcl`/`.vtc` → `did_open` in backend
2. Tree-sitter parses the file; semantic tokens and diagnostics are extracted
3. `include` statements are discovered and queued for loading
4. On edits, the AST is updated incrementally and diagnostics re-run
5. Cross-file analysis resolves definitions from all loaded documents

### VMOD loading

VMODs are loaded in priority order: `.so` binary → `.vcc` definition file. The binary path is configured via `vmod_paths` in `.varnishls.toml`.

### Testing

- Unit tests live inline in each module
- Snapshot tests (via `insta`) in `src/snapshots/` cover formatter and linter output
- Formatter test cases: `src/formatter_tests/`
- Lint test cases: `src/lint_tests/`
- Tree-sitter grammar tests are npm-based (`npx tree-sitter test`)

To update snapshots after intentional output changes: `cargo insta review`
