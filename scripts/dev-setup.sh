#!/usr/bin/env bash
# =============================================================================
# AMOS Development Environment Setup
# =============================================================================
# Manages local development infrastructure (PostgreSQL + Redis) and database
# setup for the AMOS workspace.
#
# Usage:
#   ./scripts/dev-setup.sh           # Full setup (prerequisites, databases, migrations)
#   ./scripts/dev-setup.sh setup     # Same as above
#   ./scripts/dev-setup.sh start     # Start PostgreSQL + Redis
#   ./scripts/dev-setup.sh stop      # Stop PostgreSQL + Redis
#   ./scripts/dev-setup.sh reset     # Drop and recreate all databases
#   ./scripts/dev-setup.sh status    # Show what's running
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Colors and output helpers
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No color

info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERR]${NC}   $*"; }
header()  { echo -e "\n${BOLD}${CYAN}=== $* ===${NC}\n"; }

# ---------------------------------------------------------------------------
# Configuration — mirrors docker-compose.yml defaults
# ---------------------------------------------------------------------------
POSTGRES_USER="${POSTGRES_USER:-amos}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-amos_dev_password}"
POSTGRES_DB="${POSTGRES_DB:-amos_development}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
REDIS_PORT="${REDIS_PORT:-6379}"

# Databases to create (each needs pgvector)
DATABASES=("amos_development" "amos_harness_dev" "amos_platform_dev" "amos_relay_dev")

# Docker container names (from docker-compose.yml)
PG_CONTAINER="amos_dev_postgres"
REDIS_CONTAINER="amos_dev_redis"

# Resolve the workspace root (parent of scripts/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Platform repo lives alongside the workspace
PLATFORM_ROOT="$(cd "$WORKSPACE_ROOT/../amos-platform" 2>/dev/null && pwd || echo "")"

# ---------------------------------------------------------------------------
# Infrastructure mode detection
# ---------------------------------------------------------------------------
# Returns "docker", "brew", or "none"
detect_infra_mode() {
    # Check for running Docker containers first
    if command -v docker &>/dev/null && docker info &>/dev/null 2>&1; then
        if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "$PG_CONTAINER"; then
            echo "docker"
            return
        fi
    fi

    # Check for Podman
    if command -v podman &>/dev/null && podman info &>/dev/null 2>&1; then
        if podman ps --format '{{.Names}}' 2>/dev/null | grep -q "$PG_CONTAINER"; then
            echo "docker"  # podman uses same CLI interface
            return
        fi
    fi

    # Check for Homebrew services (macOS)
    if command -v brew &>/dev/null; then
        if brew services list 2>/dev/null | grep -q "postgresql.*started"; then
            echo "brew"
            return
        fi
    fi

    # Check for system PostgreSQL
    if command -v pg_isready &>/dev/null && pg_isready -h localhost -p "$POSTGRES_PORT" &>/dev/null; then
        echo "system"
        return
    fi

    echo "none"
}

# Determine which container runtime is available
container_cmd() {
    if command -v docker &>/dev/null && docker info &>/dev/null 2>&1; then
        echo "docker"
    elif command -v podman &>/dev/null && podman info &>/dev/null 2>&1; then
        echo "podman"
    else
        echo ""
    fi
}

# ---------------------------------------------------------------------------
# psql wrapper — routes through Docker or local depending on mode
# ---------------------------------------------------------------------------
run_psql() {
    local db="${1:-$POSTGRES_DB}"
    shift
    local mode
    mode="$(detect_infra_mode)"

    case "$mode" in
        docker)
            local cmd
            cmd="$(container_cmd)"
            "$cmd" exec -i "$PG_CONTAINER" psql -U "$POSTGRES_USER" -d "$db" "$@"
            ;;
        brew|system)
            PGPASSWORD="$POSTGRES_PASSWORD" psql -h localhost -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$db" "$@"
            ;;
        *)
            error "No PostgreSQL connection available"
            return 1
            ;;
    esac
}

