#!/usr/bin/env python3
"""
Fix mutex unwrap patterns in path_validation.rs
"""

import re
import os

def fix_path_validation_patterns(file_path):
    """Fix path validation specific patterns."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Fix mutex lock patterns
    replacements = [
        # Active challenges mutex
        (r'self\.active_challenges\.lock\(\)\.unwrap\(\)', 
         'safe_mutex_lock(&self.active_challenges, "active_challenges_operation")?'),
        
        # Path metrics mutex  
        (r'self\.path_metrics\.lock\(\)\.unwrap\(\)', 
         'safe_mutex_lock(&self.path_metrics, "path_metrics_operation")?'),
        
        # Other unwrap patterns in tests (if any)
        (r'\.parse\(\)\.unwrap\(\)', '.parse().map_err(|e| Error::Internal(format!("Parse error: {}", e)))?'),
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
    file_path = 'nyx-transport/src/path_validation.rs'
    
    if os.path.exists(file_path):
        if fix_path_validation_patterns(file_path):
            print(f"Fixed path validation patterns in: {file_path}")
        else:
            print(f"No patterns to fix in: {file_path}")
    else:
        print(f"File not found: {file_path}")

if __name__ == "__main__":
    main()
