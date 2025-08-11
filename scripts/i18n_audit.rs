//! i18n audit: detect missing keys across en/ja/zh .ftl files
use std::collections::HashSet;use std::fs;
fn parse_keys(path:&str)->HashSet<String>{let Ok(txt)=fs::read_to_string(path) else {return HashSet::new()};txt.lines().filter_map(|l|{let l=l.trim();if l.starts_with('#')||l.is_empty(){return None;}if let Some((k,_))=l.split_once('='){Some(k.trim().to_string())}else{None}}).collect()}
fn main(){
	let langs=["en","ja","zh"];let mut all:HashSet<String>=HashSet::new();let mut per=std::collections::BTreeMap::new();
	for l in &langs{let p=format!("nyx-cli/i18n/{}.ftl",l);let ks=parse_keys(&p);all.extend(ks.iter().cloned());per.insert(*l,ks);}println!("Total keys union: {}",all.len());
	for k in &all{let mut missing=Vec::new();for l in &langs{if !per[l].contains(k){missing.push(*l);} }if !missing.is_empty(){println!("MISSING key='{}' in {:?}",k,missing);} }
	// Error code expected keys audit
	let expected_error_keys = [
		"error-invalid-target","error-daemon-connection","error-network-error","error-timeout","error-permission-denied","error-invalid-stream-id","error-stream-closed","error-protocol-error",
		// CLOSE / capability mapping
		"error-unsupported-cap","error-resource-exhausted","error-failed-precondition"
	];
	for ek in expected_error_keys { for l in &langs { if !per[l].contains(ek) { println!("MISSING translation: key='{}' lang='{}'", ek, l); } } }
}
