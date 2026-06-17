#!/usr/bin/env bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TARO="${TARO:-$PROJECT_DIR/target/debug/taro}"
TIMEOUT="${TARO_TIMEOUT:-5}"

if [[ ! -x "$TARO" ]]; then
    echo "Building taro..."
    (cd "$PROJECT_DIR" && cargo build)
fi

PASS=0
FAIL=0
FAILED_SCRIPTS=()

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

echo ""
echo -e "${BOLD}Running Taro script tests${RESET} (timeout: ${TIMEOUT}s each)"
echo "=============================="
echo ""

for script in "$SCRIPT_DIR"/scripts/*.taro; do
    name="$(basename "$script")"
    err_output=$(mktemp)
    if timeout "$TIMEOUT" "$TARO" "$script" >/dev/null 2>"$err_output"; then
        echo -e "  ${GREEN}PASS${RESET}  $name"
        ((PASS++))
    else
        exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            echo -e "  ${YELLOW}TIMEOUT${RESET} $name (exceeded ${TIMEOUT}s)"
        else
            echo -e "  ${RED}FAIL${RESET}  $name"
            # Show stderr content
            if [[ -s "$err_output" ]]; then
                sed 's/^/         | /' "$err_output"
            fi
        fi
        ((FAIL++))
        FAILED_SCRIPTS+=("$name")
    fi
    rm -f "$err_output"
done

echo ""
echo "=============================="
echo -e "${BOLD}Results:${RESET}  ${GREEN}$PASS passed${RESET},  ${RED}$FAIL failed${RESET}"

if [[ $FAIL -gt 0 ]]; then
    echo ""
    echo -e "${RED}Failed scripts:${RESET}"
    for f in "${FAILED_SCRIPTS[@]}"; do
        echo "  - $f"
    done
    exit 1
fi
