#!/bin/sh
# Full gate: types, unit tests, formatting, spec coverage, e2e against tsx, e2e against the bun binary.
# Fail fast: stops at the first failing stage. Prints a per stage timing summary.
cd "$(dirname "$0")/.." || exit 1
SUMMARY=""
now() { date +%s; }
stage() {
	name=$1
	shift
	printf '\n=== %s: %s ===\n' "$name" "$*"
	t0=$(now)
	"$@"
	rc=$?
	dt=$(($(now) - t0))
	if [ $rc -ne 0 ]; then
		SUMMARY="$SUMMARY
FAIL $name ${dt}s (exit $rc)"
		printf '\n=== check summary ===%s\n' "$SUMMARY"
		echo "check FAILED at stage: $name"
		exit $rc
	fi
	SUMMARY="$SUMMARY
PASS $name ${dt}s"
}
T=$(now)
stage typecheck npx tsc --noEmit -p tsconfig.json
stage unit npm test --silent
stage format npx prettier --check .
stage coverage npx tsx e2e/run.ts --coverage
stage e2e-tsx npx tsx e2e/run.ts
stage build-bin bun scripts/build-bin.ts
stage e2e-bin env XPLAIN_BIN=dist/bin/xplain npx tsx e2e/run.ts
printf '\n=== check summary ===%s\n' "$SUMMARY"
echo "check OK in $(($(now) - T))s"
