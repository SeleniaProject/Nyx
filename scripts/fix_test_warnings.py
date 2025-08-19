#!/usr/bin/env python3
"""
Fix unused Result warnings in test files by adding let _ = 
"""

import re
import os
import glob

def fix_unused_results(file_path):
    """Fix unused Result warnings by adding let _ = prefix."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Fix specific patterns that generate warnings
    patterns = [
        # server.stop() and similar
        (r'(\s+)server\.stop\(\);', r'\1let _ = server.stop();'),
        (r'(\s+)server\.wait_terminated\([^)]+\)\.await;', r'\1let _ = server.wait_terminated'),
    ]
    
    for pattern, replacement in patterns:
        content = re.sub(pattern, replacement, content)
    
    # More general pattern for method calls that return Result
    content = re.sub(
        r'(\s+)([a-zA-Z_][a-zA-Z0-9_]*\.[a-zA-Z_][a-zA-Z0-9_]*\([^)]*\)\.await);',
        lambda m: f"{m.group(1)}let _ = {m.group(2)};" if "assert" not in m.group(2) and "println" not in m.group(2) and "=" not in m.group(0) else m.group(0),
        content
    )
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    return False

def main():
    # Find all test files in nyx-transport/tests/
    test_files = glob.glob('nyx-transport/tests/*.rs')
    
    fixed_count = 0
    for file_path in test_files:
        if fix_unused_results(file_path):
            print(f"Fixed unused Result warnings in: {file_path}")
            fixed_count += 1
    
    print(f"Fixed {fixed_count} test files")

if __name__ == "__main__":
    main()
