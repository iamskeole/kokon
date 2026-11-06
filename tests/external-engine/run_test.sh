#!/bin/bash
# Integration test: kokon-service (external-engine mode) against a mock llama-swap gateway.
# See the README in this folder. Everything is loopback-only and killed on exit.
set -u
D=${QLS_EXT_TEST_DIR:-/tmp/qls-external-test}
BIN=${QLS_BIN:-$(cd "$(dirname "$0")/../.." && pwd)/target/debug/kokon-service}
PORT=${QLS_EXT_SERVICE_PORT:-39902}
GW=${QLS_EXT_GATEWAY_PORT:-39990}
S="http://127.0.0.1:$PORT/api/v1"

rm -rf "$D/data" "$D/config" "$D/requests.log"
mkdir -p "$D/data" "$D/config"
cp "$(cd "$(dirname "$0")" && pwd)/mock_gateway.py" "$D/mock_gateway.py"
cat > "$D/config/config.json" <<EOF
{
  "schemaVersion": 13,
  "servicePort": $PORT,
  "serverPort": 39900,
  "setupComplete": true,
  "externalEngine": {
    "gateway": "http://127.0.0.1:$GW/upstream",
    "model": "kokon-turbo",
    "modelName": "qwen-image-2.1-turbo-Q4_K_M.gguf",
    "sample": {}
  }
}
EOF

python3 "$D/mock_gateway.py" $GW 1.5 "$D/requests.log" > "$D/gateway.log" 2>&1 &
GWPID=$!
QLS_DATA_DIR=$D/data QLS_CONFIG_DIR=$D/config "$BIN" --port $PORT --persistent > "$D/service.log" 2>&1 &
SPID=$!
trap 'kill $GWPID $SPID 2>/dev/null' EXIT

for i in $(seq 1 50); do [ -f "$D/data/service.json" ] && break; sleep 0.2; done
TOKEN=$(python3 -c "import json;print(json.load(open('$D/data/service.json'))['token'])" 2>/dev/null || echo MISSING)
[ "$TOKEN" = MISSING ] && { echo "FATAL: no token in service.json"; cat "$D/service.log"; exit 1; }
H="Authorization: Bearer $TOKEN"

