#!/usr/bin/env bash
# ONLYOFFICE Docs through the browser protocol: the Document Server (Community, AGPL-3.0, an
# external tool we only run) in Docker on the runner, pinned by digest; host.html served by
# server.ts; then browser/run.sh. JWT is disabled explicitly (JWT_ENABLED=false): the server and
# the host page are both on the runner's loopback and bridge, nothing is exposed. Private
# addresses are allowed (ALLOW_PRIVATE_IP_ADDRESS=true) so the container may fetch the corpus
# from the host.
# usage: run.sh CDOC_BENCH PLAN CORPUS_DIR OUT_DIR ARTIFACT_DIR    (env as browser/run.sh)
set -euo pipefail
bin=$1 plan=$2 corpus=$3 out=$4 artifacts=$5
here=$(cd "$(dirname "$0")" && pwd)
image="onlyoffice/documentserver:10.0.0.1@sha256:ca17889f10b11141cc6441836aebdd2835cf3e1b49681b2fd6e155c94b7850b3"
ds_port=8080 host_port=8090 name=cdoc-onlyoffice
mkdir -p "$artifacts"
cleanup() {
  kill "${server_pid:-0}" 2>/dev/null || true
  docker logs "$name" > "$artifacts/onlyoffice-documentserver.log" 2>&1 || true
  docker rm -f "$name" >/dev/null 2>&1 || true
}
trap cleanup EXIT
docker pull -q "$image"
docker run -d --name "$name" -p "127.0.0.1:$ds_port:80" \
  --add-host=host.docker.internal:host-gateway \
  -e JWT_ENABLED=false -e ALLOW_PRIVATE_IP_ADDRESS=true "$image" >/dev/null
node "$here/../dist/onlyoffice/server.js" --port "$host_port" --corpus "$corpus" \
  --ds-url "http://localhost:$ds_port" --doc-base "http://host.docker.internal:$host_port" \
  > "$artifacts/onlyoffice-host.log" 2>&1 &
server_pid=$!
# The image's entrypoint restarts the document service after generating its caches, and logs this
# line last; a healthcheck passed before it can be answered by the service that is about to stop.
for _ in $(seq 1 120); do
  docker logs "$name" 2>&1 | grep -q "Plugins initialization finished" && break
  sleep 5
done
docker logs "$name" 2>&1 | grep -q "Plugins initialization finished"
# Then the Document Server answers its healthcheck with "true" once every service is up.
for _ in $(seq 1 120); do
  [ "$(curl -fsS "http://localhost:$ds_port/healthcheck" 2>/dev/null || true)" = "true" ] && break
  sleep 5
done
test "$(curl -fsS "http://localhost:$ds_port/healthcheck")" = "true"
curl -fsS "http://localhost:$host_port/" -o /dev/null
ONLYOFFICE_HOST_URL="http://localhost:$host_port" ONLYOFFICE_IMAGE="$image" \
  "$here/../browser/run.sh" onlyoffice "$bin" "$plan" "$corpus" "$out" "$artifacts"
