# Getting Started with AMOS

Two ways to run AMOS locally: **Docker** (recommended) or **Native**.

## Quick Start (Docker)

Requires: Docker Desktop (or Podman) and Git.

```bash
# 1. Clone and configure
git clone https://github.com/amos-labs/amos-automate.git
cd amos-automate
cp .env.docker .env

# 2. Edit .env — set AWS credentials for AI (optional, services run without them)
#    AWS_ACCESS_KEY_ID=...
#    AWS_SECRET_ACCESS_KEY=...

# 3. Start infrastructure (Postgres + Redis)
docker compose up postgres redis -d

# 4. Build and run the harness (standalone mode)
docker compose --profile standalone up --build

# 5. Verify
curl http://localhost:3000/health
```

### Services and ports

| Service    | Port | Profile      | Description                     |
|------------|------|--------------|---------------------------------|
| postgres   | 5432 | (default)    | PostgreSQL 16 + pgvector        |
| redis      | 6379 | (default)    | Redis 7                         |
| harness    | 3000 | standalone   | Per-customer AI business OS     |
| agent      | 3100 | standalone   | Autonomous AI agent             |
| relay      | 4100 | relay        | Bounty marketplace              |
| platform   | 4000 | (default)    | Managed hosting control plane   |

### Running different profiles

```bash
# Just harness + agent (most common for dev)
docker compose --profile standalone up

# Harness + relay (for bounty/marketplace work)
docker compose --profile standalone --profile relay up

# Everything including platform
docker compose --profile standalone --profile relay up
```

### Podman users (macOS)

Set the socket path in your `.env`:
```
DOCKER_SOCK=/run/user/501/podman/podman.sock
```

Build the harness image first (podman-compose can't build profiled services):
```bash
BUILDAH_FORMAT=docker podman build -t amos-harness:latest -f docker/harness/Dockerfile .
```

---

## Quick Start (Native / macOS)

Requires: Rust 1.83+, PostgreSQL 15+ with pgvector, Redis.

```bash
# 1. Install prerequisites (macOS)
brew install rust postgresql@16 redis
brew install pgvector  # or: CREATE EXTENSION vector; after DB setup

# 2. Clone and configure
git clone https://github.com/amos-labs/amos-automate.git
cd amos-automate
cp .env.example .env

# 3. Setup databases and start services
./scripts/dev-setup.sh

# 4. Run the harness
cargo run --bin amos-harness

# 5. Verify
curl http://localhost:3000/health
```

### Running individual services

```bash
cargo run --bin amos-harness     # Port 3000
cargo run --bin amos-agent       # Port 3100 (needs harness running)
cargo run --bin amos-relay       # Port 4100
cargo run --bin amos-cli         # CLI tool
```

---

## Environment Variables

AMOS uses the `AMOS__` prefix with `__` as nested separator.

### Required for AI features
```
AWS_ACCESS_KEY_ID=<your-key>
AWS_SECRET_ACCESS_KEY=<your-secret>
AWS_REGION=us-west-2
```
Without these, services start fine but AI calls will fail. Get Bedrock access from the AWS console.

### Required for database
```
AMOS__DATABASE__URL=postgres://amos:amos_dev_password@localhost:5432/amos_harness_dev
AMOS__REDIS__URL=redis://127.0.0.1:6379
```

### Optional
| Variable | Default | Purpose |
|----------|---------|---------|
| `AMOS__SERVER__PORT` | 3000 | Bind port |
| `AMOS__AGENT__MAX_ITERATIONS` | 25 | Max agent loop iterations |
| `AMOS__RELAY__ENABLED` | false | Connect harness to relay |
| `AMOS__RELAY__URL` | http://localhost:4100 | Relay endpoint |
| `STRIPE_SECRET_KEY` | (empty) | Enable billing (empty = free mode) |

See `.env.example` (native) or `.env.docker` (Docker) for full list.

---

## Running Tests

```bash
# All unit tests
cargo test --lib

# Per-crate
cargo test --lib -p amos-harness
cargo test --lib -p amos-core
cargo test --lib -p amos-agent
cargo test --lib -p amos-relay

# Integration tests (needs running database)
cargo test -p amos-platform --test integration_tests

# Lint
cargo clippy
cargo fmt --check
```

---

## Self-Hosting for Production

AMOS is designed for self-hosting. The Docker images are the same ones used in managed hosting.

```bash
# Build the harness image
docker build -t amos-harness:latest -f docker/harness/Dockerfile .

# Run with your own Postgres and Redis
docker run -d \
  -p 3000:3000 \
  -e AMOS__DATABASE__URL=postgres://user:pass@your-db:5432/amos \
  -e AMOS__REDIS__URL=redis://your-redis:6379 \
  -e AWS_ACCESS_KEY_ID=... \
  -e AWS_SECRET_ACCESS_KEY=... \
  amos-harness:latest
```

For production deployments:
- Use managed databases (RDS, ElastiCache)
- Enable TLS/SSL via reverse proxy (nginx, Caddy, ALB)
- Set strong `AMOS__VAULT__MASTER_KEY` and `AMOS__AUTH__JWT_SECRET`
- Monitor via `/health` endpoint

---

## Troubleshooting

**"connection refused" on port 5432**
PostgreSQL isn't running. Start it: `docker compose up postgres -d` or `brew services start postgresql@16`

**"pgvector extension not found"**
Install pgvector: `brew install pgvector` or ensure `pgvector/pgvector:pg16` Docker image is used.

**"Bedrock access denied"**
Your AWS credentials don't have `bedrock:InvokeModel` permission, or Bedrock isn't enabled in your region. Check AWS Console > Bedrock > Model access.

**Docker socket permission denied**
On Linux: `sudo chmod 666 /var/run/docker.sock` or add your user to the docker group.
On macOS with Podman: set `DOCKER_SOCK` in your `.env` (see Podman section above).

**"harness image not found" in Docker Compose**
Build it first: `docker compose --profile standalone build harness`