# ---------------------------------------------------------------------------
# check_prerequisites — verify required tools
# ---------------------------------------------------------------------------
check_prerequisites() {
    header "Checking Prerequisites"
    local ok=true

    # Rust
    if command -v rustc &>/dev/null; then
        local rust_version
        rust_version="$(rustc --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')"
        local rust_minor
        rust_minor="$(echo "$rust_version" | cut -d. -f2)"
        if [ "$rust_minor" -ge 83 ]; then
            success "Rust $rust_version (>= 1.83)"
        else
            error "Rust $rust_version found, need >= 1.83. Run: rustup update"
            ok=false
        fi
    else
        error "Rust not found. Install from https://rustup.rs"
        ok=false
    fi

    # Cargo
    if command -v cargo &>/dev/null; then
        success "Cargo $(cargo --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')"
    else
        error "Cargo not found"
        ok=false
    fi

    # PostgreSQL client (psql)
    if command -v psql &>/dev/null; then
        local pg_version
        pg_version="$(psql --version | grep -oE '[0-9]+\.[0-9]+'| head -1)"
        local pg_major
        pg_major="$(echo "$pg_version" | cut -d. -f1)"
        if [ "$pg_major" -ge 15 ]; then
            success "PostgreSQL client $pg_version (>= 15)"
        else
            warn "PostgreSQL client $pg_version found, recommend >= 15"
        fi
    else
        warn "psql not found locally (OK if using Docker)"
    fi

    # PostgreSQL server — check Docker container or local
    local pg_running=false
    if command -v docker &>/dev/null && docker ps --format '{{.Names}}' 2>/dev/null | grep -q "$PG_CONTAINER"; then
        success "PostgreSQL running in Docker ($PG_CONTAINER)"
        pg_running=true
    elif command -v podman &>/dev/null && podman ps --format '{{.Names}}' 2>/dev/null | grep -q "$PG_CONTAINER"; then
        success "PostgreSQL running in Podman ($PG_CONTAINER)"
        pg_running=true
    elif pg_isready -h localhost -p "$POSTGRES_PORT" &>/dev/null 2>&1; then
        success "PostgreSQL running locally on port $POSTGRES_PORT"
        pg_running=true
    else
        warn "PostgreSQL not running. Will start with 'start' command."
    fi

    # Redis — check Docker container or local
    local redis_running=false
    if command -v docker &>/dev/null && docker ps --format '{{.Names}}' 2>/dev/null | grep -q "$REDIS_CONTAINER"; then
        success "Redis running in Docker ($REDIS_CONTAINER)"
        redis_running=true
    elif command -v podman &>/dev/null && podman ps --format '{{.Names}}' 2>/dev/null | grep -q "$REDIS_CONTAINER"; then
        success "Redis running in Podman ($REDIS_CONTAINER)"
        redis_running=true
    elif command -v redis-cli &>/dev/null && redis-cli -p "$REDIS_PORT" ping &>/dev/null 2>&1; then
        success "Redis running locally on port $REDIS_PORT"
        redis_running=true
    else
        warn "Redis not running. Will start with 'start' command."
    fi

    # sqlx-cli
    if command -v sqlx &>/dev/null; then
        success "sqlx-cli $(sqlx --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo 'installed')"
    else
        warn "sqlx-cli not found. Install with: cargo install sqlx-cli --no-default-features --features postgres"
        ok=false
    fi

    # Docker / Podman (optional)
    local crt
    crt="$(container_cmd)"
    if [ -n "$crt" ]; then
        success "Container runtime: $crt"
    else
        info "No container runtime detected (Docker/Podman). Using local services."
    fi

    if [ "$ok" = false ]; then
        error "Some prerequisites are missing. Fix the errors above and retry."
        return 1
    fi
    success "All required prerequisites satisfied."
}

