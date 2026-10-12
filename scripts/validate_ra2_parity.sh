#!/bin/sh
# Runs the validation commands of docs/prompts/RA2_PARITY_PROMPT.md section 9, one after another,
# and keeps their logs under docs/validation/ra2-parity/<label>/.
#
# Run from anywhere: `sh scripts/validate_ra2_parity.sh <label> [first] [last]` (label: the
# milestone, for example `ra3`; first and last: the numbers of the first and last command to run,
# default 1 and 11). Every command runs even when an earlier one failed, so that one run shows
# every failure. `summary.txt` lists each command with its exit status and, for the test run, the
# number of tests passed, failed and ignored. Exit code 0 only if every command run exited 0.
set -u

label="${1:?usage: validate_ra2_parity.sh <label> [first] [last]}"
first="${2:-1}"
last="${3:-11}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root" || exit 2
out="$root/docs/validation/ra2-parity/$label"
mkdir -p "$out"
summary="$out/summary.txt"
[ "$first" -eq 1 ] && : > "$summary"
status=0

# The smoke tests and the package script open a window. A display that sleeps or is removed
# while one runs closes the window and fails the run ("Monitor removed", RA0), so the display is
# kept awake for as long as this script runs, and woken before each of those commands.
if command -v caffeinate > /dev/null 2>&1; then
    caffeinate -d -i -w $$ &
fi

run() {
    number="$1"
    name="$2"
    shift 2
    if [ "$number" -lt "$first" ] || [ "$number" -gt "$last" ]; then
        return
    fi
    log="$out/$(printf '%02d' "$number")-$name.log"
    case "$name" in
        smoke-*|package) command -v caffeinate > /dev/null 2>&1 && caffeinate -u -t 5 ;;
    esac
    start="$(date +%s)"
    "$@" > "$log" 2>&1
    code=$?
    end="$(date +%s)"
    line="$(printf '%02d' "$number") exit $code  $((end - start)) s  $*"
    if [ "$name" = "test" ]; then
        counts="$(awk '/^test result:/ { p += $4; f += $6; i += $8 } END { printf "%d passed, %d failed, %d ignored", p, f, i }' "$log")"
        line="$line  [$counts]"
    fi
    echo "$line" | tee -a "$summary"
    [ "$code" -eq 0 ] || status=1
}

run 1 fmt cargo fmt --all -- --check
run 2 diff-check git diff --check
run 3 clippy cargo clippy --workspace --all-targets -- -D warnings
run 4 test cargo test --workspace --release
run 5 build-release cargo build --release -p game
run 6 smoke-release cargo run --release -p game -- --smoke-test 600
run 7 smoke-debug cargo run -p game -- --smoke-test 600
run 8 benchmark cargo run --release -p sim --example m7_benchmark
run 9 soak cargo run --release -p sim --example m7_soak
run 10 duels cargo run --release -p sim --example m14_duels -- full
run 11 package sh scripts/package_macos.sh

echo "validation $label: $([ "$status" -eq 0 ] && echo PASSED || echo FAILED)" | tee -a "$summary"
exit "$status"
