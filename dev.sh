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
        echo -e "${GREEN}Starting infrastructure (Postgres + Garage)...${NC}"
        cd docker && docker compose up
        ;;
    api)
        echo -e "${GREEN}Starting Rust API...${NC}"
        cargo run -p poddyclip-api
        ;;
    web)
        echo -e "${GREEN}Starting Phoenix...${NC}"
        cd backend && mix phx.server
        ;;
    stop)
        echo -e "${YELLOW}Stopping all services...${NC}"
        pkill -f "poddyclip-api" 2>/dev/null || true
        pkill -f "beam.smp.*poddyclip" 2>/dev/null || true
        cd docker && docker compose down
        ;;
    *)
        echo "Usage: $0 {infra|api|web|stop}"
        echo ""
        echo "  infra  - Start Postgres + Garage (docker)"
        echo "  api    - Start Rust API (port 3000)"
        echo "  web    - Start Phoenix (port 4000)"
        echo "  stop   - Stop all services"
        echo ""
        echo "Run in 3 terminals:"
        echo "  Terminal 1: ./dev.sh infra"
        echo "  Terminal 2: ./dev.sh api"
        echo "  Terminal 3: ./dev.sh web"
        ;;
esac
