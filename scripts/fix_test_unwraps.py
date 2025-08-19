#!/usr/bin/env python3
"""
Automated script to fix unwrap() and expect() patterns in test files.
Converts test functions to use proper error handling with TestResult<()>.
"""

import re
import sys
import os

def fix_test_unwraps(file_path):
    """Fix unwrap() and expect() patterns in test files."""
    if not os.path.exists(file_path):
        print(f"File not found: {file_path}")
        return
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Add TestResult type if not already present
    if "TestResult<T>" not in content:
        # Find the imports section and add TestResult
        imports_pattern = r'(use [^;]+;)\n+(///|#\[)'
        if re.search(imports_pattern, content):
            content = re.sub(
                imports_pattern, 
                r'\1\n\n/// Test result type for better error handling\ntype TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;\n\n\2',
                content,
                count=1
            )
    
    # Convert test functions to return TestResult<()>
    test_fn_pattern = r'#\[tokio::test\]\s*#\[traced_test\]\s*async fn ([^(]+)\([^)]*\)\s*{'
    
    def convert_test_fn(match):
        fn_name = match.group(1)
        return f"#[tokio::test]\n#[traced_test]\nasync fn {fn_name}() -> TestResult<()> {{"
    
    content = re.sub(test_fn_pattern, convert_test_fn, content)
    
    # Replace common unwrap patterns
    replacements = [
        # .unwrap() -> ?
        (r'\.parse\(\)\.unwrap\(\)', '.parse()?'),
        (r'\.local_addr\(\)\.unwrap\(\)', '.local_addr()?'),
        (r'\.connect\([^)]+\)\.await\.unwrap\(\)', '.connect().await?'),
        (r'\.accept\(\)\.await\.unwrap\(\)', '.accept().await?'),
        
        # .expect() -> proper error handling
        (r'\.expect\("([^"]+)"\)', '?'),
        (r'\.await\s*\.expect\("([^"]+)"\)', '.await?'),
        
        # Handle tokio::spawn results
        (r'\.await\.unwrap\(\)\?', '.await??'),
        
        # Add Ok(()) returns to test functions that need them
        (r'(\s+info!\([^)]+\);\s*)\n}', r'\1\n    Ok(())\n}'),
        (r'(\s+assert[^;]+;\s*)\n}', r'\1\n    Ok(())\n}'),
        (r'(\s+debug!\([^)]+\);\s*)\n}', r'\1\n    Ok(())\n}'),
    ]
    
    for pattern, replacement in replacements:
        content = re.sub(pattern, replacement, content)
    
    # Fix specific patterns for error handling
    content = re.sub(
        r'let ([^=]+) = ([^;]+)\.unwrap\(\);',
        r'let \1 = \2?;',
        content
    )
    
    # Fix .expect patterns with better error context
    content = re.sub(
        r'\.expect\("([^"]+)"\)',
        r'.map_err(|e| format!("\1: {}", e))?',
        content
    )
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"Fixed unwrap/expect patterns in: {file_path}")
        return True
    else:
        print(f"No changes needed in: {file_path}")
        return False

def main():
    if len(sys.argv) != 2:
        print("Usage: python fix_test_unwraps.py <file_path>")
        sys.exit(1)
    
    file_path = sys.argv[1]
    
    # Convert forward slashes to backslashes on Windows
    if os.name == 'nt':
        file_path = file_path.replace('/', '\\')
    
    print(f"Processing test file: {file_path}")
    
    if fix_test_unwraps(file_path):
        print("Test unwrap/expect patterns fixed successfully!")
    else:
        print("No patterns found to fix.")

if __name__ == "__main__":
    main()
