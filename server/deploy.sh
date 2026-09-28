#!/bin/bash
# Build ribbit-server on the host from the current commit and (re)start it.
# Usage: server/deploy.sh <ssh-host>   (host already set up — see server/README.md)
# Only committed code ships: the build context is `git archive HEAD core server`.
set -euo pipefail
host="$1"
cd "$(git rev-parse --show-toplevel)"
if ! git diff --quiet HEAD -- core server; then
  echo "core/ or server/ has uncommitted changes — commit first" >&2
  exit 1
fi
tag=$(git rev-parse --short HEAD)
git archive --format=tar HEAD core server | ssh "$host" "docker build -q -f server/Dockerfile -t ribbit-server:$tag -"
scp -q server/compose.yml "$host:/opt/ribbit-server/compose.yml"
ssh "$host" "cd /opt/ribbit-server && sed -i 's/^TAG=.*/TAG=$tag/' .env && docker compose up -d && docker image prune -f >/dev/null"
echo "deployed ribbit-server:$tag to $host"
