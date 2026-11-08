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

$F build tests/globals.flen -o "$TMP/globals" >/dev/null
out=$("$TMP/globals")
[ "$out" = "2 0 XY 10 20" ] || { echo "globals FAIL: $out"; exit 1; }
echo "ok globals"

$F build tests/types.flen -o "$TMP/types" >/dev/null
out=$("$TMP/types")
[ "$out" = $'4000000000 -5 8 1\n9223372036854775807 4294967295 65' ] || { echo "types FAIL: $out"; exit 1; }
echo "ok types"

$F build tests/multidecl.flen -o "$TMP/multidecl" >/dev/null
out=$("$TMP/multidecl")
[ "$out" = "1 2 35 7" ] || { echo "multidecl FAIL: $out"; exit 1; }
echo "ok multidecl"

$F build tests/loop.flen -o "$TMP/loop" >/dev/null
out=$("$TMP/loop")
[ "$out" = "18 4 5" ] || { echo "loop FAIL: $out"; exit 1; }
echo "ok loop"

$F build tests/multidim.flen -o "$TMP/multidim" >/dev/null
out=$("$TMP/multidim")
[ "$out" = $'0 2 10\n11 12 24' ] || { echo "multidim FAIL: $out"; exit 1; }
echo "ok multidim"

$F build tests/arrayparam.flen -o "$TMP/arrayparam" >/dev/null
out=$("$TMP/arrayparam")
[ "$out" = "6" ] || { echo "arrayparam FAIL: $out"; exit 1; }
echo "ok arrayparam"

$F build tests/proto.flen -o "$TMP/proto" >/dev/null
out=$("$TMP/proto")
[ "$out" = "5" ] || { echo "proto FAIL: $out"; exit 1; }
echo "ok proto"

$F build tests/suffix.flen -o "$TMP/suffix" >/dev/null
out=$("$TMP/suffix")
[ "$out" = "255 10 255" ] || { echo "suffix FAIL: $out"; exit 1; }
echo "ok suffix"

for neg in neg_break neg_continue neg_arrsize neg_globaltype neg_classcopy \
    neg_return_type neg_arg_count neg_undef_func neg_dup_func neg_private_ctor \
    neg_this_assign neg_ptr_dot neg_dup_case; do
    if $F build "tests/$neg.flen" -o "$TMP/$neg" >/dev/null 2>&1; then
        echo "$neg FAIL: beklenmedik şekilde derlendi"; exit 1
    fi
    echo "ok $neg (reddedildi)"
done

$F build examples/shell.flen -o "$TMP/shell" >/dev/null
out=$(printf 'echo test\npwd\nexit\n' | "$TMP/shell")
echo "$out" | grep -q "test" || { echo "shell FAIL"; exit 1; }
err=$(printf 'nosuchcmd_xyz\nexit\n' | "$TMP/shell" 2>&1 >/dev/null)
echo "$err" | grep -q "No such file" || { echo "shell exec safety FAIL"; exit 1; }
echo "ok shell"

$F build tests/switch.flen -o "$TMP/switch" >/dev/null
out=$("$TMP/switch")
[ "$out" = "iki" ] || { echo "switch FAIL: $out"; exit 1; }
echo "ok switch"

$F build tests/switch_default.flen -o "$TMP/switch_default" >/dev/null
out=$("$TMP/switch_default")
[ "$out" = "uc" ] || { echo "switch_default FAIL: $out"; exit 1; }
echo "ok switch_default"
echo "TÜM TESTLER GEÇTİ"
