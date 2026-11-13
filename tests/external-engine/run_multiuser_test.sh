#!/bin/bash
# Multi-user (D-MU12/D-MU13): the real service against the mock gateway, with two named
# accounts signing in through cookie jars (like browsers): both logins, one job each, gallery
# and file scoping, per-user projects (create, cross-user 404s, the stuffed-reference filter,
# the import namespacing, the stream filter), the operator's filter, and the account
# lifecycle (delete, gate off). No GPU: the mock is the engine.
# See run_test.sh for the single-user variant.
set -u
D=${QLS_MU_TEST_DIR:-/tmp/qls-mu-test}
BIN=${QLS_BIN:-$(cd "$(dirname "$0")/../.." && pwd)/target/debug/kokon-service}
PORT=${QLS_MU_SERVICE_PORT:-39913}
GW=${QLS_MU_GATEWAY_PORT:-39992}
S="http://127.0.0.1:$PORT/api/v1"
JAR_A=$D/jar-alice; JAR_B=$D/jar-bob; JAR_O=$D/jar-operator
pass=0; fail=0
check() {
  if [ "$2" = "$3" ]; then echo "PASS: $1"; pass=$((pass+1)); else echo "FAIL: $1  (expected [$2], got [$3])"; fail=$((fail+1)); fi
}
jget() { python3 -c "import json,sys;print(json.load(sys.stdin)$1)"; }
jb() { python3 -c "import json,sys;print(str(json.load(sys.stdin)$1).lower())"; }
jc() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

rm -rf "$D/data" "$D/config" "$D/requests.log"
mkdir -p "$D/data" "$D/config"
cp "$(cd "$(dirname "$0")" && pwd)/mock_gateway.py" "$D/mock_gateway.py"
cat > "$D/config/config.json" <<EOF
{
  "schemaVersion": 15,
  "servicePort": $PORT,
  "serverPort": 39914,
  "setupComplete": true,
  "outputDir": "$D/gallery",
  "externalEngine": {
    "gateway": "http://127.0.0.1:$GW/upstream",
    "model": "kokon-qwen-image-2.1-turbo",
    "modelName": "qwen-image-2.1-turbo-Q4_K_M.gguf",
    "sample": {}
  }
}
EOF

python3 "$D/mock_gateway.py" $GW 0.5 "$D/requests.log" > "$D/gateway.log" 2>&1 &
GWPID=$!
QLS_DATA_DIR=$D/data QLS_CONFIG_DIR=$D/config "$BIN" --port $PORT --persistent > "$D/service.log" 2>&1 &
SPID=$!
trap 'kill $GWPID $SPID 2>/dev/null' EXIT

for i in $(seq 1 50); do [ -f "$D/data/service.json" ] && break; sleep 0.2; done
TOKEN=$(python3 -c "import json;print(json.load(open('$D/data/service.json'))['token'])" 2>/dev/null || echo MISSING)
[ "$TOKEN" = MISSING ] && { echo "FATAL: no token in service.json"; cat "$D/service.log"; exit 1; }
H="Authorization: Bearer $TOKEN"
CS="x-kokon-csrf: 1"
OPPW='op pass 42'
ALICEPW='alice pass 1'
BOBPW='bob pass 2'

wait_job() { # JAR JOBID: until the job is terminal (0 done, 1 failed, 2 timeout)
  for i in $(seq 1 60); do
    ST=$(curl -s -b "$1" $S/jobs/"$2" 2>/dev/null | jget "['status']" || echo missing)
    [ "$ST" = done ] && return 0
    [ "$ST" = failed ] && return 1
    [ "$ST" = missing ] || sleep 0.5
  done
  return 2
}

echo "=== 1. operator sets the shared password, turns accounts on, creates two ==="
curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/remote/password -d "{\"password\":\"$OPPW\"}" > /dev/null
check "shared password set" true "$(curl -s -H "$H" $S/remote | jb "['password_set']")"
curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/users/enable -d '{"on": true}' > /dev/null
check "multi_user on" true "$(curl -s -H "$H" $S/remote | jb "['multi_user']")"
curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/users -d "{\"name\":\"alice\",\"password\":\"$ALICEPW\"}" > /dev/null
curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/users -d "{\"name\":\"bob\",\"password\":\"$BOBPW\"}" > /dev/null
check "two accounts listed" 2 "$(curl -s -H "$H" $S/users | python3 -c "import json,sys;print(len(json.load(sys.stdin)['users']))")"
BOBID=$(curl -s -H "$H" $S/users | python3 -c "import json,sys;print([u for u in json.load(sys.stdin)['users'] if u['name']=='bob'][0]['id'])")
check "bob's id found" "ok" "$([ -n "$BOBID" ] && echo ok || echo none)"

