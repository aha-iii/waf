//! Tests extracted from `lpm/lpm.rs`.

use crate::lpm::*;
use std::net::Ipv4Addr;

    #[test]
    fn parse_cidr() {
        let (p, a) = parse_v4_prefix("10.0.0.0/8").unwrap();
        assert_eq!(p, 8);
        assert_eq!(a, Ipv4Addr::new(10, 0, 0, 0));
    }
