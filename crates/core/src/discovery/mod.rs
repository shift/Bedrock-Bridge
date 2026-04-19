/// RakNet discovery service — handles Unconnected Ping/Pong on UDP 19132.
///
/// Packet structures:
/// - 0x01 Unconnected Ping: [1 byte id] [8 bytes timestamp] [16 bytes magic] [8 bytes guid]
/// - 0x1c Unconnected Pong: [1 byte id] [8 bytes timestamp] [8 bytes server_guid] [16 bytes magic] [2 bytes motd_len] [motd bytes]
///
/// Magic bytes: 00 ff ff 00 fe fe fe fe fd fd fd fd 12 34 56 78

pub const RAKNET_MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe,
    0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

pub const PACKET_UNCONNECTED_PING: u8 = 0x01;
pub const PACKET_UNCONNECTED_PONG: u8 = 0x1c;
#[allow(dead_code)]
pub const DEFAULT_PORT: u16 = 19132;
pub const PACKET_OPEN_CONNECT_REQ_1: u8 = 0x05;
pub const PACKET_OPEN_CONNECT_REPLY_1: u8 = 0x06;

/// Build a MOTD string for Bedrock Edition discovery.
/// Format: MCPE;[ServerName];[Protocol];[Version];[OnlinePlayers];[MaxPlayers];[GUID];[LevelName];[GameMode]
pub fn build_motd(server_name: &str, server_guid: i64) -> String {
    format!(
        "MCPE;{};748;1.21.0;0;10;{};World;Survival",
        server_name, server_guid
    )
}

/// Check if a packet is an Unconnected Ping (0x01).
pub fn is_unconnected_ping(data: &[u8]) -> bool {
    !data.is_empty()
        && data[0] == PACKET_UNCONNECTED_PING
        && data.len() >= 33 // 1 + 8 + 16 + 8
}

/// Build an Unconnected Pong response.
pub fn build_pong(ping_timestamp: i64, server_guid: i64, motd: &str) -> Vec<u8> {
    let motd_bytes = motd.as_bytes();
    let motd_len = motd_bytes.len() as u16;

    let mut packet = Vec::with_capacity(1 + 8 + 8 + 16 + 2 + motd_bytes.len());
    packet.push(PACKET_UNCONNECTED_PONG);
    packet.extend_from_slice(&ping_timestamp.to_be_bytes());
    packet.extend_from_slice(&server_guid.to_be_bytes());
    packet.extend_from_slice(&RAKNET_MAGIC);
    packet.extend_from_slice(&motd_len.to_be_bytes());
    packet.extend_from_slice(motd_bytes);
    packet
}

/// Check if a packet is an Open Connection Request 1 (0x05).
/// Structure: [1 id] [5 protocol] [16 magic] [1 security] [2 mtu BE + 21 padding]
#[allow(dead_code)]
pub fn is_open_connect_req_1(data: &[u8]) -> bool {
    !data.is_empty() && data[0] == PACKET_OPEN_CONNECT_REQ_1 && data.len() > 25
}

/// Extract MTU from an Open Connection Request 1 packet.
/// The MTU is encoded as the total packet length (padding counts toward MTU).
#[allow(dead_code)]
pub fn extract_mtu_req1(data: &[u8]) -> u16 {
    data.len() as u16
}

