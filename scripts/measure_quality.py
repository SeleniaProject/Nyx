#!/usr/bin/env python3
"""
Code Quality Measurement Script for Nyx Project

This script measures various code quality metrics to track improvements
during the refactoring process. It focuses on the patterns identified
in the refactoring prompt.

Metrics Measured:
- Error-prone patterns (expect, unwrap, panic!)
- Unsafe blocks
- Function complexity (nesting depth)
- Code duplication patterns
- Line count statistics

Author: Generated for Nyx Protocol refactoring process
"""

import os
import re
import subprocess
import sys
from pathlib import Path
from collections import defaultdict
from typing import Dict, List, Tuple

class QualityMetrics:
    def __init__(self):
        self.error_prone_patterns = 0
        self.unsafe_blocks = 0
        self.max_nesting_depth = 0
        self.duplicate_patterns = 0
        self.total_lines = 0
        self.files_processed = 0
        self.pattern_details = defaultdict(list)

def find_rust_files(root_path: Path) -> List[Path]:
    """Find all Rust source files in the project."""
    rust_files = []
    for file_path in root_path.rglob("*.rs"):
        # Skip target directory and test files if needed
        if "target" not in file_path.parts:
            rust_files.append(file_path)
    return rust_files

def count_nesting_depth(content: str) -> int:
    """Calculate maximum nesting depth in the file."""
    max_depth = 0
    current_depth = 0
    
    for line in content.split('\n'):
        stripped = line.strip()
        if not stripped or stripped.startswith('//'):
            continue
            
        # Count opening braces
        open_braces = stripped.count('{')
        close_braces = stripped.count('}')
        
        current_depth += open_braces
        max_depth = max(max_depth, current_depth)
        current_depth -= close_braces
        current_depth = max(0, current_depth)  # Prevent negative
    
    return max_depth

def find_error_prone_patterns(content: str, file_path: Path) -> Tuple[int, List[str]]:
    """Find error-prone patterns like expect(), unwrap(), panic!()."""
    patterns = [
        r'\.expect\s*\(',
        r'\.unwrap\s*\(',
        r'panic!\s*\(',
        r'unreachable!\s*\(',
        r'unimplemented!\s*\(',
        r'todo!\s*\('
    ]
    
    matches = []
    total_count = 0
    
    lines = content.split('\n')
    for i, line in enumerate(lines, 1):
        # Skip comments and documentation
        stripped = line.strip()
        if stripped.startswith('//') or stripped.startswith('///') or stripped.startswith('/*'):
            continue
            
        for pattern in patterns:
            if re.search(pattern, line):
                matches.append(f"{file_path}:{i}: {stripped}")
                total_count += 1
    
    return total_count, matches

def find_unsafe_blocks(content: str, file_path: Path) -> Tuple[int, List[str]]:
    """Find unsafe blocks in the code."""
    # Look for actual unsafe blocks, not just the word "unsafe" in comments
    unsafe_pattern = r'\bunsafe\s*\{'
    
    matches = []
    count = 0
    
    lines = content.split('\n')
    for i, line in enumerate(lines, 1):
        # Skip comments
        stripped = line.strip()
        if stripped.startswith('//') or stripped.startswith('///'):
            continue
            
        if re.search(unsafe_pattern, line):
            matches.append(f"{file_path}:{i}: {stripped}")
            count += 1
    
    return count, matches

def find_duplicate_patterns(content: str, file_path: Path) -> Tuple[int, List[str]]:
    """Find potential duplicate code patterns."""
    # Look for repeated patterns that might indicate code duplication
    duplicate_indicators = [
        r'if\s+!is_authorized\s*\(',  # Authorization checks
        r'Response::err_with_id\s*\(',  # Error responses
        r'Response::ok_with_id\s*\(',   # Success responses
        r'tracing::(error|warn|info|debug)!\s*\(',  # Logging patterns
        r'#\[cfg\(feature\s*=\s*"telemetry"\)\]',  # Telemetry guards
        r'telemetry::record_counter\s*\(',  # Telemetry calls
    ]
    
    matches = []
    total_count = 0
    
    lines = content.split('\n')
    for i, line in enumerate(lines, 1):
        stripped = line.strip()
        if stripped.startswith('//'):
            continue
            
        for pattern in duplicate_indicators:
            if re.search(pattern, line):
                matches.append(f"{file_path}:{i}: Pattern - {pattern}")
                total_count += 1
    
    return total_count, matches

