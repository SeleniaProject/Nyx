#!/usr/bin/env python3
"""
Problem Files Identifier for Nyx Project

This script identifies the files with the most quality issues
to prioritize refactoring efforts.
"""

import sys
from pathlib import Path
from collections import defaultdict

# Add scripts directory to path
sys.path.append('scripts')
from measure_quality import analyze_file, find_rust_files

def find_worst_files(project_root: Path, limit: int = 20):
    """Find the files with the most quality issues."""
    rust_files = find_rust_files(project_root)
    
    file_issues = []
    
    print(f"Analyzing {len(rust_files)} files...")
    
    for i, file_path in enumerate(rust_files):
        if i % 50 == 0:
            print(f"Progress: {i}/{len(rust_files)}")
            
        try:
            metrics = analyze_file(file_path)
            
            # Calculate severity score
            severity_score = (
                metrics.error_prone_patterns * 3 +  # High priority
                metrics.unsafe_blocks * 2 +         # Medium priority  
                max(0, metrics.max_nesting_depth - 6) * 4 +  # Very high penalty for deep nesting
                metrics.duplicate_patterns * 1      # Lower priority
            )
            
            if severity_score > 0:  # Only include files with issues
                file_issues.append({
                    'file': file_path,
                    'score': severity_score,
                    'error_patterns': metrics.error_prone_patterns,
                    'unsafe_blocks': metrics.unsafe_blocks,
                    'nesting_depth': metrics.max_nesting_depth,
                    'duplicates': metrics.duplicate_patterns,
                    'lines': metrics.total_lines
                })
        except Exception as e:
            print(f"Error analyzing {file_path}: {e}")
    
    # Sort by severity score (highest first)
    file_issues.sort(key=lambda x: x['score'], reverse=True)
    
    return file_issues[:limit]

def print_priority_report(worst_files):
    """Print a prioritized report of problematic files."""
    print("\n" + "="*80)
    print("HIGH PRIORITY REFACTORING TARGETS")
    print("="*80)
    
    for i, file_info in enumerate(worst_files, 1):
        rel_path = str(file_info['file']).replace(str(Path.cwd()), '').lstrip('\\/')
        
        print(f"\n{i:2d}. {rel_path}")
        print(f"    Severity Score: {file_info['score']}")
        print(f"    Error Patterns: {file_info['error_patterns']}")
        print(f"    Unsafe Blocks:  {file_info['unsafe_blocks']}")
        print(f"    Max Nesting:    {file_info['nesting_depth']}")
        print(f"    Duplicates:     {file_info['duplicates']}")
        print(f"    Lines of Code:  {file_info['lines']}")
        
        # Priority recommendation
        if file_info['error_patterns'] > 10:
            print("    🚨 CRITICAL: Many error-prone patterns")
        elif file_info['nesting_depth'] > 10:
            print("    ⚠️  HIGH: Deep nesting complexity")
        elif file_info['unsafe_blocks'] > 5:
            print("    🔒 MEDIUM: Multiple unsafe blocks")
        else:
            print("    📋 LOW: Minor improvements needed")

def identify_specific_patterns(worst_files):
    """Identify specific patterns to focus on."""
    print("\n" + "="*80)
    print("PATTERN-SPECIFIC RECOMMENDATIONS")
    print("="*80)
    
    total_error_patterns = sum(f['error_patterns'] for f in worst_files[:10])
    total_deep_nesting = len([f for f in worst_files[:10] if f['nesting_depth'] > 8])
    
    print(f"\nTop 10 files contain {total_error_patterns} error patterns")
    print(f"Top 10 files have {total_deep_nesting} with deep nesting (>8 levels)")
    
    print("\n🎯 RECOMMENDED FOCUS AREAS:")
    print("1. Replace expect/unwrap with proper error handling")
    print("2. Break down complex functions (>8 nesting levels)")
    print("3. Create helper functions for repeated patterns")
    print("4. Add comprehensive error context and logging")

def main():
    # Find project root
    project_root = Path.cwd()
    while project_root != project_root.parent:
        if (project_root / "Cargo.toml").exists():
            break
        project_root = project_root.parent
    else:
        print("Error: Could not find Cargo.toml (project root)")
        sys.exit(1)
    
    print(f"Analyzing project at: {project_root}")
    
    # Find worst files
    worst_files = find_worst_files(project_root, limit=20)
    
    # Print reports
    print_priority_report(worst_files)
    identify_specific_patterns(worst_files)
    
    return worst_files

if __name__ == "__main__":
    main()
