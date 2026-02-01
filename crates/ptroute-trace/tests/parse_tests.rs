use ptroute_trace::parse_traceroute_n;

#[test]
fn parse_linux_numeric_with_edge_cases() {
    let text = include_str!("fixtures/traceroute_linux_1.txt");
    let run = parse_traceroute_n(text).unwrap();

    assert_eq!(run.target, "203.0.113.1");
    assert_eq!(run.hops.len(), 5);

    let hop1 = &run.hops[0];
    assert_eq!(hop1.ttl, 1);
    assert_eq!(hop1.ip.as_deref(), Some("192.168.1.1"));
    assert_eq!(hop1.rtt_ms.len(), 3);
    assert!((hop1.rtt_ms[0].unwrap() - 1.123).abs() < 1e-6);

    let hop2 = &run.hops[1];
    assert_eq!(hop2.ttl, 2);
    assert_eq!(hop2.ip.as_deref(), Some("10.0.0.1"));
    assert_eq!(hop2.rtt_ms.len(), 3);
    assert!(hop2.rtt_ms[1].is_none());

    let hop3 = &run.hops[2];
    assert_eq!(hop3.ttl, 3);
    assert_eq!(hop3.ip.as_deref(), Some("10.0.0.2"));
    assert_eq!(hop3.rtt_ms.len(), 3);

    let hop4 = &run.hops[3];
    assert_eq!(hop4.ttl, 4);
    assert!(hop4.ip.is_none());
    assert_eq!(hop4.rtt_ms, vec![None, None, None]);
}

#[test]
fn parse_macos_numeric() {
    let text = include_str!("fixtures/traceroute_macos_1.txt");
    let run = parse_traceroute_n(text).unwrap();

    assert_eq!(run.target, "1.1.1.1");
    assert_eq!(run.hops.len(), 4);
    assert_eq!(run.hops[2].rtt_ms, vec![None, None, None]);
    assert_eq!(run.hops[3].ip.as_deref(), Some("1.1.1.1"));
}

#[test]
fn parse_ipv6_numeric() {
    let text = include_str!("fixtures/traceroute_ipv6_1.txt");
    let run = parse_traceroute_n(text).unwrap();

    assert_eq!(run.target, "2606:4700:4700::1111");
    assert_eq!(run.hops.len(), 4);
    assert_eq!(run.hops[0].ip.as_deref(), Some("fe80::1"));
    assert_eq!(run.hops[3].ip.as_deref(), Some("2606:4700:4700::1111"));
}

#[test]
fn parse_multi_ip_per_hop() {
    let text = include_str!("fixtures/traceroute_multi_ip_1.txt");
    let run = parse_traceroute_n(text).unwrap();

    assert_eq!(run.target, "198.51.100.10");
    assert_eq!(run.hops.len(), 2);
    assert_eq!(run.hops[0].ip.as_deref(), Some("10.0.0.1"));
    assert_eq!(run.hops[0].rtt_ms.len(), 3);
    assert_eq!(run.hops[1].ip.as_deref(), Some("198.51.100.10"));
}
