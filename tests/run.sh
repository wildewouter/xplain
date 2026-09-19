#!/bin/sh
# Run all tests concurrently; exit nonzero on any FAIL or crash.
# Builds a fixture (files stored as *.fx so tsc ignores them) git repo (HEAD = fixture/base, working tree = fixture/work) in a temp dir.
cd "$(dirname "$0")/.." || exit 1
T=$(mktemp -d "${CLAUDE_JOB_DIR:-${TMPDIR:-/tmp}}/xplain-test-XXXXXX") || exit 1
trap 'rm -rf "$T"' EXIT
R="$T/repo"
mkdir -p "$R"
cp -R tests/fixture/base/. "$R/"
for x in $(cd "$R" && find . -name '*.fx'); do mv "$R/$x" "$R/${x%.fx}"; done
git -C "$R" init -q
git -C "$R" add -A
git -C "$R" -c user.email=a@b.c -c user.name=x commit -qm init
cp -R tests/fixture/work/. "$R/"
for x in $(cd "$R" && find . -name '*.fx'); do mv "$R/$x" "$R/${x%.fx}"; done
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
