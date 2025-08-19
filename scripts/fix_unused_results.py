#!/usr/bin/env python3
"""
Fix remaining unused Result warnings in source files
"""

import re
import os

def fix_unused_result_warnings(file_path):
    """Fix unused Result warnings by adding let _ = prefix."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Specific patterns that need fixing
    replacements = [
        (r'(\s+)traversal\.cleanup_sessions\(\);', r'\1let _ = traversal.cleanup_sessions();'),
        (r'(\s+)server\.cleanup_relay_sessions\(\);', r'\1let _ = server.cleanup_relay_sessions();'),
        (r'(\s+)server\.wait_terminated\([^)]+\)\.await;', r'\1let _ = server.wait_terminated'),
    ]
    
    for pattern, replacement in replacements:
        content = re.sub(pattern, replacement, content)
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    return False

def main():
    files_to_fix = [
        'nyx-transport/src/stun_server.rs',
        'nyx-transport/tests/stun_server.rs', 
        'nyx-transport/tests/stun_stop_idempotent.rs',
        'nyx-transport/tests/enhanced_stun_stop_idempotent.rs'
    ]
    
    fixed_count = 0
    for file_path in files_to_fix:
        if os.path.exists(file_path) and fix_unused_result_warnings(file_path):
            print(f"Fixed unused Result warnings in: {file_path}")
            fixed_count += 1
    
    print(f"Fixed {fixed_count} files")

if __name__ == "__main__":
    main()