# ---------------------------------------------------------------------------
# start — bring up PostgreSQL + Redis
# ---------------------------------------------------------------------------
cmd_start() {
    header "Starting Infrastructure"

    local crt
    crt="$(container_cmd)"

    if [ -n "$crt" ]; then
        info "Using Docker Compose to start postgres + redis..."
        (cd "$WORKSPACE_ROOT" && docker compose up postgres redis -d)
        info "Waiting for PostgreSQL to be ready..."
        local retries=30
        while [ $retries -gt 0 ]; do
            if docker exec "$PG_CONTAINER" pg_isready -U "$POSTGRES_USER" &>/dev/null 2>&1; then
                break
            fi
            retries=$((retries - 1))
            sleep 1
        done
        if [ $retries -eq 0 ]; then
            error "PostgreSQL did not become ready in time"
            return 1
        fi
        success "PostgreSQL is ready"

        info "Waiting for Redis to be ready..."
        retries=15
        while [ $retries -gt 0 ]; do
            if docker exec "$REDIS_CONTAINER" redis-cli ping &>/dev/null 2>&1; then
                break
            fi
            retries=$((retries - 1))
            sleep 1
        done
        if [ $retries -eq 0 ]; then
            error "Redis did not become ready in time"
            return 1
        fi
        success "Redis is ready"
    else
        # Fallback: Homebrew (macOS)
        if command -v brew &>/dev/null; then
            info "Starting PostgreSQL via Homebrew..."
            brew services start postgresql@16 2>/dev/null || brew services start postgresql 2>/dev/null || true

            info "Starting Redis via Homebrew..."
            brew services start redis 2>/dev/null || true

            info "Waiting for services..."
            sleep 3

            if pg_isready -h localhost -p "$POSTGRES_PORT" &>/dev/null; then
                success "PostgreSQL is ready"
            else
                error "PostgreSQL failed to start"
                return 1
            fi

            if redis-cli -p "$REDIS_PORT" ping &>/dev/null 2>&1; then
                success "Redis is ready"
            else
                error "Redis failed to start"
                return 1
            fi
        else
            error "No container runtime or Homebrew found. Cannot start services."
            error "Install Docker/Podman or Homebrew, then retry."
            return 1
        fi
    fi
}

# ---------------------------------------------------------------------------
# stop — bring down PostgreSQL + Redis
# ---------------------------------------------------------------------------
cmd_stop() {
    header "Stopping Infrastructure"

    local crt
    crt="$(container_cmd)"

    if [ -n "$crt" ] && docker ps --format '{{.Names}}' 2>/dev/null | grep -qE "($PG_CONTAINER|$REDIS_CONTAINER)"; then
        info "Stopping Docker Compose services..."
        (cd "$WORKSPACE_ROOT" && docker compose stop postgres redis)
        success "Docker services stopped"
    fi

    if command -v brew &>/dev/null; then
        if brew services list 2>/dev/null | grep -q "postgresql.*started"; then
            info "Stopping Homebrew PostgreSQL..."
            brew services stop postgresql@16 2>/dev/null || brew services stop postgresql 2>/dev/null || true
            success "Homebrew PostgreSQL stopped"
        fi
        if brew services list 2>/dev/null | grep -q "redis.*started"; then
            info "Stopping Homebrew Redis..."
            brew services stop redis 2>/dev/null || true
            success "Homebrew Redis stopped"
        fi
    fi

    success "Infrastructure stopped"
}

# ---------------------------------------------------------------------------
# create_databases — create all databases and enable pgvector
# ---------------------------------------------------------------------------
create_databases() {
    header "Creating Databases"

    for db in "${DATABASES[@]}"; do
        local exists
        exists=$(run_psql "postgres" -tAc "SELECT 1 FROM pg_database WHERE datname = '$db'" 2>/dev/null || echo "")
        if [ "$exists" = "1" ]; then
            success "Database '$db' already exists"
        else
            info "Creating database '$db'..."
            run_psql "postgres" -c "CREATE DATABASE $db OWNER $POSTGRES_USER;" 2>/dev/null
            success "Created database '$db'"
        fi

        # Enable pgvector
        info "Enabling pgvector on '$db'..."
        run_psql "$db" -c "CREATE EXTENSION IF NOT EXISTS vector;" 2>/dev/null
        success "pgvector enabled on '$db'"
    done
}

# ---------------------------------------------------------------------------
# run_migrations — run sqlx migrations for each crate that has them
# ---------------------------------------------------------------------------
run_migrations() {
    header "Running Migrations"

    if ! command -v sqlx &>/dev/null; then
        warn "sqlx-cli not installed, skipping migrations."
        warn "Install with: cargo install sqlx-cli --no-default-features --features postgres"
        return 0
    fi

    # Harness migrations -> amos_harness_dev
    local harness_migrations="$WORKSPACE_ROOT/amos-harness/migrations"
    if [ -d "$harness_migrations" ]; then
        info "Running harness migrations..."
        DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/amos_harness_dev" \
            sqlx migrate run --source "$harness_migrations"
        success "Harness migrations complete"
    else
        info "No harness migrations directory found, skipping"
    fi

    # Relay migrations -> amos_relay_dev
    local relay_migrations="$WORKSPACE_ROOT/amos-relay/migrations"
    if [ -d "$relay_migrations" ]; then
        info "Running relay migrations..."
        DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/amos_relay_dev" \
            sqlx migrate run --source "$relay_migrations"
        success "Relay migrations complete"
    else
        info "No relay migrations directory found, skipping"
    fi

    # Platform migrations -> amos_platform_dev (separate repo)
    if [ -n "$PLATFORM_ROOT" ] && [ -d "$PLATFORM_ROOT/migrations" ]; then
        info "Running platform migrations (from $PLATFORM_ROOT)..."
        DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/amos_platform_dev" \
            sqlx migrate run --source "$PLATFORM_ROOT/migrations"
        success "Platform migrations complete"
    else
        info "Platform migrations directory not found (expected at ../amos-platform/migrations), skipping"
    fi
}

