# Contributing to Nyx

Thanks for your interest! This document describes how to propose changes and the quality bar expected by this repository.

## Licensing

This project is dual-licensed under Apache-2.0 and MIT. By contributing, you agree that your contributions are licensed under the same terms.

- Apache-2.0: `LICENSE-APACHE`
- MIT: `LICENSE-MIT`

## Minimum Requirements

- Rust MSRV: 1.70+ (see badge in `README.md`).
- No unsafe code: crates enforce `#![forbid(unsafe_code)]`.
- Pure Rust policy: avoid crates requiring C/C++ toolchains or native libs (e.g., OpenSSL, ring) unless explicitly approved.

## Development Quickstart

Workspace-wide commands (run at repo root):

- Build (fast typecheck): `cargo check --workspace`
- Lint (deny warnings): `cargo clippy --workspace -- -D warnings`
- Format (check in CI): `cargo fmt --all -- --check`
- Test: `cargo test --workspace`

Per-crate examples:

- `cargo check -p nyx-daemon`
- `cargo test -p nyx-crypto`

## Submitting Changes

1) Discuss major changes in an issue first. Minor fixes can go straight to PR.
2) Keep PRs small and focused; include tests when behavior changes.
3) Ensure CI passes: fmt, clippy (no warnings), tests.
4) Update docs when public behavior or APIs change (see `docs/`, currently being rewritten; keep `mkdocs.yml` nav consistent).

## Commit / PR Style

- Conventional Commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, etc.
- Clear titles and descriptive bodies. Link related issues (e.g., `Closes #123`).
- Avoid unrelated reformatting in the same commit.

## Code Guidelines

- Error handling: use `thiserror`/`anyhow` judiciously; preserve context.
- Logging: prefer `tracing` with structured fields.
- Async: prefer `tokio`; avoid blocking in async contexts.
- Platform support: changes must compile on Windows/Linux/macOS (CI will check).
- Security: do not introduce crates with non-Rust dependencies unless approved.

## Running the Daemon Locally (example)

- `cargo run -p nyx-daemon`
- On Windows named pipes and Unix sockets, see `nyx-daemon/examples/ipc_client.rs`.

## Reviews

- Two-party review is preferred for substantial changes; small fixes can be single-review.
- Maintainers may request test additions or refactors to maintain quality.

## Contact

- Security reports: see `SECURITY.md` (private reporting via GitHub Security Advisories).
- Code of Conduct: see `CODE_OF_CONDUCT.md`.
