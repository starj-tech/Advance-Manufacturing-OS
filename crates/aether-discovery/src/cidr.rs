//! IPv4 CIDR parsing and host enumeration.
//!
//! ## Why an in-crate parser
//! Factory LANs are universally IPv4 — a /24 with 256 hosts is the
//! canonical scan target. Pulling in `ipnet` for that one job adds
//! dependency surface and pulls in IPv6 abstractions that the
//! discovery scanner never exercises. A ~50-line parser limited to
//! IPv4 covers every realistic use case (single host through /16)
//! with zero new deps.
//!
//! ## Maximum prefix length
//! Anything wider than /16 (65 536 hosts) is rejected as
//! `CidrError::RangeTooWide`. A naïve /8 scan would queue 16 M probes
//! against a single Scanner — the resulting connect-storm would trip
//! every IDS in the building. If a user genuinely needs to scan a /16,
//! they should split it into per-/24 jobs. /16 is the soft cap; /24
//! (the factory norm) is well under it.

use std::net::Ipv4Addr;
use thiserror::Error;

/// Maximum CIDR width we accept. /16 = 65 536 hosts; wider scans are
/// rejected so a typo can't enqueue millions of probes.
pub const MAX_HOSTS: u32 = 1 << 16;

#[derive(Debug, Error)]
pub enum CidrError {
    #[error("invalid IPv4 address: {0}")]
    InvalidAddr(String),
    #[error("invalid prefix length: {0}")]
    InvalidPrefix(String),
    #[error("CIDR range too wide ({hosts} hosts > {max_hosts}); split into per-/24 jobs")]
    RangeTooWide { hosts: u64, max_hosts: u32 },
}

/// Parse `"X.Y.Z.W/N"` (or `"X.Y.Z.W"` for /32) and return the list of
/// host IPs in the range. The network and broadcast addresses are
/// included — the discovery scanner doesn't bother with classful
/// semantics and a "network address" on a /24 may legitimately host a
/// PLC that responds to MQTT.
pub fn expand_cidr(input: &str) -> Result<Vec<Ipv4Addr>, CidrError> {
    let (addr_str, prefix_str) = match input.split_once('/') {
        Some((a, p)) => (a.trim(), p.trim()),
        None => (input.trim(), "32"),
    };

    let ip: Ipv4Addr = addr_str
        .parse()
        .map_err(|_| CidrError::InvalidAddr(addr_str.to_string()))?;
    let prefix: u8 = prefix_str
        .parse()
        .map_err(|_| CidrError::InvalidPrefix(prefix_str.to_string()))?;
    if prefix > 32 {
        return Err(CidrError::InvalidPrefix(prefix.to_string()));
    }

    let host_bits = 32u32 - prefix as u32;
    // u32 left-shift by 32 is UB; gate /0 separately.
    let host_count: u64 = if host_bits >= 32 {
        1u64 << 32
    } else {
        1u64 << host_bits
    };
    if host_count > MAX_HOSTS as u64 {
        return Err(CidrError::RangeTooWide {
            hosts: host_count,
            max_hosts: MAX_HOSTS,
        });
    }

    // Mask the host bits off so a "192.168.1.5/24" treats 192.168.1.0
    // as the network — the typed address can carry stray host bits
    // that don't match the prefix.
    let mask: u32 = if prefix == 0 { 0 } else { !0u32 << host_bits };
    let network = u32::from(ip) & mask;
    let mut out = Vec::with_capacity(host_count as usize);
    for i in 0..host_count {
        out.push(Ipv4Addr::from(network | i as u32));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_host_without_prefix_defaults_to_slash_32() {
        let hosts = expand_cidr("10.0.0.5").unwrap();
        assert_eq!(hosts, vec![Ipv4Addr::new(10, 0, 0, 5)]);
    }

    #[test]
    fn single_host_slash_32() {
        let hosts = expand_cidr("192.168.1.42/32").unwrap();
        assert_eq!(hosts, vec![Ipv4Addr::new(192, 168, 1, 42)]);
    }

    #[test]
    fn slash_30_yields_four_hosts() {
        // /30 = 4 hosts: .0, .1, .2, .3
        let hosts = expand_cidr("192.168.1.0/30").unwrap();
        assert_eq!(hosts.len(), 4);
        assert_eq!(hosts[0], Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(hosts[3], Ipv4Addr::new(192, 168, 1, 3));
    }

    #[test]
    fn slash_24_yields_256_hosts_starting_at_network_address() {
        // /24 = 256 hosts, indexed by the last octet 0..=255.
        let hosts = expand_cidr("10.0.5.0/24").unwrap();
        assert_eq!(hosts.len(), 256);
        assert_eq!(hosts[0], Ipv4Addr::new(10, 0, 5, 0));
        assert_eq!(hosts[255], Ipv4Addr::new(10, 0, 5, 255));
    }

    #[test]
    fn host_bits_in_input_are_masked_off() {
        // "192.168.1.42/24" → network 192.168.1.0/24. The "42" is
        // discarded so the iteration always starts at the network
        // address.
        let hosts = expand_cidr("192.168.1.42/24").unwrap();
        assert_eq!(hosts.len(), 256);
        assert_eq!(hosts[0], Ipv4Addr::new(192, 168, 1, 0));
        assert_eq!(hosts[42], Ipv4Addr::new(192, 168, 1, 42));
    }

    #[test]
    fn slash_16_is_at_the_upper_bound_and_accepted() {
        // /16 is exactly 65 536 hosts — at the cap, must accept.
        let hosts = expand_cidr("172.16.0.0/16").unwrap();
        assert_eq!(hosts.len(), 65_536);
    }

    #[test]
    fn slash_15_exceeds_max_and_is_rejected() {
        // /15 = 131 072 hosts, above the cap.
        let err = expand_cidr("172.16.0.0/15").unwrap_err();
        assert!(matches!(err, CidrError::RangeTooWide { .. }));
    }

    #[test]
    fn slash_0_is_rejected_even_though_well_formed() {
        // Belt-and-braces against an operator typo that would
        // otherwise queue 4 billion probes.
        let err = expand_cidr("0.0.0.0/0").unwrap_err();
        assert!(matches!(err, CidrError::RangeTooWide { .. }));
    }

    #[test]
    fn invalid_addr_returns_typed_error() {
        assert!(matches!(
            expand_cidr("not.an.ip/24").unwrap_err(),
            CidrError::InvalidAddr(_)
        ));
    }

    #[test]
    fn invalid_prefix_returns_typed_error() {
        assert!(matches!(
            expand_cidr("10.0.0.0/abc").unwrap_err(),
            CidrError::InvalidPrefix(_)
        ));
        assert!(matches!(
            expand_cidr("10.0.0.0/33").unwrap_err(),
            CidrError::InvalidPrefix(_)
        ));
    }

    #[test]
    fn whitespace_around_input_is_tolerated() {
        // Pasting from a UI/console often introduces stray whitespace.
        let hosts = expand_cidr("  10.0.0.1 / 32 ").unwrap();
        assert_eq!(hosts, vec![Ipv4Addr::new(10, 0, 0, 1)]);
    }
}
