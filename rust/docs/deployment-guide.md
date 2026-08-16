# Deployment Guide — dsh-rust

## Overview

This guide covers deploying dsh-rust in development, staging, and production environments.

## Prerequisites

- Rust 1.77+ (stable)
- Docker (optional, for containerized deployment)
- DeepSeek API key (or OpenAI/Anthropic key)

---

## Local Development

### 1. Build

```bash
cd rust
cargo build
```

### 2. Run Headless

```bash
cargo run -p dsh-headless -- \
  --task "What is 2+2?" \
  --api-key sk-xxx \
  --model deepseek-chat \
  --output session.jsonl
```

### 3. Run Web Server

```bash
cargo run -p dsh-web -- \
  --api-key sk-xxx \
  --port 3000 \
  --host 0.0.0.0
```

### 4. Test the API

```bash
# Health check
curl http://localhost:3000/health

# Chat
curl -X POST http://localhost:3000/api/v1/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello!"}'
```

---

## Docker Deployment

### Dockerfile

```dockerfile
FROM rust:1.77-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release -p dsh-web

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/dsh-web /usr/local/bin/

EXPOSE 3000
CMD ["dsh-web", "--port", "3000", "--host", "0.0.0.0"]
```

### Build and Run

```bash
# Build
docker build -t dsh-rust -f Dockerfile .

# Run
docker run -p 3000:3000 \
  -e DEEPSEEK_API_KEY=sk-xxx \
  dsh-rust
```

### Docker Compose

```yaml
version: '3.8'

services:
  dsh-web:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "3000:3000"
    environment:
      - DEEPSEEK_API_KEY=${DEEPSEEK_API_KEY}
      - RUST_LOG=info
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

---

## Production Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DEEPSEEK_API_KEY` | DeepSeek API key | Required |
| `DEEPSEEK_BASE_URL` | Custom API endpoint | `https://api.deepseek.com` |
| `RUST_LOG` | Log level | `info` |
| `DSH_PORT` | Server port | `3000` |
| `DSH_HOST` | Bind address | `0.0.0.0` |

### Profile Configuration

Create a `dsh.json` profile:

```json
{
  "name": "production",
  "llm": "deepseek",
  "model": "deepseek-chat",
  "sandbox": "auto",
  "persistence": "sqlite",
  "persistence_path": "/data/sessions.db"
}
```

Load with:
```bash
dsh-web --profile /etc/dsh/dsh.json
```

---

## Scaling

### Horizontal Scaling

The web server is stateless (sessions are stored externally). Scale horizontally with a load balancer:

```yaml
# docker-compose.scale.yml
services:
  dsh-web:
    deploy:
      replicas: 3
    # ... same config as above
```

### Distributed Sessions

For multi-node deployments, use the distributed session store:

```json
{
  "persistence": "distributed",
  "distributed_url": "redis://redis:6379",
  "distributed_ttl": 3600
}
```

---

## Monitoring

### Health Endpoint

```
GET /health
```

Returns `200 OK` when healthy.

### Metrics

The observability module exposes metrics via the `Metrics` struct. In production, export to Prometheus/Grafana:

```rust
use dsh_seam_observability::Metrics;

let metrics = Metrics::new();
// ... use metrics ...
let snapshot = metrics.snapshot(); // JSON export
```

### Logging

Configure structured logging:

```bash
# Pretty (development)
RUST_LOG=debug cargo run -p dsh-web

# JSON (production)
RUST_LOG=info cargo run -p dsh-web
```

---

## Security

### API Key Management

1. Generate strong API keys: `openssl rand -hex 32`
2. Store in environment variables, never in code
3. Rotate keys regularly
4. Use separate keys per environment

### Rate Limiting

Configure in profile:

```json
{
  "rate_limit": {
    "max_tokens": 60,
    "refill_rate": 1.0
  }
}
```

### Sandbox

Enable sandboxing for tool execution:

```json
{
  "sandbox": "auto"
}
```

- **macOS**: Uses `sandbox-exec` (seatbelt)
- **Linux**: Uses Landlock (kernel 5.13+)
- **Windows**: Uses restricted tokens

---

## Troubleshooting

### Common Issues

**Port already in use**
```bash
lsof -i :3000
kill -9 <PID>
```

**API key not working**
```bash
# Verify the key
curl -H "Authorization: Bearer sk-xxx" http://localhost:3000/health
```

**High latency**
- Check LLM provider status
- Monitor `llm.latency_ms` metric
- Consider using a faster model

**Memory usage**
- Monitor `agent.turns.total` metric
- Set session TTL to clean up old sessions
- Use distributed session store for multi-node
