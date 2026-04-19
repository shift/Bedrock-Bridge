use bedrock_bridge_core::{self as core};

/// Integration test: verify ping packet construction and pong response round-trip.
#[tokio::test]
async fn test_discovery_ping_pong_roundtrip() {
    let motd = core::discovery::build_motd("Test LAN", 0x1234);
    let pong = core::discovery::build_pong(42i64, 0x1234i64, &motd);

    // Verify pong structure
    assert_eq!(pong[0], 0x1c);
    assert_eq!(&pong[17..33], &core::discovery::RAKNET_MAGIC);

    let motd_len = u16::from_be_bytes([pong[33], pong[34]]);
    assert_eq!(motd_len as usize, motd.len());
    assert_eq!(&pong[35..], motd.as_bytes());
    assert!(motd.contains("Test LAN"));
}

#[test]
fn test_build_ping_and_detect() {
    // Build a valid unconnected ping packet
    let mut ping = vec![0u8; 33];
    ping[0] = 0x01;
    ping[1..9].copy_from_slice(&99i64.to_be_bytes());
    ping[9..25].copy_from_slice(&core::discovery::RAKNET_MAGIC);
    ping[25..33].copy_from_slice(&0i64.to_be_bytes());

    assert!(core::discovery::is_unconnected_ping(&ping));

    // Build pong from it
    let ts = i64::from_be_bytes(ping[1..9].try_into().unwrap());
    let motd = core::discovery::build_motd("My Server", 12345);
    let pong = core::discovery::build_pong(ts, 12345i64, &motd);

    assert_eq!(pong[0], 0x1c);
    // Timestamp echoed back
    assert_eq!(i64::from_be_bytes(pong[1..9].try_into().unwrap()), 99i64);
}