echo "=== 2. logins: the two accounts and the operator (shared password) ==="
R=$(curl -s -c "$JAR_A" -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d "{\"password\":\"$ALICEPW\",\"username\":\"alice\"}")
check "alice signs in" true "$(echo "$R" | jb "['signed_in']")"
R=$(curl -s -c "$JAR_B" -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d "{\"password\":\"$BOBPW\",\"username\":\"bob\"}")
check "bob signs in" true "$(echo "$R" | jb "['signed_in']")"
R=$(curl -s -c "$JAR_O" -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d "{\"password\":\"$OPPW\"}")
check "operator signs in with the shared password" true "$(echo "$R" | jb "['signed_in']")"
check "alice's session says who" "alice true" "$(curl -s -b "$JAR_A" $S/auth/session | python3 -c "import json,sys;s=json.load(sys.stdin);print(s['user'], str(s['multi_user']).lower(), sep=' ')")"
check "operator's session has no name" "None true" "$(curl -s -b "$JAR_O" $S/auth/session | python3 -c "import json,sys;s=json.load(sys.stdin);print(s['user'], str(s['multi_user']).lower(), sep=' ')")"
check "wrong password for a name" "401" "$(jc -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d "{\"password\":\"nope\",\"username\":\"alice\"}")"
R=$(curl -s -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d '{"password":"nope","username":"ghost"}')
check "unknown name" "Unknown name." "$(echo "$R" | jget "['error']['message']")"

echo "=== 3. one job each, through the gateway ==="
JA=$(curl -s -b "$JAR_A" -X POST -H "$CS" -H 'Content-Type: application/json' $S/jobs -d '{"prompt":"alice red square","mode":"generate","quality":"fast","width":64,"height":64}' | jget "['id']")
JB=$(curl -s -b "$JAR_B" -X POST -H "$CS" -H 'Content-Type: application/json' $S/jobs -d '{"prompt":"bob blue square","mode":"generate","quality":"fast","width":64,"height":64}' | jget "['id']")
wait_job "$JAR_A" "$JA"; check "alice's job done" 0 $?
wait_job "$JAR_B" "$JB"; check "bob's job done" 0 $?
check "alice's job says alice" "alice" "$(curl -s -b "$JAR_A" $S/jobs/$JA | jget "['user']")"
check "bob's job says bob" "bob" "$(curl -s -b "$JAR_B" $S/jobs/$JB | jget "['user']")"

echo "=== 4. the job list and the per-job endpoints are scoped ==="
check "alice lists her job only" 1 "$(curl -s -b "$JAR_A" $S/jobs | python3 -c "import json,sys;print(len(json.load(sys.stdin)))")"
check "bob lists his job only" 1 "$(curl -s -b "$JAR_B" $S/jobs | python3 -c "import json,sys;print(len(json.load(sys.stdin)))")"
check "operator lists both" 2 "$(curl -s -b "$JAR_O" $S/jobs | python3 -c "import json,sys;print(len(json.load(sys.stdin)))")"
check "alice's view of bob's job: 404" "404" "$(jc -b "$JAR_A" $S/jobs/$JB)"
check "alice cancelling bob's job: 404" "404" "$(jc -b "$JAR_A" -X POST -H "$CS" $S/jobs/$JB/cancel)"
check "alice waiting on bob's job: 404" "404" "$(jc -b "$JAR_A" "$S/jobs/$JB/wait?timeout=1")"

