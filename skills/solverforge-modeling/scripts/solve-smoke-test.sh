#!/usr/bin/env sh
# Solve smoke test for a generated SolverForge app.
#
# For web/API, boots the app, starts a real solve job, and requires clean
# completion. For CLI, validates only the generated demo-data command because
# that shell has no generated solve command.
#
# Usage: solve-smoke-test.sh <app-dir> [port] [demo-size]
#   app-dir    directory containing solverforge.app.toml (default: .)
#   port       web/api bind port (default: a process-derived high port)
#   demo-size  demo data id to solve (default: the catalog default)

set -eu

app_dir=${1:-.}
port=${2:-$((20000 + ($$ % 20000)))}
demo_size=${3:-}
solve_timeout=${SF_SMOKE_TIMEOUT_SECONDS:-120}

[ -d "$app_dir" ] || { echo "error: no such directory: $app_dir" >&2; exit 2; }
[ -f "$app_dir/solverforge.app.toml" ] || {
	echo "error: $app_dir/solverforge.app.toml not found; not a generated app" >&2
	exit 2
}
app_dir=$(CDPATH= cd -- "$app_dir" && pwd)

command -v python3 >/dev/null 2>&1 || {
	echo "error: python3 is required for JSON parsing" >&2
	exit 2
}

shell=$(sed -n 's/^[[:space:]]*shell[[:space:]]*=[[:space:]]*"\(.*\)".*/\1/p' "$app_dir/solverforge.app.toml" | head -n1)
[ -n "$shell" ] || shell=web

pkg=$(sed -n 's/^name[[:space:]]*=[[:space:]]*"\(.*\)".*/\1/p' "$app_dir/Cargo.toml" | head -n1)
[ -n "$pkg" ] || { echo "error: could not read package name from Cargo.toml" >&2; exit 2; }
bin=$(printf '%s' "$pkg" | tr '-' '_')

workdir=$(mktemp -d)
log="$workdir/server.log"
plan="$workdir/plan.json"
server_pid=""

cleanup() {
	if [ -n "$server_pid" ]; then
		kill "$server_pid" 2>/dev/null || true
		wait "$server_pid" 2>/dev/null || true
	fi
	rm -rf "$workdir"
}
trap cleanup EXIT HUP INT TERM

echo "==> building $pkg ($shell shell)"
if ! (cd "$app_dir" && cargo build --quiet) >"$log" 2>&1; then
	echo "FAIL: cargo build failed"
	cat "$log"
	exit 1
fi

target_dir=$(cd "$app_dir" && cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
bin_path="$target_dir/debug/$bin"
[ -x "$bin_path" ] || { echo "FAIL: built binary not found at $bin_path"; exit 1; }

if [ "$shell" = "cli" ]; then
	echo "==> cli shell: running demo-data"
	if ! out=$(cd "$app_dir" && "$bin_path" demo-data 2>>"$log"); then
		echo "FAIL: demo-data exited non-zero"
		cat "$log"
		exit 1
	fi
	printf '%s' "$out" | python3 -c 'import json,sys; text=sys.stdin.read(); start=text.find("{"); assert start >= 0; json.loads(text[start:])' 2>/dev/null || {
		echo "FAIL: demo-data did not contain a valid JSON document"
		printf '%s\n' "$out"
		exit 1
	}
	if grep -qi panic "$log"; then
		echo "FAIL: panic in output"
		grep -i panic "$log"
		exit 1
	fi
	echo "PASS: cli demo-data emitted valid JSON with no panic (serialization only; the generated CLI has no solve command)"
	exit 0
fi

command -v curl >/dev/null 2>&1 || {
	echo "error: curl is required for web/API smoke tests" >&2
	exit 2
}

if curl -sf "http://127.0.0.1:$port/health" >/dev/null 2>&1; then
	echo "FAIL: port $port is already serving a SolverForge health endpoint"
	exit 1
fi

echo "==> booting server on port $port"
(cd "$app_dir" && PORT="$port" "$bin_path") >>"$log" 2>&1 &
server_pid=$!

up=0
i=0
while [ "$i" -lt 300 ]; do
	if curl -sf "http://127.0.0.1:$port/health" >/dev/null 2>&1; then
		up=1
		break
	fi
	if ! kill -0 "$server_pid" 2>/dev/null; then
		echo "FAIL: server exited during startup"
		cat "$log"
		exit 1
	fi
	i=$((i + 1))
	sleep 1
done
[ "$up" = 1 ] || { echo "FAIL: server did not become healthy in 300s"; cat "$log"; exit 1; }
kill -0 "$server_pid" 2>/dev/null || { echo "FAIL: launched server exited after health check"; cat "$log"; exit 1; }

if [ -z "$demo_size" ]; then
	demo_size=$(curl -sf "http://127.0.0.1:$port/demo-data" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("defaultId","STANDARD"))' 2>/dev/null || echo STANDARD)
fi
echo "==> solving demo data $demo_size"

curl -sf "http://127.0.0.1:$port/demo-data/$demo_size" > "$plan"
job=$(curl -sf -X POST -H 'content-type: application/json' --data @"$plan" "http://127.0.0.1:$port/jobs" | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
[ -n "$job" ] || { echo "FAIL: no job id returned"; exit 1; }
echo "==> job $job started; polling status"

state=""
status_json=""
i=0
while [ "$i" -lt "$solve_timeout" ]; do
	status_json=$(curl -sf "http://127.0.0.1:$port/jobs/$job/status" || true)
	if [ -n "$status_json" ]; then
		state=$(printf '%s' "$status_json" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("lifecycleState",""))' 2>/dev/null || true)
		case "$state" in
		COMPLETED | FAILED | CANCELLED) break ;;
		esac
	fi
	i=$((i + 1))
	sleep 1
done

if grep -qi panic "$log"; then
	echo "FAIL: constraint panic during solve"
	grep -i panic "$log"
	exit 1
fi

if [ -z "$status_json" ]; then
	echo "FAIL: no status returned for job $job"
	cat "$log"
	exit 1
fi

printf '%s' "$status_json" | python3 -c '
import json,sys
d=json.load(sys.stdin)
print("lifecycleState:", d.get("lifecycleState"))
print("currentScore:  ", d.get("currentScore"))
print("bestScore:     ", d.get("bestScore"))
t=d.get("telemetry") or {}
print("stepCount:     ", t.get("stepCount"))
'

if ! printf '%s' "$status_json" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d.get("currentScore") is not None and d.get("bestScore") is not None' 2>/dev/null; then
	echo "FAIL: job did not publish current and best scores"
	exit 1
fi

case "$state" in
COMPLETED)
	echo "PASS: job $job reached $state with no panic"
	exit 0
	;;
FAILED | CANCELLED)
	echo "FAIL: job ended $state"
	exit 1
	;;
SOLVING | PAUSED | PAUSE_REQUESTED)
	echo "FAIL: job did not complete within $solve_timeout seconds (last state: $state)"
	exit 1
	;;
*)
	echo "FAIL: job reported unexpected lifecycle state '$state'"
	exit 1
	;;
esac