/// Cap MTU in a forwarded packet to the given maximum.
/// For 0x05 (Open Connect Request 1): truncates padding to match capped MTU.
/// For 0x06 (Open Connect Reply 1): rewrites the MTU field at byte offset 26-27 (u16 BE).
/// Returns modified packet data.
pub fn cap_mtu(data: &[u8], max_mtu: u16) -> Vec<u8> {
    if data.is_empty() {
        return data.to_vec();
    }

    match data[0] {
        PACKET_OPEN_CONNECT_REQ_1 => {
            // Truncate packet to max_mtu size (the padding IS the MTU indicator)
            if data.len() > max_mtu as usize {
                data[..max_mtu as usize].to_vec()
            } else {
                data.to_vec()
            }
        }
        PACKET_OPEN_CONNECT_REPLY_1 => {
            // 0x06 structure: [1 id] [8 guid] [16 magic] [1 security] [2 mtu BE]
            // MTU field is at offset 26-27
            let mut buf = data.to_vec();
            if buf.len() >= 28 {
                let mtu = u16::from_be_bytes([buf[26], buf[27]]);
                if mtu > max_mtu {
                    buf[26] = (max_mtu >> 8) as u8;
                    buf[27] = (max_mtu & 0xFF) as u8;
                }
            }
            buf
        }
        _ => data.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_bytes() {
        assert_eq!(RAKNET_MAGIC.len(), 16);
        assert_eq!(RAKNET_MAGIC[0], 0x00);
        assert_eq!(RAKNET_MAGIC[15], 0x78);
    }

    #[test]
    fn test_build_motd() {
        let motd = build_motd("Test Server", 12345);
        assert!(motd.starts_with("MCPE;"));
        assert!(motd.contains("Test Server"));
        assert!(motd.contains("748"));
        assert!(motd.contains("12345"));
        assert!(motd.contains("Survival"));
        let fields: Vec<&str> = motd.split(';').collect();
        assert_eq!(fields.len(), 9);
        assert_eq!(fields[0], "MCPE");
        assert_eq!(fields[1], "Test Server");
        assert_eq!(fields[2], "748");
    }

    #[test]
    fn test_is_unconnected_ping_valid() {
        let mut packet = vec![0u8; 33];
        packet[0] = PACKET_UNCONNECTED_PING;
        packet[1..9].copy_from_slice(&42i64.to_be_bytes());
        packet[9..25].copy_from_slice(&RAKNET_MAGIC);
        packet[25..33].copy_from_slice(&0i64.to_be_bytes());
        assert!(is_unconnected_ping(&packet));
    }

    #[test]
    fn test_is_unconnected_ping_wrong_id() {
        let mut packet = vec![0u8; 33];
        packet[0] = 0x1c;
        assert!(!is_unconnected_ping(&packet));
    }

    #[test]
    fn test_is_unconnected_ping_too_short() {
        let packet = vec![PACKET_UNCONNECTED_PING; 10];
        assert!(!is_unconnected_ping(&packet));
    }

    #[test]
    fn test_is_unconnected_ping_empty() {
        assert!(!is_unconnected_ping(&[]));
    }

    #[test]
    fn test_build_pong_structure() {
        let motd = "MCPE;Test;748;1.0;0;10;1234;World;Survival";
        let pong = build_pong(42i64, 0xABCDEF01i64, motd);

        assert_eq!(pong[0], PACKET_UNCONNECTED_PONG);
        assert_eq!(i64::from_be_bytes(pong[1..9].try_into().unwrap()), 42);
        assert_eq!(i64::from_be_bytes(pong[9..17].try_into().unwrap()), 0xABCDEF01i64);
        assert_eq!(&pong[17..33], &RAKNET_MAGIC);
        let motd_len = u16::from_be_bytes([pong[33], pong[34]]);
        assert_eq!(motd_len as usize, motd.len());
        assert_eq!(&pong[35..], motd.as_bytes());
    }

    #[test]
    fn test_build_pong_roundtrip() {
        let motd = "MCPE;Hello World;748;1.21.0;0;10;999;Bedrock;Creative";
        let pong = build_pong(-100i64, i64::MAX, motd);
        assert_eq!(pong[0], 0x1c);
        assert_eq!(i64::from_be_bytes(pong[1..9].try_into().unwrap()), -100);
    }

    #[test]
    fn test_is_open_connect_req_1_valid() {
        let mut packet = vec![0u8; 30];
        packet[0] = PACKET_OPEN_CONNECT_REQ_1;
        assert!(is_open_connect_req_1(&packet));
    }

    #[test]
    fn test_is_open_connect_req_1_too_short() {
        let packet = vec![PACKET_OPEN_CONNECT_REQ_1; 20];
        assert!(!is_open_connect_req_1(&packet));
    }

    #[test]
    fn test_cap_mtu_truncates_req1() {
        let mut packet = vec![PACKET_OPEN_CONNECT_REQ_1; 1500];
        packet[0] = PACKET_OPEN_CONNECT_REQ_1;
        let capped = cap_mtu(&packet, 1400);
        assert_eq!(capped.len(), 1400);
        assert_eq!(capped[0], PACKET_OPEN_CONNECT_REQ_1);
    }

    #[test]
    fn test_cap_mtu_no_change_if_under() {
        let packet = vec![PACKET_OPEN_CONNECT_REQ_1; 1000];
        let capped = cap_mtu(&packet, 1400);
        assert_eq!(capped.len(), 1000);
    }

    #[test]
    fn test_cap_mtu_rewrites_reply1() {
        let mut packet = vec![0u8; 28];
        packet[0] = PACKET_OPEN_CONNECT_REPLY_1;
        packet[26] = (1492u16 >> 8) as u8;
        packet[27] = (1492u16 & 0xFF) as u8;
        let capped = cap_mtu(&packet, 1400);
        let new_mtu = u16::from_be_bytes([capped[26], capped[27]]);
        assert_eq!(new_mtu, 1400);
    }

    #[test]
    fn test_cap_mtu_reply1_no_change_if_under() {
        let mut packet = vec![0u8; 28];
        packet[0] = PACKET_OPEN_CONNECT_REPLY_1;
        packet[26] = (1200u16 >> 8) as u8;
        packet[27] = (1200u16 & 0xFF) as u8;
        let capped = cap_mtu(&packet, 1400);
        let new_mtu = u16::from_be_bytes([capped[26], capped[27]]);
        assert_eq!(new_mtu, 1200);
    }

    #[test]
    fn test_cap_mtu_passthrough_other_packets() {
        let packet = vec![0xFF; 50];
        let capped = cap_mtu(&packet, 1400);
        assert_eq!(capped, packet);
    }

    #[test]
    fn test_cap_mtu_empty() {
        let capped = cap_mtu(&[], 1400);
        assert!(capped.is_empty());
    }

    #[test]
    fn test_extract_mtu_req1() {
        let packet = vec![0u8; 1447];
        assert_eq!(extract_mtu_req1(&packet), 1447);
    }
}
