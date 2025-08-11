//! Enhanced gRPC API audit.
//! Outputs:
//!  1. RPCs declared in proto.
//!  2. Missing in trait / missing in main impl.
//!  3. Trait methods not in proto.
//!  4. Unreferenced RPC symbol usage across workspace (approx).
use std::{fs, collections::{HashSet, BTreeSet}};
fn main(){
    // --- Parse proto ---
    let proto_txt = fs::read_to_string("nyx-daemon/proto/control.proto").expect("read proto");
    let proto_rpcs: HashSet<String> = proto_txt.lines()
        .map(|l| l.trim()).filter(|l| l.starts_with("rpc "))
        .filter_map(|l| l.split_whitespace().nth(1))
        .map(|s| s.trim().trim_end_matches('(').to_string()).collect();
    // --- Parse trait ---
    let trait_src = fs::read_to_string("nyx-daemon/src/proto.rs").unwrap_or_default();
    let trait_methods: HashSet<String> = trait_src.lines().map(|l| l.trim())
        .filter(|l| l.starts_with("async fn "))
        .filter_map(|l| l.split_whitespace().nth(2))
        .filter_map(|n| n.split('(').next())
        .map(|s| s.to_string()).collect();
    // --- Parse main impl ---
    let main_src = fs::read_to_string("nyx-daemon/src/main.rs").unwrap_or_default();
    let impl_methods: HashSet<String> = main_src.lines().map(|l| l.trim())
        .filter(|l| l.starts_with("async fn "))
        .filter_map(|l| l.split_whitespace().nth(2))
        .filter_map(|n| n.split('(').next())
        .map(|s| s.to_string()).collect();
    let missing_in_trait: BTreeSet<_> = proto_rpcs.difference(&trait_methods).cloned().collect();
    let missing_in_impl: BTreeSet<_> = proto_rpcs.difference(&impl_methods).cloned().collect();
    let trait_not_proto: BTreeSet<_> = trait_methods.difference(&proto_rpcs).cloned().collect();
    // --- Reference scan ---
    let mut unused_refs = BTreeSet::new();
    for rpc in &proto_rpcs {
        let pattern = format!("{}(", rpc);
        let mut found = false;
        for entry in walkdir::WalkDir::new(".") { let entry=entry.unwrap(); if entry.file_type().is_file() { if let Ok(c)=fs::read_to_string(entry.path()){ if c.contains(&pattern){found=true; break;} } } }
        if !found { unused_refs.insert(rpc.clone()); }
    }
    println!("PROTO RPCS ({}): {:?}", proto_rpcs.len(), proto_rpcs);
    println!("Missing in trait: {:?}", missing_in_trait);
    println!("Missing in main impl: {:?}", missing_in_impl);
    println!("Trait methods not in proto: {:?}", trait_not_proto);
    println!("No code references found (scan): {:?}", unused_refs);
}
