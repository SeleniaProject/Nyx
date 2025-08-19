#!/usr/bin/env python3
"""
超安全版aggressive_fix.py v2 - より精密な危険検出
"""

import re
import sys

def safe_aggressive_fix(file_path):
    """安全なアグレッシブ修正 v2"""
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # より精密な危険ゾーンの検出 - 実際に壊れてる構造のみ
    dangerous_patterns = [
        r'pub struct \w+\s*{[^}]*Ok\(\(\)',  # 構造体内にOk(())が混入
        r'impl\s+\w+[^{]*{[^}]*Ok\(\(\)[^}]*}[^}]*Ok\(\(\)',  # impl内にOk(())が重複
    ]
    
    for pattern in dangerous_patterns:
        if re.search(pattern, content, re.DOTALL):
            print(f"⚠️  DANGEROUS: File contains broken structures, skipping: {file_path}")
            return 0
    
    # 超安全パターン - 単体行でのみ動作
    safe_patterns = [
        # 完全な.expect()行の置き換え
        (r'^(\s*)(.+)\.expect\([^)]+\);(\s*)$', r'\1\2?;\3'),
        (r'^(\s*)(.+)\.unwrap\(\);(\s*)$', r'\1\2?;\3'),
        
        # 戻り値のexpect/unwrap
        (r'^(\s*)(.+)\.expect\([^)]+\)(\s*)$', r'\1\2?\3'),
        (r'^(\s*)(.+)\.unwrap\(\)(\s*)$', r'\1\2?\3'),
        
        # テスト用のpanic!
        (r'^(\s*)panic!\("([^"]+)"\);(\s*)$', r'\1return Err("\2".into());\3'),
        
        # expect_err
        (r'^(\s*)(.+)\.expect_err\([^)]+\);(\s*)$', r'\1\2.unwrap_err();\3'),
    ]
    
    fixed = content
    total_fixes = 0
    
    for pattern, replacement in safe_patterns:
        old_content = fixed
        fixed = re.sub(pattern, replacement, fixed, flags=re.MULTILINE)
        if fixed != old_content:
            matches = len(re.findall(pattern, old_content, re.MULTILINE))
            total_fixes += matches
            print(f"�️  Safe fix: {matches} patterns - {pattern[:40]}...")
    
    # 内容が変更された場合のみファイルを書き込み
    if fixed != content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(fixed)
        print(f"🚀 SAFELY FIXED {total_fixes} patterns in {file_path}")
        return total_fixes
    else:
        print(f"✅ No safe fixes needed for {file_path}")
        return 0

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python safe_aggressive_fix.py <file_path>")
        sys.exit(1)
    
    file_path = sys.argv[1]
    fixes = safe_aggressive_fix(file_path)
    print(f"Total fixes applied: {fixes}")