echo "=== 5. the SSE stream: alice hears her job, not bob's ==="
(timeout 20 curl -sN -b "$JAR_A" $S/events > "$D/sse-alice.log" 2>/dev/null || true) &
SSEPID=$!
sleep 1
JC=$(curl -s -b "$JAR_B" -X POST -H "$CS" -H 'Content-Type: application/json' $S/jobs -d '{"prompt":"bob second","mode":"generate","quality":"fast","width":64,"height":64}' | jget "['id']")
JD=$(curl -s -b "$JAR_A" -X POST -H "$CS" -H 'Content-Type: application/json' $S/jobs -d '{"prompt":"alice second","mode":"generate","quality":"fast","width":64,"height":64}' | jget "['id']")
wait_job "$JAR_B" "$JC"; check "bob's second job done" 0 $?
wait_job "$JAR_A" "$JD"; check "alice's second job done" 0 $?
wait $SSEPID 2>/dev/null; sleep 0.5
grep -q "\"$JD\"" "$D/sse-alice.log" && { echo "PASS: alice's stream carries her own job"; pass=$((pass+1)); } || { echo "FAIL: alice's stream lacks her own job $JD"; fail=$((fail+1)); }
grep -q "\"$JC\"" "$D/sse-alice.log" && { echo "FAIL: alice's stream carries bob's job $JC"; fail=$((fail+1)); } || { echo "PASS: alice's stream drops bob's job"; pass=$((pass+1)); }

