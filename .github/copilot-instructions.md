# Ghost AI - Developer Instructions

## Project Overview

Ghost AI is an invisible AI-powered desktop assistant written in Rust. The project reuses the production scaffolding from the Mai0313 `rust_template` to provide:

- Modern Cargo layout with both library and binary targets
- Comprehensive CI/CD automation via GitHub Actions
- Docker and Dev Container environments for reproducible builds
- Automated formatting, linting, and testing hooks
- Release management with cross-platform artifacts

## Technical Architecture

### Project Structure

```
ghost-ai/
|- src/
|  |- lib.rs            # Library exports shared with integration tests
|  |- main.rs           # Desktop launcher using eframe/egui
|  |- app.rs            # UI state and rendering
|  |- config.rs         # Persistent configuration handling
|  `- ...
|- tests/
|  `- basic.rs          # Integration tests covering configuration defaults
|- docker/
|  `- Dockerfile        # Multi-stage container build
|- .devcontainer/
|  |- devcontainer.json # VS Code Remote Container configuration
|  `- Dockerfile        # Development container image
|- .github/
|  |- workflows/        # CI, release, quality, and automation pipelines
|  `- ISSUE_TEMPLATE/   # Standardised issue forms
|- Makefile             # Convenience commands mirroring CI jobs
|- Cargo.toml           # Crate metadata and dependencies
`- README.md            # Project documentation
```

### Key Runtime Components

- UI layer: `eframe` + `egui` render the assistant interface
- Audio I/O: `cpal` and `hound` handle capture and playback
- Networking: `reqwest` provides async HTTP for OpenAI APIs
- Configuration: `directories` + `serde_json` persist user settings
- Async runtime: `tokio` powers background work and scheduling

## Development Environment

### Prerequisites

- Rust toolchain (managed through `rustup`)
- Cargo package manager (installed with Rust toolchain)
- Git
- Docker (optional, for container builds)
- Make (optional, for convenience commands)

### Getting Started

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and set up the repository
git clone https://github.com/Mai0313/ghost-ai.git
cd ghost-ai
cargo build
```

### Daily Commands

```bash
# Formatting and linting
make fmt
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Testing
make test
make test-verbose
cargo test --all

# Coverage (requires cargo-llvm-cov)
make coverage
cargo llvm-cov --workspace --lcov --output-path lcov.info

# Building
make build
make build-release
cargo build --release --locked

# Running
make run
cargo run --release

# Packaging
make package
cargo package --locked --allow-dirty
```

### Recommended Workflow

- Run `cargo build` frequently while developing new features
- Before committing, run `make fmt` and `make test`
- Before pushing, execute the full `cargo test --all --verbose`

## Build and Release Process

### Local Release Build

```bash
cargo build --release --locked
```

### Supported Release Targets

- x86_64-unknown-linux-gnu
- x86_64-unknown-linux-musl
- aarch64-unknown-linux-gnu
- aarch64-unknown-linux-musl
- x86_64-apple-darwin
- aarch64-apple-darwin
- x86_64-pc-windows-msvc
- aarch64-pc-windows-msvc

### Release Workflow

1. Tag the release: `git tag -a v1.0.0 -m "Release v1.0.0"`
2. Push the tag: `git push origin v1.0.0`
3. `build_release.yml` runs to build and publish release assets

### Asset Naming Convention

- Unix: `ghost-ai-v{version}-{target}.tar.gz`
- Windows: `ghost-ai-v{version}-{target}.zip`

Example: `ghost-ai-v1.0.0-x86_64-unknown-linux-gnu.tar.gz`

## CI/CD Workflows

### Core Workflows

1. `test.yml`: Runs formatting, linting, and tests with coverage
2. `code-quality-check.yml`: Enforces code formatting and linting
3. `build_package.yml`: Produces publishable Cargo packages
4. `build_image.yml`: Builds and optionally publishes container images
5. `build_release.yml`: Cross-compiles binaries for tagged releases

### Automation Features

- Automatic PR labelling based on file paths and branches
- Security scanning for dependencies and secrets
- Release drafting with change aggregation
- Semantic pull request enforcement
- Dependabot updates for dependencies

## Quality Standards

### Rust Code Guidelines

- Format with `rustfmt`
- Address all `clippy` warnings (fail CI if any remain)
- Document public interfaces when they stabilise
- Add unit or integration tests for new behaviour

### Commit Conventions

Follow [Conventional Commits](https://conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

### Pull Request Checklist

- All CI checks green
- Conventional commit-style title
- Tests or docs updated as needed
- No unchecked warnings or clippy lint failures

## Platform Notes

### Binary Naming

- Unix targets: binary name `ghost-ai`
- Windows targets: binary `ghost-ai.exe`

### Archive Rules

- Unix archives ship as `.tar.gz`
- Windows archives ship as `.zip`
- Debug symbols stripped in release builds (`strip = "symbols"`)

### Platform Dependencies

- MUSL builds require `musl-tools` plus `pkg-config`
- macOS builds run under Xcode command line tools
- Windows builds use the MSVC toolchain

## Troubleshooting

### Common Issues

- MUSL build failures: `sudo apt install -y musl-tools pkg-config`
- Cross-compilation: use the provided Dockerfile or configure `zig` as a linker
- Permission errors: verify GitHub token scopes when publishing releases

### Performance Tips

- Prefer `cargo build --release` when profiling or distributing
- LTO and symbol stripping are enabled in `Cargo.toml`

## Security Practices

- Restrict GitHub workflow permissions to least privilege
- Rotate stored secrets regularly
- Audit dependency updates prompted by Dependabot
- Enable clippy's pedantic/security lints where practical

## Deployment

### Docker

```bash
docker build -f docker/Dockerfile --target prod -t ghost-ai:latest .
docker run --rm ghost-ai:latest
```

### Native Binaries

Download platform-specific archives from GitHub Releases and distribute them directly.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Implement changes following the guidelines above
4. Add or update tests
5. Ensure CI is green
6. Submit a PR with a conventional commit title

## Helpful References

- [Rust Documentation](https://doc.rust-lang.org/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [Conventional Commits](https://www.conventionalcommits.org/)
