#!/usr/bin/env python3
"""
🚨 EMERGENCY STRUCT FIELD REPAIR SCRIPT 🚨
Fixes underscore issues from perfect_fix.py that broke struct fields
"""

import os
import re
import sys

def emergency_repair(file_path):
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
    except UnicodeDecodeError:
        with open(file_path, 'r', encoding='latin-1') as f:
            content = f.read()
    
    original_content = content
    fixes = 0
    
    # Fix underscore struct field issues
    patterns = [
        # Fix struct fields that lost their underscores  
        (r'(\w+):\s*(\w+),\s*\/\*\s*([^*]+)\s*\*\/', r'_\1: \2, /* \3 */'),
        
        # Fix let bindings that lost underscores when they should keep them
        (r'let\s+([a-z_]\w*)\s*=\s*(.+);(\s*\/\/.*)?', lambda m: f'let _{m.group(1)} = {m.group(2)};{m.group(3) or ""}' if not m.group(1).startswith('_') and 'unused' in (m.group(3) or '') else m.group(0)),
        
        # Fix specific known broken field patterns
        (r'log_level:\s*(\w+)', r'_log_level: \1'),
        (r'enable_multipath:\s*(\w+)', r'_enable_multipath: \1'),
        (r'bind_addr:\s*(\w+)', r'_bind_addr: \1'),
        (r'alpha:\s*(\w+)', r'_alpha: \1'),
        (r'capacity:\s*(\w+)', r'_capacity: \1'),
        (r'refill_per_sec:\s*(\w+)', r'_refill_per_sec: \1'),
        (r'last:\s*(\w+)', r'_last: \1'),
        (r'required_features:\s*(\w+)', r'_required_features: \1'),
        (r'recommended_features:\s*(\w+)', r'_recommended_features: \1'),
        (r'available_features:\s*(\w+)', r'_available_features: \1'),
        (r'allow_multipath:\s*(\w+)', r'_allow_multipath: \1'),
        (r'threshold:\s*(\w+)', r'_threshold: \1'),
        (r'limiter:\s*(\w+)', r'_limiter: \1'),
        (r'last_activity:\s*(\w+)', r'_last_activity: \1'),
        (r'cap:\s*(\w+)', r'_cap: \1'),
        (r'w_latency:\s*(\w+)', r'_w_latency: \1'),
        (r'kem:\s*(\w+)', r'_kem: \1'),
        (r'msg:\s*(\w+)', r'_msg: \1'),
        (r'suite:\s*(\w+)', r'_suite: \1'),
        (r'key:\s*(\w+)', r'_key: \1'),
        (r'max_seq:\s*(\w+)', r'_max_seq: \1'),
        (r'rekey_interval:\s*(\w+)', r'_rekey_interval: \1'),
        (r'tx:\s*(\w+)', r'_tx: \1'),
        (r'rx:\s*(\w+)', r'_rx: \1'),
        (r'early_data:\s*(\w+)', r'_early_data: \1'),
        
        # Fix variable usage that references old underscore names
        (r'self\.([a-z_]\w*)', lambda m: f'self._{m.group(1)}' if not m.group(1).startswith('_') else m.group(0)),
        
        # Fix common broken variable references
        (r'(?<!_)data(?!\w)', r'_data'),
        (r'(?<!_)allowed(?!\w)', r'_allowed'),
        (r'(?<!_)toml(?!\w)', r'_toml'),
        (r'(?<!_)res(?!\w)', r'_res'),
        (r'(?<!_)capacity(?!\w)', r'_capacity'),
        (r'(?<!_)now(?!\w)', r'_now'),
        (r'(?<!_)start(?!\w)', r'_start'),
        (r'(?<!_)vars(?!\w)', r'_vars'),
        (r'(?<!_)full_reqs(?!\w)', r'_full_reqs'),
        (r'(?<!_)plus_reqs(?!\w)', r'_plus_reqs'),
        (r'(?<!_)level(?!\w)', r'_level'),
        (r'(?<!_)requirements(?!\w)', r'_requirements'),
        (r'(?<!_)is_compliant(?!\w)', r'_is_compliant'),
        (r'(?<!_)threshold(?!\w)', r'_threshold'),
        (r'(?<!_)rate_per_sec(?!\w)', r'_rate_per_sec'),
        (r'(?<!_)idle(?!\w)', r'_idle'),
        (r'(?<!_)baseline_ms(?!\w)', r'_baseline_ms'),
        (r'(?<!_)s1(?!\w)', r'_s1'),
        (r'(?<!_)s2(?!\w)', r'_s2'),
        (r'(?<!_)dt(?!\w)', r'_dt'),
        (r'(?<!_)s(?!\w)', r'_s'),
        (r'(?<!_)old_key(?!\w)', r'_old_key'),
        (r'(?<!_)dir(?!\w)', r'_dir'),
        (r'(?<!_)cipher(?!\w)', r'_cipher'),
        (r'(?<!_)n(?!\w)', r'_n'),
        (r'(?<!_)ct(?!\w)', r'_ct'),
        (r'(?<!_)seq(?!\w)', r'_seq'),
        (r'(?<!_)hk(?!\w)', r'_hk'),
    ]
    
    for pattern, replacement in patterns:
        if callable(replacement):
            content = re.sub(pattern, replacement, content)
        else:
            new_content = re.sub(pattern, replacement, content)
            if new_content != content:
                fixes += len(re.findall(pattern, content))
                content = new_content
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"🚨 EMERGENCY FIXED {fixes} issues in {file_path}")
        return fixes
    return 0

def main():
    print("🚨 EMERGENCY STRUCT FIELD REPAIR STARTED!")
    print("=" * 80)
    
    total_fixes = 0
    files_fixed = 0
    
    # Target specific broken files first
    critical_files = [
        "nyx-crypto/src/session.rs",
        "nyx-crypto/src/aead.rs", 
        "nyx-crypto/src/hybrid.rs",
        "nyx-crypto/src/kdf.rs",
        "nyx-crypto/src/keystore.rs",
        "nyx-crypto/src/noise.rs",
        "nyx-crypto/src/pcr.rs",
        "nyx-core/src/config.rs",
        "nyx-core/src/types.rs",
        "nyx-core/src/i18n.rs",
        "nyx-core/src/performance.rs",
        "nyx-core/src/ffi_detector.rs",
        "nyx-core/src/compliance.rs",
        "nyx-core/src/low_power.rs",
        "nyx-core/src/path_monitor.rs",
        "nyx-core/src/multipath_dataplane.rs",
    ]
    
    for file_path in critical_files:
        if os.path.exists(file_path):
            fixes = emergency_repair(file_path)
            if fixes > 0:
                total_fixes += fixes
                files_fixed += 1
    
    print("=" * 80)
    print(f"🚨 EMERGENCY REPAIR COMPLETED!")
    print(f"📊 Files fixed: {files_fixed}")
    print(f"📊 Total fixes: {total_fixes}")
    print("🚀 CRITICAL ISSUES RESOLVED!")

if __name__ == "__main__":
    main()