# ---------------------------------------------------------------------------
# copy_env — copy .env.example to .env if it doesn't exist
# ---------------------------------------------------------------------------
copy_env() {
    header "Environment Configuration"

    local env_example="$WORKSPACE_ROOT/.env.example"
    local env_file="$WORKSPACE_ROOT/.env"

    if [ -f "$env_file" ]; then
        success ".env already exists"
    elif [ -f "$env_example" ]; then
        info "Copying .env.example to .env..."
        cp "$env_example" "$env_file"
        success "Created .env from .env.example — edit it with your credentials"
    else
        warn "No .env.example found, skipping .env creation"
    fi
}

# ---------------------------------------------------------------------------
# cmd_setup — full setup (default command)
# ---------------------------------------------------------------------------
cmd_setup() {
    header "AMOS Development Environment Setup"
    echo -e "  Workspace: ${BOLD}$WORKSPACE_ROOT${NC}"
    echo -e "  Platform:  ${BOLD}${PLATFORM_ROOT:-not found}${NC}"
    echo ""

    check_prerequisites

    # Start infrastructure if not running
    local mode
    mode="$(detect_infra_mode)"
    if [ "$mode" = "none" ]; then
        cmd_start
    else
        success "Infrastructure already running (mode: $mode)"
    fi

    copy_env
    create_databases
    run_migrations

    header "Setup Complete"
    echo -e "  PostgreSQL: ${GREEN}localhost:${POSTGRES_PORT}${NC} (user: $POSTGRES_USER)"
    echo -e "  Redis:      ${GREEN}localhost:${REDIS_PORT}${NC}"
    echo ""
    echo -e "  Databases:"
    for db in "${DATABASES[@]}"; do
        echo -e "    - ${CYAN}$db${NC}"
    done
    echo ""
    echo -e "  Next steps:"
    echo -e "    ${BOLD}cargo build${NC}                          # Build the workspace"
    echo -e "    ${BOLD}cargo run --bin amos-harness${NC}         # Start harness on :3000"
    echo -e "    ${BOLD}cargo run --bin amos-platform${NC}        # Start platform on :4000 (from ../amos-platform)"
    echo ""
}

# ---------------------------------------------------------------------------
# cmd_reset — drop and recreate all databases
# ---------------------------------------------------------------------------
cmd_reset() {
    header "Resetting Databases"
    warn "This will DROP and recreate all AMOS development databases!"
    echo ""

    # Prompt for confirmation unless --yes flag is passed
    if [ "${1:-}" != "--yes" ] && [ "${1:-}" != "-y" ]; then
        echo -n "  Continue? [y/N] "
        read -r confirm
        if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
            info "Aborted."
            return 0
        fi
    fi

    for db in "${DATABASES[@]}"; do
        info "Dropping database '$db'..."
        # Terminate active connections first
        run_psql "postgres" -c "
            SELECT pg_terminate_backend(pg_stat_activity.pid)
            FROM pg_stat_activity
            WHERE pg_stat_activity.datname = '$db'
            AND pid <> pg_backend_pid();" &>/dev/null || true
        run_psql "postgres" -c "DROP DATABASE IF EXISTS $db;" 2>/dev/null
        success "Dropped '$db'"
    done

    create_databases
    run_migrations

    success "All databases reset and migrations applied"
}