def analyze_file(file_path: Path) -> QualityMetrics:
    """Analyze a single Rust file for quality metrics."""
    try:
        content = file_path.read_text(encoding='utf-8')
    except (UnicodeDecodeError, PermissionError) as e:
        print(f"Warning: Could not read {file_path}: {e}")
        return QualityMetrics()
    
    metrics = QualityMetrics()
    metrics.files_processed = 1
    metrics.total_lines = len(content.split('\n'))
    
    # Measure nesting depth
    metrics.max_nesting_depth = count_nesting_depth(content)
    
    # Find error-prone patterns
    error_count, error_details = find_error_prone_patterns(content, file_path)
    metrics.error_prone_patterns = error_count
    metrics.pattern_details['error_prone'].extend(error_details)
    
    # Find unsafe blocks
    unsafe_count, unsafe_details = find_unsafe_blocks(content, file_path)
    metrics.unsafe_blocks = unsafe_count
    metrics.pattern_details['unsafe'].extend(unsafe_details)
    
    # Find duplicate patterns
    dup_count, dup_details = find_duplicate_patterns(content, file_path)
    metrics.duplicate_patterns = dup_count
    metrics.pattern_details['duplicates'].extend(dup_details)
    
    return metrics

def merge_metrics(metrics_list: List[QualityMetrics]) -> QualityMetrics:
    """Merge multiple metrics objects into one."""
    combined = QualityMetrics()
    
    for metrics in metrics_list:
        combined.error_prone_patterns += metrics.error_prone_patterns
        combined.unsafe_blocks += metrics.unsafe_blocks
        combined.max_nesting_depth = max(combined.max_nesting_depth, metrics.max_nesting_depth)
        combined.duplicate_patterns += metrics.duplicate_patterns
        combined.total_lines += metrics.total_lines
        combined.files_processed += metrics.files_processed
        
        # Merge pattern details
        for category, details in metrics.pattern_details.items():
            combined.pattern_details[category].extend(details)
    
    return combined

def print_metrics_report(metrics: QualityMetrics, verbose: bool = False):
    """Print a comprehensive metrics report."""
    print("=" * 60)
    print("Nyx Project Code Quality Metrics Report")
    print("=" * 60)
    print(f"Files Processed: {metrics.files_processed}")
    print(f"Total Lines of Code: {metrics.total_lines:,}")
    print()
    
    print("QUALITY METRICS:")
    print(f"  Error-prone patterns (expect/unwrap/panic): {metrics.error_prone_patterns}")
    print(f"  Unsafe blocks: {metrics.unsafe_blocks}")
    print(f"  Maximum nesting depth: {metrics.max_nesting_depth}")
    print(f"  Duplicate code patterns: {metrics.duplicate_patterns}")
    print()
    
    # Calculate quality score (lower is better)
    quality_score = (
        metrics.error_prone_patterns * 2 +  # High penalty for error-prone patterns
        metrics.unsafe_blocks * 1 +         # Medium penalty for unsafe
        max(0, metrics.max_nesting_depth - 5) * 3 +  # Penalty for excessive nesting
        metrics.duplicate_patterns * 1      # Penalty for duplication
    )
    
    print(f"QUALITY SCORE: {quality_score} (lower is better)")
    print()
    
    if verbose:
        print("DETAILED FINDINGS:")
        print("-" * 40)
        
        if metrics.pattern_details['error_prone']:
            print(f"\nError-prone patterns ({len(metrics.pattern_details['error_prone'])}):")
            for detail in metrics.pattern_details['error_prone'][:10]:  # Show first 10
                print(f"  {detail}")
            if len(metrics.pattern_details['error_prone']) > 10:
                print(f"  ... and {len(metrics.pattern_details['error_prone']) - 10} more")
        
        if metrics.pattern_details['unsafe']:
            print(f"\nUnsafe blocks ({len(metrics.pattern_details['unsafe'])}):")
            for detail in metrics.pattern_details['unsafe']:
                print(f"  {detail}")
        
        if metrics.pattern_details['duplicates']:
            print(f"\nDuplicate patterns ({len(metrics.pattern_details['duplicates'])}):")
            for detail in metrics.pattern_details['duplicates'][:10]:  # Show first 10
                print(f"  {detail}")
            if len(metrics.pattern_details['duplicates']) > 10:
                print(f"  ... and {len(metrics.pattern_details['duplicates']) - 10} more")

def main():
    """Main function to run the quality measurement."""
    # Determine if we should show verbose output
    verbose = '--verbose' in sys.argv or '-v' in sys.argv
    
    # Find project root (look for Cargo.toml)
    current_dir = Path.cwd()
    project_root = current_dir
    
    while project_root != project_root.parent:
        if (project_root / "Cargo.toml").exists():
            break
        project_root = project_root.parent
    else:
        print("Error: Could not find Cargo.toml (project root)")
        sys.exit(1)
    
    print(f"Analyzing Rust project at: {project_root}")
    
    # Find all Rust files
    rust_files = find_rust_files(project_root)
    
    if not rust_files:
        print("No Rust files found!")
        sys.exit(1)
    
    print(f"Found {len(rust_files)} Rust source files")
    
    # Analyze each file
    all_metrics = []
    for file_path in rust_files:
        if verbose:
            print(f"Analyzing: {file_path.relative_to(project_root)}")
        
        file_metrics = analyze_file(file_path)
        all_metrics.append(file_metrics)
    
    # Merge all metrics
    combined_metrics = merge_metrics(all_metrics)
    
    # Print the report
    print_metrics_report(combined_metrics, verbose)

if __name__ == "__main__":
    main()
