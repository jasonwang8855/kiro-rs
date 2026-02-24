# Docker Usage

## Prerequisites

- Docker Desktop is installed and running.
- `config.json` and `credentials.json` exist in project root (`kiro.rs/`).

## Build Images

```bash
docker compose build
```

## Run Service

```bash
docker compose up -d kiro-rs
```

Service listens on `127.0.0.1:8990`.

## Run Tests In Docker

```bash
docker compose --profile test run --rm kiro-rs-test
```

This command runs `cargo test --all-targets -- --nocapture` fully in container.

## Stop Service

```bash
docker compose down
```