# ---------------------------------------------------------------------------
# cmd_status — show what's running
# ---------------------------------------------------------------------------
cmd_status() {
    header "AMOS Dev Environment Status"

    # PostgreSQL
    echo -e "${BOLD}PostgreSQL:${NC}"
    local pg_found=false
    local crt
    crt="$(container_cmd)"

    if [ -n "$crt" ]; then
        local pg_status
        pg_status=$(docker ps --filter "name=$PG_CONTAINER" --format "  Container: {{.Names}}  Status: {{.Status}}  Ports: {{.Ports}}" 2>/dev/null || echo "")
        if [ -n "$pg_status" ]; then
            echo -e "  ${GREEN}Running (Docker)${NC}"
            echo "$pg_status"
            pg_found=true
        fi
    fi

    if ! $pg_found && pg_isready -h localhost -p "$POSTGRES_PORT" &>/dev/null 2>&1; then
        echo -e "  ${GREEN}Running (local) on port $POSTGRES_PORT${NC}"
        pg_found=true
    fi

    if ! $pg_found; then
        echo -e "  ${RED}Not running${NC}"
    fi

    echo ""

    # Redis
    echo -e "${BOLD}Redis:${NC}"
    local redis_found=false

    if [ -n "$crt" ]; then
        local redis_status
        redis_status=$(docker ps --filter "name=$REDIS_CONTAINER" --format "  Container: {{.Names}}  Status: {{.Status}}  Ports: {{.Ports}}" 2>/dev/null || echo "")
        if [ -n "$redis_status" ]; then
            echo -e "  ${GREEN}Running (Docker)${NC}"
            echo "$redis_status"
            redis_found=true
        fi
    fi

    if ! $redis_found && command -v redis-cli &>/dev/null && redis-cli -p "$REDIS_PORT" ping &>/dev/null 2>&1; then
        echo -e "  ${GREEN}Running (local) on port $REDIS_PORT${NC}"
        redis_found=true
    fi

    if ! $redis_found; then
        echo -e "  ${RED}Not running${NC}"
    fi

    echo ""

    # Databases
    echo -e "${BOLD}Databases:${NC}"
    if $pg_found; then
        for db in "${DATABASES[@]}"; do
            local exists
            exists=$(run_psql "postgres" -tAc "SELECT 1 FROM pg_database WHERE datname = '$db'" 2>/dev/null || echo "")
            if [ "$exists" = "1" ]; then
                # Check pgvector
                local has_vector
                has_vector=$(run_psql "$db" -tAc "SELECT 1 FROM pg_extension WHERE extname = 'vector'" 2>/dev/null || echo "")
                if [ "$has_vector" = "1" ]; then
                    echo -e "  ${GREEN}$db${NC} (pgvector enabled)"
                else
                    echo -e "  ${YELLOW}$db${NC} (pgvector NOT enabled)"
                fi
            else
                echo -e "  ${RED}$db${NC} (does not exist)"
            fi
        done
    else
        echo -e "  ${YELLOW}Cannot check — PostgreSQL not running${NC}"
    fi

    echo ""

    # AMOS services
    echo -e "${BOLD}AMOS Services:${NC}"
    for svc_info in "Harness:3000" "Platform:4000" "Relay:4100" "Agent:3100"; do
        local svc_name="${svc_info%%:*}"
        local svc_port="${svc_info##*:}"
        if curl -sf "http://localhost:$svc_port/health" &>/dev/null; then
            echo -e "  ${GREEN}$svc_name${NC} — http://localhost:$svc_port"
        else
            echo -e "  ${RED}$svc_name${NC} — not responding on port $svc_port"
        fi
    done

    echo ""
}

# ---------------------------------------------------------------------------
# Main dispatch
# ---------------------------------------------------------------------------
main() {
    local cmd="${1:-setup}"

    case "$cmd" in
        setup|"")    cmd_setup ;;
        start)       cmd_start ;;
        stop)        cmd_stop ;;
        reset)       shift; cmd_reset "$@" ;;
        status)      cmd_status ;;
        -h|--help|help)
            echo "Usage: $0 [command]"
            echo ""
            echo "Commands:"
            echo "  setup   Full setup: prerequisites, databases, migrations (default)"
            echo "  start   Start PostgreSQL + Redis"
            echo "  stop    Stop PostgreSQL + Redis"
            echo "  reset   Drop and recreate all databases"
            echo "  status  Show what's running"
            echo ""
            ;;
        *)
            error "Unknown command: $cmd"
            echo "Run '$0 --help' for usage."
            exit 1
            ;;
    esac
}

main "$@"