echo "=== 6. the gallery is scoped, and the files live in the owner's folder ==="
check "alice lists her two rows" 2 "$(curl -s -b "$JAR_A" $S/gallery | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "bob lists his two rows" 2 "$(curl -s -b "$JAR_B" $S/gallery | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "operator lists all four" 4 "$(curl -s -b "$JAR_O" $S/gallery | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
BOBIMG=$(curl -s -b "$JAR_B" "$S/gallery?limit=1" | python3 -c "import json,sys;print(json.load(sys.stdin)['items'][0]['id'])")
ALICEIMG=$(curl -s -b "$JAR_A" "$S/gallery?limit=1" | python3 -c "import json,sys;print(json.load(sys.stdin)['items'][0]['id'])")
check "alice's row says alice" "alice" "$(curl -s -b "$JAR_A" $S/gallery/$ALICEIMG | jget "['user']")"
check "alice's file is in her folder" "yes" "$(curl -s -b "$JAR_A" $S/gallery/$ALICEIMG | python3 -c "import json,sys,os;p=json.load(sys.stdin)['file_path'];print('yes' if '/alice/' in p and os.path.isfile(p) else 'no')")"
check "bob's file is in his folder" "yes" "$(curl -s -b "$JAR_O" $S/gallery/$BOBIMG | python3 -c "import json,sys,os;p=json.load(sys.stdin)['file_path'];print('yes' if '/bob/' in p and os.path.isfile(p) else 'no')")"
check "alice cannot widen her list with ?user=bob" 2 "$(curl -s -b "$JAR_A" "$S/gallery?user=bob" | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "alice opening bob's row: 404" "404" "$(jc -b "$JAR_A" $S/gallery/$BOBIMG)"
check "alice thumbing bob's row: 404" "404" "$(jc -b "$JAR_A" $S/gallery/$BOBIMG/thumb)"
check "alice favouriting bob's row: 404" "404" "$(jc -b "$JAR_A" -X PATCH -H "$CS" -H 'Content-Type: application/json' -d '{"favorite":true}' $S/gallery/$BOBIMG)"
check "bob editing from alice's row: 404" "404" "$(jc -b "$JAR_B" -X POST -H "$CS" -H 'Content-Type: application/json' -d "{\"prompt\":\"x\",\"mode\":\"edit\",\"images\":[\"gen:$ALICEIMG\"]}" $S/plan)"
check "operator's ?user=alice" 2 "$(curl -s -b "$JAR_O" "$S/gallery?user=alice" | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "operator's ?user=bob" 2 "$(curl -s -b "$JAR_O" "$S/gallery?user=bob" | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "operator's ?user=me (no owner rows left)" 0 "$(curl -s -b "$JAR_O" "$S/gallery?user=me" | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "named account managing accounts: 403" "403" "$(jc -b "$JAR_A" $S/users)"

echo "=== 7. projects are per user (D-MU13) ==="
# One shared project (the machine's list) and one per account; the items reference the
# owner's own row, so the records checks have something to filter.
curl -s -X PUT -H "$H" -H "$CS" -H 'Content-Type: application/json' $S/projects/p-op \
  -d '{"project":{"meta":{"id":"p-op","title":"op","updatedAt":1,"cover":null,"count":0},"items":[],"selected":null,"rev":0},"known":[]}' > /dev/null
curl -s -b "$JAR_A" -X PUT -H "$CS" -H 'Content-Type: application/json' $S/projects/p-alice \
  -d "{\"project\":{\"meta\":{\"id\":\"p-alice\",\"title\":\"a\",\"updatedAt\":1,\"cover\":null,\"count\":1},\"items\":[{\"key\":\"a\",\"kind\":\"gen\",\"genId\":\"$ALICEIMG\"}],\"selected\":\"a\",\"rev\":0},\"known\":[]}" > /dev/null
curl -s -b "$JAR_B" -X PUT -H "$CS" -H 'Content-Type: application/json' $S/projects/p-bob \
  -d "{\"project\":{\"meta\":{\"id\":\"p-bob\",\"title\":\"b\",\"updatedAt\":1,\"cover\":null,\"count\":1},\"items\":[{\"key\":\"b\",\"kind\":\"gen\",\"genId\":\"$BOBIMG\"}],\"selected\":\"b\",\"rev\":0},\"known\":[]}" > /dev/null
check "the token lists all three" 3 "$(curl -s -H "$H" $S/projects | python3 -c "import json,sys;print(len(json.load(sys.stdin)['projects']))")"
check "alice lists exactly her project" "p-alice" "$(curl -s -b "$JAR_A" $S/projects | python3 -c "import json,sys;print(','.join(p['meta']['id'] for p in json.load(sys.stdin)['projects']))")"
check "bob lists exactly his project" "p-bob" "$(curl -s -b "$JAR_B" $S/projects | python3 -c "import json,sys;print(','.join(p['meta']['id'] for p in json.load(sys.stdin)['projects']))")"
check "the operator lists all three" 3 "$(curl -s -b "$JAR_O" $S/projects | python3 -c "import json,sys;print(len(json.load(sys.stdin)['projects']))")"
check "alice opening the shared project: 404" "404" "$(jc -b "$JAR_A" $S/projects/p-op)"
check "alice opening bob's project: 404" "404" "$(jc -b "$JAR_A" $S/projects/p-bob)"
check "alice editing the shared project: 404" "404" "$(jc -b "$JAR_A" -X PUT -H "$CS" -H 'Content-Type: application/json' -d '{"project":{"meta":{"id":"p-op","title":"x","updatedAt":2,"cover":null,"count":0},"items":[],"selected":null,"rev":0},"known":[]}' $S/projects/p-op)"
check "alice deleting bob's project: 404" "404" "$(jc -b "$JAR_A" -X DELETE -H "$CS" $S/projects/p-bob)"
check "alice's records: her own row only" 1 "$(curl -s -b "$JAR_A" $S/projects/p-alice/records | python3 -c "import json,sys;print(len(json.load(sys.stdin)['generations']))")"
# Stuffing: she can reference bob's row in her items, but the record never comes back.
curl -s -b "$JAR_A" -X PUT -H "$CS" -H 'Content-Type: application/json' $S/projects/p-alice \
  -d "{\"project\":{\"meta\":{\"id\":\"p-alice\",\"title\":\"a\",\"updatedAt\":3,\"cover\":null,\"count\":2},\"items\":[{\"key\":\"a\",\"kind\":\"gen\",\"genId\":\"$ALICEIMG\"},{\"key\":\"s\",\"kind\":\"gen\",\"genId\":\"$BOBIMG\"}],\"selected\":\"s\",\"rev\":1},\"known\":[\"a\"]}" > /dev/null
check "the stuffed reference gets no record for her" 1 "$(curl -s -b "$JAR_A" $S/projects/p-alice/records | python3 -c "import json,sys;print(len(json.load(sys.stdin)['generations']))")"
check "the operator's records: both rows" 2 "$(curl -s -b "$JAR_O" $S/projects/p-alice/records | python3 -c "import json,sys;print(len(json.load(sys.stdin)['generations']))")"
check "alice reading bob's records: 404" "404" "$(jc -b "$JAR_A" $S/projects/p-bob/records)"
# The first-launch import is namespaced: hers merges and creates, the rest is left alone.
R=$(curl -s -b "$JAR_A" -X POST -H "$CS" -H 'Content-Type: application/json' $S/projects/import -d '{"projects":[{"meta":{"id":"p-op","title":"op","updatedAt":1,"cover":null,"count":0},"items":[],"selected":null,"rev":0},{"meta":{"id":"p-bob","title":"b","updatedAt":1,"cover":null,"count":1},"items":[],"selected":null,"rev":0},{"meta":{"id":"p-alice2","title":"a2","updatedAt":1,"cover":null,"count":0},"items":[],"selected":null,"rev":0}]}')
check "the import created only her new project" 1 "$(echo "$R" | python3 -c "import json,sys;print(json.load(sys.stdin)['changed'])")"
check "the import left the shared and bob's projects" 4 "$(curl -s -H "$H" $S/projects | python3 -c "import json,sys;print(len(json.load(sys.stdin)['projects']))")"
# The stream: alice hears her project's news, not bob's.
(timeout 8 curl -sN -b "$JAR_A" $S/events > "$D/sse-projects.log" 2>/dev/null || true) &
SSEP2=$!
sleep 1
curl -s -b "$JAR_B" -X PUT -H "$CS" -H 'Content-Type: application/json' $S/projects/p-bob -d '{"project":{"meta":{"id":"p-bob","title":"b","updatedAt":9,"cover":null,"count":1},"items":[{"key":"b","kind":"gen","genId":"'"$BOBIMG"'"}],"selected":"b","rev":2},"known":["b"]}' > /dev/null
curl -s -b "$JAR_A" -X PUT -H "$CS" -H 'Content-Type: application/json' $S/projects/p-alice -d '{"project":{"meta":{"id":"p-alice","title":"a","updatedAt":4,"cover":null,"count":2},"items":[{"key":"a","kind":"gen","genId":"'"$ALICEIMG"'"},{"key":"s","kind":"gen","genId":"'"$BOBIMG"'"}],"selected":"s","rev":3},"known":["a","s"]}' > /dev/null
wait $SSEP2 2>/dev/null; sleep 0.3
grep -q '"type":"projects","id":"p-alice"' "$D/sse-projects.log" && { echo "PASS: alice's stream carries her project"; pass=$((pass+1)); } || { echo "FAIL: alice's stream lacks her project"; fail=$((fail+1)); }
grep -q '"id":"p-bob"' "$D/sse-projects.log" && { echo "FAIL: alice's stream carries bob's project"; fail=$((fail+1)); } || { echo "PASS: alice's stream drops bob's project"; pass=$((pass+1)); }

echo "=== 8. deleting bob takes his rows, his files, his project and his session ==="
R=$(curl -s -H "$H" -X DELETE $S/users/$BOBID)
check "bob deleted, his two rows and his project removed" "bob 2 1" "$(echo "$R" | python3 -c "import json,sys;d=json.load(sys.stdin);print(d['deleted'], d['rows_removed'], d['projects_removed'], sep=' ')")"
check "bob's project is gone from the list" 3 "$(curl -s -H "$H" $S/projects | python3 -c "import json,sys;print(len(json.load(sys.stdin)['projects']))")"
check "bob is signed out" false "$(curl -s -b "$JAR_B" $S/auth/session | jb "['signed_in']")"
check "alice still sees her two rows" 2 "$(curl -s -b "$JAR_A" $S/gallery | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
check "bob's rows are gone for the operator" 2 "$(curl -s -b "$JAR_O" $S/gallery | python3 -c "import json,sys;print(len(json.load(sys.stdin)['items']))")"
N=$(find "$D/gallery/bob" -type f 2>/dev/null | wc -l)
check "bob's files are gone from disk" 0 "$N"

echo "=== 9. the gate off: named logins stop, named sessions end ==="
curl -s -X POST -H "$H" -H 'Content-Type: application/json' $S/users/enable -d '{"on": false}' > /dev/null
check "multi_user off" false "$(curl -s -H "$H" $S/remote | jb "['multi_user']")"
check "alice's session ended" false "$(curl -s -b "$JAR_A" $S/auth/session | jb "['signed_in']")"
check "the operator's session survived" true "$(curl -s -b "$JAR_O" $S/auth/session | jb "['signed_in']")"
check "named login refused with the gate off" "403" "$(jc -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d "{\"password\":\"$ALICEPW\",\"username\":\"alice\"}")"
R=$(curl -s -c /dev/null -X POST -H "$CS" -H 'Content-Type: application/json' $S/auth/login -d "{\"password\":\"$OPPW\"}")
check "the shared password still signs in" true "$(echo "$R" | jb "['signed_in']")"

echo
echo "RESULT: $pass passed, $fail failed"
exit $fail
