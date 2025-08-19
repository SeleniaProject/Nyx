#!/usr/bin/env python3
"""
Fix double let _ = patterns created by previous script
"""

import re
import os
import glob

def fix_double_let(file_path):
    """Fix double let _ = patterns."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Fix double let _ = patterns
    content = re.sub(r'let _ = let _ = ', 'let _ = ', content)
    
    # Fix expressions that shouldn't have let _ = 
    content = re.sub(r'let result = let _ = ', 'let result = ', content)
    content = re.sub(r'let ([a-zA-Z_][a-zA-Z0-9_]*) = let _ = ', r'let \1 = ', content)
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    return False

def main():
    # Find all test files in nyx-transport/tests/ and src/
    test_files = glob.glob('nyx-transport/tests/*.rs') + glob.glob('nyx-transport/src/*.rs')
    
    fixed_count = 0
    for file_path in test_files:
        if fix_double_let(file_path):
            print(f"Fixed double let patterns in: {file_path}")
            fixed_count += 1
    
    print(f"Fixed {fixed_count} files")

if __name__ == "__main__":
    main()
