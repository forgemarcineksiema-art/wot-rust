use game_core::TankId;
use net::{ClientInputCommand, ProtocolMessage, decode_message, encode_message};
use sim::TankCommand;

#[test]
fn input_command_round_trips_through_binary_protocol() {
    let message = ProtocolMessage::Input(ClientInputCommand {
        client_tick: 7,
        tank_id: TankId(42),
        command: TankCommand::drive(1.0, 0.25),
    });

    let bytes = encode_message(&message).expect("message should encode");
    let decoded = decode_message(&bytes).expect("message should decode");

    assert_eq!(decoded, message);
}

#[test]
fn decode_rejects_trailing_bytes() {
    let mut bytes = encode_message(&ProtocolMessage::Ping { client_time_us: 7 }).unwrap();
    bytes.push(0xAB);

    assert!(decode_message(&bytes).is_err(), "a trailing byte must not be silently dropped");
}

#[test]
fn decode_rejects_truncated_and_garbage_input() {
    let bytes = encode_message(&ProtocolMessage::Ping { client_time_us: 7 }).unwrap();
    assert!(decode_message(&bytes[..bytes.len() - 1]).is_err(), "truncated input must error");

    // Out-of-range enum discriminant / empty input must error, never panic.
    assert!(decode_message(&[0xFF, 0xFF, 0xFF, 0xFF]).is_err());
    assert!(decode_message(&[]).is_err());
}
