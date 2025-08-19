#!/usr/bin/env python3
"""
Automated Mutex Unwrap Replacer for Nyx Transport

This script automatically replaces .lock().unwrap() patterns with 
safe_mutex_lock() calls in the stun_server.rs file.
"""

import re
from pathlib import Path

def replace_mutex_unwraps(file_path: Path):
    """Replace mutex .lock().unwrap() patterns with safe_mutex_lock calls."""
    
    content = file_path.read_text(encoding='utf-8')
    original_content = content
    
    # Pattern to match various mutex lock unwrap patterns
    patterns = [
        # self.field.lock().unwrap()
        (r'self\.(\w+)\.lock\(\)\.unwrap\(\)', r'safe_mutex_lock(&self.\1, "\1_operation")?'),
        
        # variable.lock().unwrap()
        (r'(\w+)\.lock\(\)\.unwrap\(\)', r'safe_mutex_lock(&\1, "\1_operation")?'),
        
        # More specific patterns for common variable names
        (r'sessions\.lock\(\)\.unwrap\(\)', r'safe_mutex_lock(&sessions, "sessions_access")?'),
        (r'clients\.lock\(\)\.unwrap\(\)', r'safe_mutex_lock(&clients, "clients_access")?'),
        (r'running\.lock\(\)\.unwrap\(\)', r'safe_mutex_lock(&running, "running_state")?'),
    ]
    
    replacements = 0
    
    for pattern, replacement in patterns:
        new_content, count = re.subn(pattern, replacement, content)
        content = new_content
        replacements += count
        
        if count > 0:
            print(f"Replaced {count} instances of pattern: {pattern}")
    
    # Special handling for more complex cases
    # Handle cases like: while *running.lock().unwrap() {
    content = re.sub(
        r'while \*(\w+)\.lock\(\)\.unwrap\(\)',
        r'while *safe_mutex_lock(&\1, "\1_loop_check")?',
        content
    )
    
    if content != original_content:
        file_path.write_text(content, encoding='utf-8')
        print(f"Successfully replaced {replacements} mutex unwrap patterns")
        return True
    else:
        print("No patterns found to replace")
        return False

def add_result_types_to_functions(file_path: Path):
    """Add Result return types to functions that now use ? operator."""
    
    content = file_path.read_text(encoding='utf-8')
    original_content = content
    
    # Find functions that use ? but don't return Result
    # This is a simple heuristic - may need manual review
    
    functions_to_fix = []
    lines = content.split('\n')
    
    in_function = False
    function_signature = ""
    function_start_line = 0
    
    for i, line in enumerate(lines):
        # Look for function declarations
        if re.match(r'\s*(?:pub\s+)?(?:async\s+)?fn\s+\w+', line.strip()):
            in_function = True
            function_signature = line.strip()
            function_start_line = i
        elif in_function and line.strip() == "}":
            in_function = False
        elif in_function and "?" in line and "StunResult" not in function_signature and "Result" not in function_signature:
            # This function uses ? but doesn't return Result
            functions_to_fix.append((function_start_line, function_signature))
            in_function = False
    
    # Report functions that may need manual fixing
    if functions_to_fix:
        print(f"Functions that may need Result return type added:")
        for line_num, sig in functions_to_fix:
            print(f"  Line {line_num + 1}: {sig}")
    
    return len(functions_to_fix) > 0

def main():
    file_path = Path("nyx-transport/src/stun_server.rs")
    
    if not file_path.exists():
        print(f"File not found: {file_path}")
        return
    
    print(f"Processing: {file_path}")
    
    # Replace mutex unwraps
    replaced = replace_mutex_unwraps(file_path)
    
    if replaced:
        print("\nChecking for functions that may need Result return types...")
        needs_manual_review = add_result_types_to_functions(file_path)
        
        if needs_manual_review:
            print("\nSome functions may need manual review to add Result return types.")
        else:
            print("\nNo additional manual review needed.")
    
    print("\nAutomated mutex unwrap replacement completed!")

if __name__ == "__main__":
    main()
