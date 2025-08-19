#!/usr/bin/env python3
"""
Batch fix unwrap/expect/panic patterns in test code
"""

import re
import sys
from pathlib import Path

def fix_test_patterns(file_path):
    """Fix unwrap/expect/panic patterns in test code"""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Test-specific patterns
    patterns = [
        # .expect_err("msg") -> .unwrap_err()
        (r'\.expect_err\([^)]+\)', '.unwrap_err()'),
        
        # .expect("msg") -> .unwrap()
        (r'\.expect\([^)]+\)', '.unwrap()'),
        
        # panic!("msg") in match arms -> unreachable!("msg") 
        (r'panic!\("([^"]+)"\)', r'unreachable!("\1")'),
        
        # panic!("{var:?}") -> unreachable!("{:?}", var)
        (r'panic!\("\{([^}:]+):?\?\}"\)', r'unreachable!("{:?}", \1)'),
        
        # .unwrap_or(()) -> .unwrap_or_default()
        (r'\.unwrap_or\(\(\)\)', '.unwrap_or_default()'),
    ]
    
    changes = 0
    for pattern, replacement in patterns:
        new_content = re.sub(pattern, replacement, content)
        if new_content != content:
            changes += len(re.findall(pattern, content))
            content = new_content
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"✅ Fixed {changes} patterns in {file_path}")
        return changes
    
    return 0

if __name__ == "__main__":
    file_path = sys.argv[1] if len(sys.argv) > 1 else "nyx-stream/src/plugin_dispatch.rs"
    fix_test_patterns(file_path)
