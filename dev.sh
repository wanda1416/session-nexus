#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

case "${1:-}" in
  -h|--help)
    cat <<'EOF'
Usage: ./dev.sh [option] [args...]

Options:
  (default)       Start Tauri desktop app in dev mode (pnpm tauri dev)
  --web, --vite   Start frontend-only Vite dev server (pnpm dev)
  --test          Run Rust unit tests (cargo test --lib)
  --check         Run TypeScript/Vite build and Cargo check
  --build         Build Tauri release bundle (pnpm tauri build)
  -h, --help      Show this help message
EOF
    ;;
  --web|--vite)
    shift
    exec pnpm dev "$@"
    ;;
  --test)
    shift
    exec cargo test --manifest-path src-tauri/Cargo.toml --lib "$@"
    ;;
  --check)
    pnpm build
    exec cargo check --manifest-path src-tauri/Cargo.toml
    ;;
  --build)
    shift
    exec pnpm tauri build "$@"
    ;;
  *)
    if [ ! -d "node_modules" ]; then
      echo "[dev] node_modules not found, running pnpm install..."
      pnpm install
    fi
    exec pnpm tauri dev "$@"
    ;;
esac
