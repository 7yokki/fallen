#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q
F=target/release/fallen
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

$F build tests/hello.flen -o "$TMP/hello" >/dev/null
out=$("$TMP/hello")
[ "$out" = $'Merhaba Fallen!\na+b = 42' ] || { echo "hello FAIL"; exit 1; }
echo "ok hello"

$F build tests/oop.flen -o "$TMP/oop" >/dev/null
out=$("$TMP/oop")
[ "$out" = $'p=7\np2=17\nsum=14' ] || { echo "oop FAIL: $out"; exit 1; }
echo "ok oop"

$F build tests/bounds.flen -o "$TMP/bounds" >/dev/null
set +e
out=$("$TMP/bounds" 2>&1)
code=$?
set -e
[ "$code" -eq 1 ] && echo "$out" | grep -q "sinir ihlali" || { echo "bounds FAIL"; exit 1; }
echo "ok bounds"

$F build tests/macros.flen -o "$TMP/macros" >/dev/null
out=$("$TMP/macros")
[ "$out" = "5 25 9" ] || { echo "macros FAIL: $out"; exit 1; }
echo "ok macros"

$F build tests/abi.flen -o "$TMP/abi" >/dev/null
out=$("$TMP/abi")
[ "$out" = $'neg-ok\nchar=-56' ] || { echo "abi FAIL: $out"; exit 1; }
echo "ok abi"

$F build examples/shell.flen -o "$TMP/shell" >/dev/null
out=$(printf 'echo test\npwd\nexit\n' | "$TMP/shell")
echo "$out" | grep -q "test" || { echo "shell FAIL"; exit 1; }
err=$(printf 'nosuchcmd_xyz\nexit\n' | "$TMP/shell" 2>&1 >/dev/null)
echo "$err" | grep -q "No such file" || { echo "shell exec safety FAIL"; exit 1; }
echo "ok shell"
echo "TÜM TESTLER GEÇTİ"
