# Claude Development Guidelines: rust-mantis-bt

This file instructs Claude Code and Anthropic models on project conventions, workflows, and standards for `rust-mantis-bt`.

@AGENTS.md

---

## Quick Reference Commands

- **Run all tests:** `cargo test`
- **Run single test:** `cargo test <test_name>`
- **Format code:** `cargo fmt`
- **Check format:** `cargo fmt --check`
- **Lint code:** `cargo clippy --all-targets --all-features -- -D warnings`
- **Run dev server:** `cargo run` (Listens on `127.0.0.1:3000` by default; override via `MANTIS_ADDR` and `MANTIS_DB_PATH`)
- **Create API token via CLI:** `cargo run -- create-token --user <username> [--name <label>]`
- **Manual smoke test suite:** `./tests/manual_test.sh`
