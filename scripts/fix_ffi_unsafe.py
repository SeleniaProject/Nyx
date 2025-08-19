#!/usr/bin/env python3
"""
Replace unsafe FFI calls with safe wrapper functions in mobile FFI tests
"""

import re
import os

def fix_ffi_unsafe_calls(file_path):
    """Replace unsafe FFI calls with safe wrapper functions."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # Replace unsafe FFI calls with safe wrapper calls
    replacements = [
        # Basic FFI calls
        (r'unsafe\s*{\s*nyx_mobile_init\(\)\s*}', 'safe_ffi::mobile_init()'),
        (r'unsafe\s*{\s*nyx_mobile_shutdown\(\)\s*}', 'safe_ffi::mobile_shutdown()'),
        (r'unsafe\s*{\s*nyx_push_wake\(\)\s*}', 'safe_ffi::push_wake()'),
        (r'unsafe\s*{\s*nyx_resume_low_power_session\(\)\s*}', 'safe_ffi::resume_low_power_session()'),
        
        # Power state calls
        (r'unsafe\s*{\s*nyx_power_set_state\(([^)]+)\)\s*}', r'safe_ffi::power_set_state(\1)'),
        (r'unsafe\s*{\s*nyx_power_get_state\(&mut\s+([^)]+)\)\s*}', r'safe_ffi::power_get_state(&mut \1)'),
        
        # Telemetry calls - basic case
        (r'unsafe\s*{\s*nyx_mobile_set_telemetry_label\(([^,]+)\.as_ptr\(\),\s*([^)]+)\.as_ptr\(\)\)\s*}', 
         r'safe_ffi::set_telemetry_label(&\1, &\2)'),
        
        # Telemetry calls - null value case
        (r'unsafe\s*{\s*nyx_mobile_set_telemetry_label\(([^,]+)\.as_ptr\(\),\s*std::ptr::null\(\)\)\s*}', 
         r'safe_ffi::set_telemetry_label_null_value(&\1)'),
    ]
    
    for pattern, replacement in replacements:
        content = re.sub(pattern, replacement, content)
    
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    return False

def main():
    file_path = 'nyx-mobile-ffi/tests/power_policy_e2e.rs'
    
    if os.path.exists(file_path):
        if fix_ffi_unsafe_calls(file_path):
            print(f"Fixed unsafe FFI calls in: {file_path}")
        else:
            print(f"No unsafe calls to fix in: {file_path}")
    else:
        print(f"File not found: {file_path}")

if __name__ == "__main__":
    main()
