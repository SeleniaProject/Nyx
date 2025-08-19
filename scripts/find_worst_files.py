#!/usr/bin/env python3
"""
Find worst files for refactoring priority
"""

import os
import re
from pathlib import Path
from collections import defaultdict

def count_patterns(file_path):
    """Count error-prone patterns in a file"""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
    except:
        return 0, 0, 0
    
    # Count unwrap/expect/panic patterns
    error_patterns = len(re.findall(r'\.(unwrap|expect)\s*\(|panic!\s*\(', content))
    
    # Count unsafe blocks
    unsafe_blocks = len(re.findall(r'\bunsafe\s*\{', content))
    
    # Estimate complexity by nesting depth
    max_depth = 0
    current_depth = 0
    for line in content.split('\n'):
        stripped = line.strip()
        if stripped and not stripped.startswith('//'):
            current_depth += stripped.count('{')
            max_depth = max(max_depth, current_depth)
            current_depth -= stripped.count('}')
            current_depth = max(0, current_depth)
    
    return error_patterns, unsafe_blocks, max_depth

def find_worst_files():
    """Find files with highest refactoring priority"""
    results = []
    
    for file_path in Path('.').rglob('*.rs'):
        if 'target' in str(file_path):
            continue
            
        error_patterns, unsafe_blocks, max_depth = count_patterns(file_path)
        
        # Calculate severity score
        severity = error_patterns * 3 + unsafe_blocks * 2 + max(0, max_depth - 5)
        
        if severity > 30:  # Only show high-severity files
            results.append({
                'file': str(file_path),
                'errors': error_patterns,
                'unsafe': unsafe_blocks,
                'depth': max_depth,
                'severity': severity
            })
    
    # Sort by severity
    results.sort(key=lambda x: x['severity'], reverse=True)
    
    print("🔥 TOP 15 WORST FILES FOR REFACTORING:")
    print("=" * 80)
    
    for i, result in enumerate(results[:15], 1):
        print(f"{i:2d}. {result['file']:<50} | "
              f"Errors: {result['errors']:2d} | "
              f"Unsafe: {result['unsafe']:2d} | "
              f"Depth: {result['depth']:2d} | "
              f"Score: {result['severity']:3d}")

if __name__ == "__main__":
    find_worst_files()
