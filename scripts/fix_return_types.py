#!/usr/bin/env python3
"""
Fix Function Return Types for Nyx Transport

This script fixes function return types to handle the ? operator properly.
"""

import re
from pathlib import Path

def fix_function_return_types(file_path: Path):
    """Fix function return types to use StunResult where appropriate."""
    
    content = file_path.read_text(encoding='utf-8')
    original_content = content
    
    # Define specific function fixes based on the compiler errors
    fixes = [
        # void functions that need Result return
        {
            'pattern': r'(pub fn cleanup_sessions\(&self\)) \{',
            'replacement': r'\1 -> StunResult<()> {',
            'add_ok': True
        },
        {
            'pattern': r'(pub fn stop\(&self\)) \{',
            'replacement': r'\1 -> StunResult<()> {',
            'add_ok': True
        },
        {
            'pattern': r'(pub fn cleanup_clients\(&self, max_age: Duration\)) \{',
            'replacement': r'\1 -> StunResult<()> {',
            'add_ok': True
        },
        {
            'pattern': r'(pub fn cleanup_relay_sessions\(&self\)) \{',
            'replacement': r'\1 -> StunResult<()> {',
            'add_ok': True
        },
        {
            'pattern': r'(pub async fn wait_terminated\(&self, max_wait: Duration\)) \{',
            'replacement': r'\1 -> StunResult<()> {',
            'add_ok': True
        },
        # Functions that return specific types but need Result wrapping
        {
            'pattern': r'(fn generate_session_id\(&self\) -> u64) \{',
            'replacement': r'\1 -> StunResult<u64> {',
            'add_ok': False  # Return value needs wrapping
        },
        {
            'pattern': r'(pub fn get_relay_statistics\(&self\) -> RelayStatistics) \{',
            'replacement': r'\1 -> StunResult<RelayStatistics> {',
            'add_ok': False  # Return value needs wrapping
        },
        # Option-returning functions that need special handling
        {
            'pattern': r'(pub fn get_session_status\(&self, session_id: u64\) -> Option<ConnectivityState>) \{',
            'replacement': r'\1 -> Option<ConnectivityState> {',
            'add_ok': False,  # Special handling needed
            'use_ok': True
        },
        {
            'pattern': r'(pub fn get_session_state\(&self, session_id: u64\) -> Option<HolePunchState>) \{',
            'replacement': r'\1 -> Option<HolePunchState> {',
            'add_ok': False,
            'use_ok': True
        }
    ]
    
    for fix in fixes:
        if re.search(fix['pattern'], content):
            content = re.sub(fix['pattern'], fix['replacement'], content)
            print(f"Fixed function signature: {fix['pattern']}")
    
    # Add Ok(()) before closing braces for functions that need it
    # This is a simple heuristic - may need refinement
    lines = content.split('\n')
    in_target_function = False
    target_functions = ['cleanup_sessions', 'stop', 'cleanup_clients', 'cleanup_relay_sessions']
    function_name = ""
    brace_depth = 0
    
    for i, line in enumerate(lines):
        # Check if we're entering a target function
        for func in target_functions:
            if f'pub fn {func}(' in line and 'StunResult<()>' in line:
                in_target_function = True
                function_name = func
                brace_depth = 0
                break
        
        if in_target_function:
            brace_depth += line.count('{') - line.count('}')
            
            # If we're at the end of the function and there's no Ok(()) return
            if brace_depth == 0 and line.strip() == '}':
                # Look at the previous non-empty line
                j = i - 1
                while j >= 0 and lines[j].strip() == '':
                    j -= 1
                
                if j >= 0 and 'Ok(())' not in lines[j]:
                    # Insert Ok(()) before the closing brace
                    lines.insert(i, '        Ok(())')
                    print(f"Added Ok(()) return to function: {function_name}")
                
                in_target_function = False
                function_name = ""
    
    content = '\n'.join(lines)
    
    # Special handling for functions that need .ok()?
    content = re.sub(
        r'(let sessions = safe_mutex_lock\([^)]+\))\?;',
        r'\1.ok()?;',
        content
    )
    
    # Handle async blocks in spawn that need Result returns
    # This is more complex and may need manual intervention
    
    if content != original_content:
        file_path.write_text(content, encoding='utf-8')
        print("Function return types updated")
        return True
    
    return False

def fix_async_blocks(file_path: Path):
    """Fix async blocks that use ? operator."""
    content = file_path.read_text(encoding='utf-8')
    
    # For now, just report the async blocks that need manual fixing
    # These are typically in tokio::spawn blocks
    spawn_blocks = re.findall(r'tokio::spawn\(async move \{[^}]+?\}\)', content, re.DOTALL)
    
    if spawn_blocks:
        print(f"Found {len(spawn_blocks)} async spawn blocks that may need manual fixing")
        for i, block in enumerate(spawn_blocks):
            if '?' in block:
                print(f"  Block {i+1} uses ? operator and may need Result return type")
    
    return len(spawn_blocks) > 0

def main():
    file_path = Path("nyx-transport/src/stun_server.rs")
    
    if not file_path.exists():
        print(f"File not found: {file_path}")
        return
    
    print(f"Fixing function return types in: {file_path}")
    
    # Fix function return types
    fixed = fix_function_return_types(file_path)
    
    if fixed:
        print("Function return types fixed. Checking for async blocks...")
        fix_async_blocks(file_path)
        print("Manual review may be needed for async spawn blocks.")
    
    print("Return type fixes completed!")

if __name__ == "__main__":
    main()
