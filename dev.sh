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

# macOS: use system cert bundle to avoid rustls-native-certs parsing issues
if [ "$(uname)" = "Darwin" ]; then
    export SSL_CERT_FILE=/etc/ssl/cert.pem
fi

case "${1:-}" in
    infra)
        echo -e "${GREEN}Starting infrastructure (Postgres + MinIO + OpenObserve)...${NC}"
        cd docker && docker compose up
        ;;
    api)
        echo -e "${GREEN}Starting Rust API (release build)...${NC}"
        RUST_LOG=info,poddyclip_api::processing=debug cargo run --release -p poddyclip-api
        ;;
    web)
        echo -e "${GREEN}Starting Phoenix (with admin dashboard)...${NC}"
        cd backend && ADMIN_ENABLED=false mix compile --force && ADMIN_ENABLED=false ADMIN_PASSWORD=dev mix phx.server
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
    build)
        echo -e "${GREEN}Building Docker images...${NC}"
        docker build --network=host -f Dockerfile.api -t malgamoe/stuff:api .
        docker build --network=host -f backend/Dockerfile -t malgamoe/stuff:phoenix ./backend
        echo -e "${GREEN}Done! Images: malgamoe/stuff:api, malgamoe/stuff:phoenix${NC}"
        ;;
    build-no-cache)
        echo -e "${GREEN}Building Docker images...${NC}"
        docker build --no-cache --network=host -f Dockerfile.api -t malgamoe/stuff:api .
        docker build --no-cache --network=host -f backend/Dockerfile -t malgamoe/stuff:phoenix ./backend
        echo -e "${GREEN}Done! Images: malgamoe/stuff:api, malgamoe/stuff:phoenix${NC}"
        ;;
    build-api)
        echo -e "${GREEN}Building Rust API image...${NC}"
        docker build --network=host -f Dockerfile.api -t malgamoe/stuff:api .
        ;;
    build-phoenix)
        echo -e "${GREEN}Building Phoenix image...${NC}"
        docker build --network=host -f backend/Dockerfile -t malgamoe/stuff:phoenix ./backend
        ;;
    push)
        echo -e "${GREEN}Pushing images to Docker Hub...${NC}"
        docker push malgamoe/stuff:api
        docker push malgamoe/stuff:phoenix
        echo -e "${GREEN}Done!${NC}"
        ;;
    push-api)
        echo -e "${GREEN}Pushing Rust API image...${NC}"
        docker push malgamoe/stuff:api
        ;;
    push-phoenix)
        echo -e "${GREEN}Pushing Phoenix image...${NC}"
        docker push malgamoe/stuff:phoenix
        ;;
    deploy)
        echo -e "${GREEN}Building and pushing all images...${NC}"
        $0 build
        $0 push
        echo -e "${GREEN}Images pushed! On server run: docker compose -f docker-compose.prod.yml pull && docker compose -f docker-compose.prod.yml up -d${NC}"
        ;;
    deploy-clean)
        echo -e "${GREEN}Building and pushing all images...${NC}"
        $0 build-no-cache
        $0 push
        echo -e "${GREEN}Images pushed! On server run: docker compose -f docker-compose.prod.yml pull && docker compose -f docker-compose.prod.yml up -d${NC}"
        ;;
    *)
        echo "Usage: $0 {infra|api|web|stop|reset-db|seed|build|push|deploy}"
        echo ""
        echo "  infra          - Start Postgres + MinIO + OpenObserve (docker)"
        echo "  api            - Start Rust API (port 3000)"
        echo "  web            - Start Phoenix (port 4000)"
        echo "  stop           - Stop all services"
        echo "  reset-db       - Drop, recreate, migrate, and seed database"
        echo "  seed           - Run seeds only (create/update plans)"
        echo ""
        echo "  build          - Build both Docker images"
        echo "  build-no-cache - Build both images without cache"
        echo "  build-api      - Build Rust API image only"
        echo "  build-phoenix  - Build Phoenix image only"
        echo "  push           - Push both images to Docker Hub"
        echo "  push-api       - Push Rust API image only"
        echo "  push-phoenix   - Push Phoenix image only"
        echo "  deploy         - Build + push all"
        echo "  deploy-clean   - Build (no cache) + push all"
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
