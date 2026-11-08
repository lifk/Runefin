#!/usr/bin/env bash
# Point git at the tracked hooks. Run once per clone.
set -euo pipefail
cd "$(dirname "$0")/.."
chmod +x .githooks/pre-commit
git config core.hooksPath .githooks
echo "hooks installed: $(git config core.hooksPath)"
echo "pre-commit now refuses tokens, private hosts and local secret files."
