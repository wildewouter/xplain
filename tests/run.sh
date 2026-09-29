#!/bin/sh
# Run all tests concurrently; exit nonzero on any FAIL or crash.
# Builds a fixture (files stored as *.fx so tsc ignores them) git repo (HEAD = fixture/base, working tree = fixture/work) in a temp dir.
cd "$(dirname "$0")/.." || exit 1
T=$(mktemp -d "${TMPDIR:-/tmp}/xplain-test-XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
R="$T/repo"
sh e2e/fixture.sh "$R" standard || exit 1
# Run files concurrently (each has its own out file; tests use mkdtemp/port 0, keys uses keys-* dirs only it owns).
for f in tests/*.test.ts tests/*.test.tsx; do
	b=$(basename "$f")
	(
		npx tsx "$f" "$R" >"$T/$b.out" 2>&1
		echo $? >"$T/$b.code"
	) &
done
wait
rc=0
for f in tests/*.test.ts tests/*.test.tsx; do
	b=$(basename "$f")
	out="$T/$b.out"
	code=$(cat "$T/$b.code" 2>/dev/null || echo 1)
	pass=$(grep -c '^PASS' "$out")
	fail=$(grep -c '^FAIL' "$out")
	echo "$f: $pass PASS, $fail FAIL (exit $code)"
	if [ "$fail" -ne 0 ] || [ "$code" -ne 0 ]; then
		grep '^FAIL' "$out"
		[ "$code" -ne 0 ] && tail -n 15 "$out"
		rc=1
	fi
done
exit $rc
