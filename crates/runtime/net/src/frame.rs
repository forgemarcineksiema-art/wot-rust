use crate::{NetError, PROTOCOL_VERSION, ProtocolMessage, decode_message, encode_message};

pub const FRAME_MAGIC: [u8; 4] = *b"WOT1";
pub const FRAME_HEADER_LEN: usize = FRAME_MAGIC.len() + 2;

pub fn encode_frame(message: &ProtocolMessage) -> Result<Vec<u8>, NetError> {
    let payload = encode_message(message)?;
    let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + payload.len());
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_frame(bytes: &[u8]) -> Result<ProtocolMessage, NetError> {
    if bytes.len() < FRAME_HEADER_LEN {
        return Err(NetError::FrameTooShort { len: bytes.len() });
    }

    if &bytes[..FRAME_MAGIC.len()] != FRAME_MAGIC.as_slice() {
        return Err(NetError::InvalidFrameMagic);
    }

    let actual = u16::from_le_bytes([bytes[FRAME_MAGIC.len()], bytes[FRAME_MAGIC.len() + 1]]);
    if actual != PROTOCOL_VERSION {
        return Err(NetError::ProtocolVersionMismatch { expected: PROTOCOL_VERSION, actual });
    }

    decode_message(&bytes[FRAME_HEADER_LEN..])
}
