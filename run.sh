#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --locked
exec ./target/release/technetium-battle-bot "$1"
