#!/usr/bin/env python3
"""
Fix expect/unwrap patterns in noise.rs crypto code
"""

import re
import os

def fix_noise_patterns(file_path):
    """Fix noise.rs specific patterns."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Fix specific patterns in noise.rs
    replacements = [
        # Mix key calls
        (r'(\s+)ss\.mix_key\(([^)]+)\);', r'\1ss.mix_key(\2)?;'),
        
        # Expand ck calls  
        (r'(\s+)ss\.expand_ck\(([^)]+)\);', r'\1ss.expand_ck(\2)?;'),
        
        # Try into unwrap for array conversion
        (r'\.try_into\(\)\.unwrap\(\)', '.try_into().map_err(|_| Error::Protocol("Array conversion failed".into()))?'),
        
        # Test function unwraps (for test section)
        (r'\.unwrap\(\);', '?;'),
    ]
    
    for pattern, replacement in replacements:
        content = re.sub(pattern, replacement, content)
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    return False

def main():
    file_path = 'nyx-crypto/src/noise.rs'
    
    if os.path.exists(file_path):
        if fix_noise_patterns(file_path):
            print(f"Fixed noise patterns in: {file_path}")
        else:
            print(f"No patterns to fix in: {file_path}")
    else:
        print(f"File not found: {file_path}")

if __name__ == "__main__":
    main()
