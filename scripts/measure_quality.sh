#!/bin/bash
# Code quality metrics collector for Nyx project
# Measures various code quality indicators before and after refactoring

echo "=== Code Quality Metrics for Nyx Project ===" 

WORKSPACE_ROOT="/mnt/c/Users/Aqua/Programming/SeleniaProject/NyxNet"

# Function to count lines of code, excluding tests and generated files
count_loc() {
    find "$WORKSPACE_ROOT" -name "*.rs" \
        -not -path "*/target/*" \
        -not -path "*/tests/*" \
        -not -path "*test*" \
        -not -name "*test*.rs" \
        -not -name "build.rs" \
        | xargs wc -l | tail -1 | awk '{print $1}'
}

# Function to measure cyclomatic complexity using tokei
measure_complexity() {
    if command -v tokei &> /dev/null; then
        tokei "$WORKSPACE_ROOT" --type rust --output json 2>/dev/null | \
        jq -r '.["Rust"].stats.complexity // "N/A"' 2>/dev/null || echo "N/A"
    else
        echo "N/A (tokei not available)"
    fi
}

# Function to count expect/unwrap/panic occurrences
count_error_prone() {
    find "$WORKSPACE_ROOT" -name "*.rs" \
        -not -path "*/target/*" \
        -not -path "*/tests/*" \
        -not -path "*test*" \
        -not -name "*test*.rs" \
        | xargs grep -n "expect\|unwrap\|panic!" | wc -l
}

# Function to count TODO/FIXME comments
count_todos() {
    find "$WORKSPACE_ROOT" -name "*.rs" \
        -not -path "*/target/*" \
        | xargs grep -i "todo\|fixme\|xxx\|hack" | wc -l
}

# Function to count unsafe blocks
count_unsafe() {
    find "$WORKSPACE_ROOT" -name "*.rs" \
        -not -path "*/target/*" \
        | xargs grep -n "unsafe" | grep -v "forbid(unsafe_code)" | wc -l
}

# Function to measure maximum nesting depth
measure_max_nesting() {
    max_depth=0
    while IFS= read -r -d '' file; do
        depth=$(awk '
        BEGIN { max=0; current=0 }
        /{/ { current++; if(current > max) max = current }
        /}/ { current-- }
        END { print max }
        ' "$file")
        if [ "$depth" -gt "$max_depth" ]; then
            max_depth=$depth
        fi
    done < <(find "$WORKSPACE_ROOT" -name "*.rs" -not -path "*/target/*" -not -path "*/tests/*" -print0)
    echo $max_depth
}

# Function to count duplicate code blocks (simple heuristic)
count_duplicates() {
    find "$WORKSPACE_ROOT" -name "*.rs" \
        -not -path "*/target/*" \
        -not -path "*/tests/*" \
        | xargs grep -h "^[[:space:]]*[^/].*{$" \
        | sort | uniq -c | awk '$1 > 1 {sum++} END {print sum+0}'
}

# Function to count crates
count_crates() {
    find "$WORKSPACE_ROOT" -name "Cargo.toml" \
        -not -path "*/target/*" | wc -l
}

echo "Collecting metrics..."

LOC=$(count_loc)
COMPLEXITY=$(measure_complexity)
ERROR_PRONE=$(count_error_prone)
TODOS=$(count_todos)
UNSAFE=$(count_unsafe)
MAX_NESTING=$(measure_max_nesting)
DUPLICATES=$(count_duplicates)
CRATES=$(count_crates)

echo ""
echo "| Metric | Value |"
echo "|--------|-------|" 
echo "| Lines of Code (non-test) | $LOC |"
echo "| Cyclomatic Complexity | $COMPLEXITY |"
echo "| Error-prone patterns (expect/unwrap/panic) | $ERROR_PRONE |"
echo "| TODO/FIXME comments | $TODOS |"
echo "| Unsafe blocks | $UNSAFE |"
echo "| Maximum nesting depth | $MAX_NESTING |"
echo "| Potential duplicate patterns | $DUPLICATES |"
echo "| Number of crates | $CRATES |"
echo ""

# Export for comparison
cat > "$WORKSPACE_ROOT/quality_metrics_before.json" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "lines_of_code": $LOC,
  "cyclomatic_complexity": "$COMPLEXITY",
  "error_prone_patterns": $ERROR_PRONE,
  "todos": $TODOS,
  "unsafe_blocks": $UNSAFE,
  "max_nesting_depth": $MAX_NESTING,
  "duplicates": $DUPLICATES,
  "crates": $CRATES
}
EOF

echo "Metrics saved to quality_metrics_before.json"