pass=0; fail=0
check() {
  if [ "$2" = "$3" ]; then echo "PASS: $1"; pass=$((pass+1)); else echo "FAIL: $1  (expected [$2], got [$3])"; fail=$((fail+1)); fi
}
# server_field STATE OWNED MODEL  <- reads the status JSON on stdin
sf() { curl -s -H "$H" $S/status | python3 -c "
import json, sys
s = json.load(sys.stdin)['server']
print(s['state'], str(s['owned']).lower(), s.get('model') or '-')"; }
# gw_state FIELD <- mock's internal state
gs() { curl -s http://127.0.0.1:$GW/_control/state | python3 -c "import json,sys;print(json.load(sys.stdin)['$1'])"; }
# job_status JOBID <- the service's job view
js() { curl -s -H "$H" $S/jobs/"$1" | python3 -c "import json,sys;print(json.load(sys.stdin)['status'])"; }
upstream_reqs() { grep -c "upstream/" "$D/requests.log" 2>/dev/null || echo 0; }

echo "=== 1. rest: no preload on startup ==="
sleep 2
R=$(sf)
check "service at rest reports stopped, not owned, no model" "stopped false -" "$R"
check "zero engine requests while idle (a resting UI visit preloads nothing)" 0 "$(upstream_reqs)"

echo "=== 2. load: POST /backend/start ==="
curl -s -X POST -H "$H" $S/backend/start > /dev/null
sleep 4
R=$(sf)
check "state after load request" "ready false qwen-image-2.1-turbo-Q4_K_M.gguf" "$R"
check "gateway loaded the model exactly once" 1 "$(gs loads)"

echo "=== 3. job through the gateway ==="
JID=$(curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/jobs -d '{"prompt":"a red square","mode":"generate","quality":"fast","width":64,"height":64}' | python3 -c "import json,sys;print(json.load(sys.stdin)['id'])")
JST=queued
for i in $(seq 1 40); do
  sleep 0.5
  JST=$(js "$JID")
  [ "$JST" = done ] || [ "$JST" = failed ] && break
done
check "job completed through /upstream/kokon-turbo" done "$JST"
grep -q "upstream/kokon-turbo/sdcpp/v1/img_gen" "$D/requests.log" \
  && { echo "PASS: img_gen request used the /upstream prefix"; pass=$((pass+1)); } \
  || { echo "FAIL: no /upstream img_gen request logged"; fail=$((fail+1)); }
JERR=$(curl -s -H "$H" $S/jobs/"$JID" | python3 -c "import json,sys;d=json.load(sys.stdin);print(d.get('error') or '-')")
[ "$JERR" = "-" ] || echo "  (job error: $JERR)"

echo "=== 4. flavor switch: turbo -> full (records the pick, touches nothing) ==="
curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/models/switch -d "{
  \"gateway\": \"http://127.0.0.1:$GW/upstream\",
  \"model\": \"kokon-full\",
  \"modelName\": \"qwen-image-2.1-Q8_0.gguf\",
  \"sample\": {\"steps\": 20, \"cfg\": 6.0, \"sampler\": \"euler\", \"clear_sigmas\": true}
}" > /dev/null
sleep 1
R=$(sf)
check "after switch: still ready on turbo, no load started" "ready false qwen-image-2.1-turbo-Q4_K_M.gguf" "$R"
check "gateway: no load, no eviction for a mere switch" "1 0" "$(gs loads) $(gs evicts)"
check "config now selects full" "kokon-full" "$(curl -s -H "$H" $S/config | python3 -c "import json,sys;print(json.load(sys.stdin)['externalEngine']['model'])")"

echo "=== 5. the next generation triggers the swap ==="
JID=$(curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/jobs -d '{"prompt":"a red square","mode":"generate","quality":"fast","width":64,"height":64}' | python3 -c "import json,sys;print(json.load(sys.stdin)['id'])")
for i in $(seq 1 30); do S5=$(js $JID); [ "$S5" = done ] && break; [ "$S5" = error ] && break; sleep 1; done
check "job on the newly picked model finished" "done" "$(js $JID)"
check "gateway evicted the old tenant once and loaded the new one" "2 1" "$(gs loads) $(gs evicts)"
R=$(sf)
check "state: full loaded, still not owned" "ready false qwen-image-2.1-Q8_0.gguf" "$R"

echo "=== 6. gateway down ==="
kill $GWPID 2>/dev/null; wait $GWPID 2>/dev/null
curl -s -X POST -H "$H" $S/backend/start > /dev/null
sleep 5
R=$(sf)
check "after death: error state, not owned" "error false qwen-image-2.1-Q8_0.gguf" "$R"
MSG=$(curl -s -H "$H" $S/status | python3 -c "import json,sys;print((json.load(sys.stdin)['server']['error'] or {}).get('message','-'))")
case "$MSG" in *"not reachable"*) echo "PASS: error says the engine is not reachable (a switch, not a crash)"; pass=$((pass+1));; *) echo "FAIL: unexpected error message: $MSG"; fail=$((fail+1));; esac
pgrep -f "sd-server" > /dev/null && { echo "FAIL: service spawned a local engine!"; fail=$((fail+1)); } || { echo "PASS: no local engine was spawned"; pass=$((pass+1)); }

echo "=== 7. recovery ==="
python3 "$D/mock_gateway.py" $GW 1.5 "$D/requests.log" > "$D/gateway.log" 2>&1 &
GWPID=$!
sleep 1
curl -s -X POST -H "$H" $S/backend/start > /dev/null
sleep 4
R=$(sf)
check "recovered: full loaded and ready" "ready false qwen-image-2.1-Q8_0.gguf" "$R"

echo
echo "RESULT: $pass passed, $fail failed"
exit $fail
