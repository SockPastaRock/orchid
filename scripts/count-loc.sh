#!/usr/bin/env bash
# Count lines of code split by production and test code using cloc

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

# Check if cloc is installed
if ! command -v cloc &>/dev/null; then
    echo "Error: cloc is not installed. Install it with: brew install cloc"
    exit 1
fi

echo "Lines of Code Analysis"
echo "====================="
echo ""

# Extract test code from all src files to temporary directory
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

find src -name "*.rs" -type f | while read -r file; do
    awk '
        /^[[:space:]]*#\[cfg\(test\)\]/ { in_test = 1; next }
        /^[[:space:]]*#\[test\]/ { in_test = 1; next }
        in_test && /^[[:space:]]*}[[:space:]]*$/ { 
            print
            in_test = 0
            next
        }
        in_test { print }
    ' "$file" > "$TEMP_DIR/$(basename "$file" .rs)_tests.rs" 2>/dev/null || true
done

# Count production code (all src files)
echo "All Code (src/)"
echo "---------------"
ALL_OUTPUT=$(cloc src/ --include-lang=Rust 2>/dev/null)
ALL_FILES=$(echo "$ALL_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f2)
ALL_BLANK=$(echo "$ALL_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f3)
ALL_COMMENTS=$(echo "$ALL_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f4)
ALL_CODE=$(echo "$ALL_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f5)
echo "$ALL_FILES files, $ALL_CODE lines of code, $ALL_BLANK blank lines"

# Count test code
echo ""
echo "Test Code (only)"
echo "----------------"
TEST_CODE=0
TEST_BLANK=0
TEST_FILES=0

if [ -d "$TEMP_DIR" ] && [ -n "$(find "$TEMP_DIR" -name "*.rs" -type f 2>/dev/null)" ]; then
    TEST_OUTPUT=$(cloc "$TEMP_DIR" --include-lang=Rust 2>/dev/null)
    if echo "$TEST_OUTPUT" | grep -q "^Rust"; then
        TEST_FILES=$(echo "$TEST_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f2)
        TEST_BLANK=$(echo "$TEST_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f3)
        TEST_CODE=$(echo "$TEST_OUTPUT" | grep "^Rust" | tr -s ' ' | cut -d' ' -f5)
    fi
fi

if [ "$TEST_CODE" -gt 0 ]; then
    echo "$TEST_FILES files, $TEST_CODE lines of code, $TEST_BLANK blank lines"
else
    echo "0 files, 0 lines of code"
fi

# Calculate production-only code
echo ""
echo "Production Code (code - tests)"
echo "------------------------------"
if [ "$TEST_CODE" -gt 0 ]; then
    PROD_CODE=$((ALL_CODE - TEST_CODE))
    PROD_FILES=$((ALL_FILES - TEST_FILES))
    echo "$PROD_FILES files, $PROD_CODE lines of code"
else
    PROD_CODE=$ALL_CODE
    echo "$ALL_FILES files, $PROD_CODE lines of code"
fi

# Summary stats
echo ""
echo "Summary"
echo "-------"
FILE_COUNT=$(find src -name "*.rs" -type f | wc -l)
echo "Total Rust files: $FILE_COUNT"
if [ "$TEST_CODE" -gt 0 ]; then
    RATIO=$(echo "scale=1; ($TEST_CODE * 100) / $ALL_CODE" | bc 2>/dev/null || echo "0")
    echo "Code to Test Ratio: $RATIO% tests"
fi

# Cargo.toml info
echo ""
echo "Project Info"
echo "------------"
if [ -f Cargo.toml ]; then
    NAME=$(grep "^name" Cargo.toml | cut -d'"' -f2)
    VERSION=$(grep "^version" Cargo.toml | cut -d'"' -f2)
    echo "$NAME v$VERSION"
fi

