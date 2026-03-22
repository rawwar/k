#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/webapp"

if ! command -v node &> /dev/null; then
    echo "Error: Node.js is not installed. Please install Node.js 18+ first."
    echo "Visit: https://nodejs.org/"
    exit 1
fi

NODE_VERSION=$(node -v | cut -d'v' -f2 | cut -d'.' -f1)
if [ "$NODE_VERSION" -lt 18 ]; then
    echo "Error: Node.js 18+ is required. Current version: $(node -v)"
    exit 1
fi

if [ -d "node_modules" ]; then
    echo "Dependencies already installed (node_modules/ exists). Skipping npm install."
else
    echo "Installing dependencies..."
    npm install
fi

echo ""
echo "NOTE: The code snapshots in learn/code/ are Rust projects."
echo "To compile them, you need Rust and Cargo installed: https://rustup.rs/"
echo ""
echo "Starting VitePress dev server..."
echo "The site will be available at: http://localhost:5173"
echo ""
npx vitepress dev
