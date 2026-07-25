use net::{FRAME_MAGIC, NetError, PROTOCOL_VERSION, ProtocolMessage, decode_frame, encode_frame};

const SESSION_ID: u64 = 0x1020_3040_5060_7080;

#[test]
fn frame_round_trips_message_with_protocol_version_header() {
    let message = ProtocolMessage::Ping { session_id: SESSION_ID, client_time_us: 42 };

    let bytes = encode_frame(&message).expect("frame should encode");

    assert_eq!(&bytes[..FRAME_MAGIC.len()], FRAME_MAGIC.as_slice());
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), PROTOCOL_VERSION);
    assert_eq!(decode_frame(&bytes).expect("frame should decode"), message);
}

#[test]
fn frame_rejects_mismatched_protocol_version_before_payload_decode() {
    let mut bytes =
        encode_frame(&ProtocolMessage::Ping { session_id: SESSION_ID, client_time_us: 7 })
            .expect("frame should encode");
    let mismatched = PROTOCOL_VERSION + 1;
    bytes[4..6].copy_from_slice(&mismatched.to_le_bytes());

    assert!(matches!(
        decode_frame(&bytes),
        Err(NetError::ProtocolVersionMismatch {
            expected: PROTOCOL_VERSION,
            actual
        }) if actual == mismatched
    ));
}

#[test]
fn frame_rejects_bad_magic_and_short_headers() {
    let mut bytes =
        encode_frame(&ProtocolMessage::Ping { session_id: SESSION_ID, client_time_us: 7 })
            .expect("frame should encode");
    bytes[0] ^= 0xFF;

    assert!(matches!(decode_frame(&bytes), Err(NetError::InvalidFrameMagic)));
    assert!(matches!(decode_frame(&bytes[..3]), Err(NetError::FrameTooShort { len: 3 })));
}

#[test]
fn hello_messages_advertise_current_protocol_version() {
    let client =
        ProtocolMessage::ClientHello { session_id: SESSION_ID, protocol_version: PROTOCOL_VERSION };
    let server = ProtocolMessage::ServerHello {
        session_id: SESSION_ID,
        protocol_version: PROTOCOL_VERSION,
        map_id: terrain::MapId::ProkhorovkaHill252_2,
        weather: game_core::MatchWeather::default(),
        map_content_hash: 0xDEAD_BEEF_0000_0001,
    };

    assert_eq!(decode_frame(&encode_frame(&client).unwrap()).unwrap(), client);
    assert_eq!(decode_frame(&encode_frame(&server).unwrap()).unwrap(), server);
}
