#!/bin/bash
# Start all services for local development

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

# Load env
if [ -f .env.local ]; then
    export $(grep -v '^#' .env.local | xargs)
elif [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

case "${1:-}" in
    infra)
        echo -e "${GREEN}Starting infrastructure (Postgres + MinIO + OpenObserve)...${NC}"
        cd docker && docker compose up
        ;;
    api)
        echo -e "${GREEN}Starting Rust API (release build)...${NC}"
        cargo run --release -p poddyclip-api
        ;;
    web)
        echo -e "${GREEN}Starting Phoenix (with admin dashboard)...${NC}"
        cd backend && ADMIN_ENABLED=true mix compile --force && ADMIN_ENABLED=true ADMIN_PASSWORD=dev mix phx.server
        ;;
    stop)
        echo -e "${YELLOW}Stopping all services...${NC}"
        pkill -f "poddyclip-api" 2>/dev/null || true
        pkill -f "beam.smp.*poddyclip" 2>/dev/null || true
        cd docker && docker compose down
        ;;
    reset-db)
        echo -e "${YELLOW}Resetting database...${NC}"
        cd backend
        mix compile --force
        echo -e "${RED}Dropping database...${NC}"
        mix ecto.drop
        echo -e "${GREEN}Creating database...${NC}"
        mix ecto.create
        echo -e "${GREEN}Running migrations...${NC}"
        mix ecto.migrate
        echo -e "${GREEN}Running seeds...${NC}"
        mix run priv/repo/seeds.exs
        echo -e "${GREEN}Database reset complete!${NC}"
        ;;
    seed)
        echo -e "${GREEN}Running seeds...${NC}"
        cd backend && mix run priv/repo/seeds.exs
        ;;
    *)
        echo "Usage: $0 {infra|api|web|stop|reset-db|seed}"
        echo ""
        echo "  infra    - Start Postgres + MinIO + OpenObserve (docker)"
        echo "  api      - Start Rust API (port 3000)"
        echo "  web      - Start Phoenix (port 4000)"
        echo "  stop     - Stop all services"
        echo "  reset-db - Drop, recreate, migrate, and seed database"
        echo "  seed     - Run seeds only (create/update plans)"
        echo ""
        echo "Run in 3 terminals:"
        echo "  Terminal 1: ./dev.sh infra"
        echo "  Terminal 2: ./dev.sh api"
        echo "  Terminal 3: ./dev.sh web"
        echo ""
        echo "Services:"
        echo "  Postgres:    localhost:5432"
        echo "  MinIO:       localhost:9000 (S3), localhost:9001 (console)"
        echo "  OpenObserve: localhost:5080 (login: admin@poddyclip.local / dev)"
        ;;
esac
