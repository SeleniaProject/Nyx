#!/usr/bin/env python3
"""
session.rsのテスト関数を一括修正
"""

import re

def fix_session_tests():
    file_path = r"C:\Users\Aqua\Programming\SeleniaProject\NyxNet\nyx-crypto\src\session.rs"
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # テスト関数の戻り値型修正と末尾のOk(())削除
    test_patterns = [
        # テスト関数の最後のOk(())を削除
        (r'(\s+)Ok\(\(\)\s*}\s*$', r'\1}'),
        
        # test関数の署名を修正 (fn test_name() -> Result<()>)
        (r'#\[test\]\s*fn ([^(]+)\(\)', r'#[test]\n    fn \1() -> Result<()>'),
    ]
    
    fixed = content
    total_fixes = 0
    
    for pattern, replacement in test_patterns:
        matches = re.findall(pattern, fixed, re.MULTILINE)
        if matches:
            fixed = re.sub(pattern, replacement, fixed, flags=re.MULTILINE)
            total_fixes += len(matches)
            print(f"🔧 Fixed {len(matches)} occurrences: {pattern[:30]}...")
    
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(fixed)
    
    print(f"🚀 Total fixes: {total_fixes}")

if __name__ == "__main__":
    fix_session_tests()
