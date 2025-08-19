#!/usr/bin/env python3
"""
session.rsの重複戻り値型を一括修正
"""

import re

def fix_duplicate_return_types():
    file_path = r"C:\Users\Aqua\Programming\SeleniaProject\NyxNet\nyx-crypto\src\session.rs"
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # 重複した戻り値型を修正
    patterns = [
        # -> Result<()> -> Result<()> を -> Result<()> に修正
        (r'-> Result<\(\)>\s*-> Result<\(\)>', r'-> Result<()>'),
        
        # その他の重複パターン
        (r'-> Result<\(\)>\s*-> Result<\(\)>', r'-> Result<()>'),
    ]
    
    fixed = content
    total_fixes = 0
    
    for pattern, replacement in patterns:
        matches = re.findall(pattern, fixed)
        if matches:
            fixed = re.sub(pattern, replacement, fixed)
            total_fixes += len(matches)
            print(f"🔧 Fixed {len(matches)} duplicate return types")
    
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(fixed)
    
    print(f"🚀 Total fixes: {total_fixes}")

if __name__ == "__main__":
    fix_duplicate_return_types()
