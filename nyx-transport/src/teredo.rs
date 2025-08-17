//! Minimal helpers around IPv6/IPv4 mapping (placeholder for Teredo handling).
use std::net::{Ipv4Addr, Ipv6Addr};

/// Create an IPv6-mapped IPv4 address (::ffff:a.b.c.d)
pub fn ipv6_mapped(ipv4: Ipv4Addr) -> Ipv6Addr {
	// v6 mapped: ::ffff:a.b.c.d -> 16 bytes with last 4 as IPv4 and preceding 2 bytes as 0xffff
	let oct = ipv4.octets();
	Ipv6Addr::from([
		0,0,0,0,0,0,0,0,0,0,0xff,0xff, oct[0], oct[1], oct[2], oct[3]
	])
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn mapped() {
		let v6 = ipv6_mapped(Ipv4Addr::new(127,0,0,1));
		assert_eq!(v6.to_string(), "::ffff:127.0.0.1");
	}
}
