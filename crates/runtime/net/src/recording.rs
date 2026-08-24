//! Frame-stream recording (N5): the accepted protocol frames of a session, appended to a file
//! as length-prefixed records. Append-only frames ARE the replay format — a reader feeds them
//! back through the same ingest the live game uses, so a recorded battle cannot drift from a
//! watched one. This is the debug recorder today and the foundation of the player-facing replay
//! (W3/S7) tomorrow.

use std::io::{Read, Write};

use crate::{NetError, ProtocolMessage, decode_frame, encode_frame};

/// The largest a recorded frame can legitimately be: a recording stores whole wire frames, and the
/// transport refuses to fragment anything past `MAX_FRAGMENTS * MAX_DATAGRAM_PAYLOAD`, so a longer
/// record is corrupt or hostile. Used to cap the read allocation before it is attempted.
const MAX_RECORDING_FRAME_LEN: usize =
    crate::transport::MAX_FRAGMENTS * crate::transport::MAX_DATAGRAM_PAYLOAD;

/// Writes one session's frames. Each record: `u32` little-endian length + the frame bytes
/// (magic, version, payload — exactly what the wire carried).
pub struct FrameRecorder<W: Write> {
    sink: W,
    frames: u64,
}

impl<W: Write> FrameRecorder<W> {
    pub fn new(sink: W) -> Self {
        Self { sink, frames: 0 }
    }

    pub fn record(&mut self, message: &ProtocolMessage) -> Result<(), NetError> {
        let frame = encode_frame(message)?;
        let len = frame.len() as u32;
        self.sink
            .write_all(&len.to_le_bytes())
            .and_then(|()| self.sink.write_all(&frame))
            .map_err(|error| NetError::Transport(error.to_string()))?;
        self.frames += 1;
        Ok(())
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }
}

/// Reads a recording back into messages. Stops cleanly at EOF; a torn tail record (the game was
/// killed mid-write) is dropped rather than erred — every complete frame before it stays valid.
pub fn read_recording<R: Read>(mut source: R) -> Result<Vec<ProtocolMessage>, NetError> {
    let mut messages = Vec::new();
    loop {
        let mut len_bytes = [0u8; 4];
        match source.read_exact(&mut len_bytes) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(NetError::Transport(error.to_string())),
        }
        let len = u32::from_le_bytes(len_bytes) as usize;
        // A recorded frame is a wire frame, so it can never exceed what the reassembler would carry.
        // Cap the allocation on the declared length: a corrupt or hostile prefix (a torn record whose
        // length word is garbage, or a crafted replay) otherwise asks for up to 4 GiB before a single
        // byte is read. Reject it instead of letting the OOM through.
        if len > MAX_RECORDING_FRAME_LEN {
            return Err(NetError::RecordingFrameTooLong { len, max: MAX_RECORDING_FRAME_LEN });
        }
        let mut frame = vec![0u8; len];
        match source.read_exact(&mut frame) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(NetError::Transport(error.to_string())),
        }
        messages.push(decode_frame(&frame)?);
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_ID: u64 = 0x1020_3040_5060_7080;

    #[test]
    fn a_recording_roundtrips_and_a_torn_tail_is_dropped_not_fatal() {
        let mut buffer = Vec::new();
        {
            let mut recorder = FrameRecorder::new(&mut buffer);
            recorder
                .record(&ProtocolMessage::Ping { session_id: SESSION_ID, client_time_us: 7 })
                .expect("record");
            recorder
                .record(&ProtocolMessage::LobbyState {
                    session_id: SESSION_ID,
                    players: 2,
                    needed: 7,
                    countdown_ticks: 9,
                })
                .expect("record");
            assert_eq!(recorder.frames(), 2);
        }
        let full = read_recording(&buffer[..]).expect("read");
        assert_eq!(full.len(), 2);
        assert!(matches!(
            full[0],
            ProtocolMessage::Ping { session_id: SESSION_ID, client_time_us: 7 }
        ));

        // Kill the game mid-write: the torn tail vanishes, everything before it survives.
        let torn = &buffer[..buffer.len() - 3];
        let recovered = read_recording(torn).expect("torn tail is not fatal");
        assert_eq!(recovered.len(), 1);
    }

    #[test]
    fn an_oversized_length_prefix_is_rejected_before_it_is_allocated() {
        // A corrupt or hostile record whose length word claims ~4 GiB must error, not send
        // `read_recording` off to `vec![0u8; 4 GiB]` and OOM the process on open. The cap is the
        // largest frame the transport could ever have fragmented, so no real record is refused.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        match read_recording(&bytes[..]) {
            Err(NetError::RecordingFrameTooLong { len, max }) => {
                assert_eq!(len, u32::MAX as usize);
                assert_eq!(max, MAX_RECORDING_FRAME_LEN);
                assert!(len > max);
            }
            other => panic!("an oversized length prefix must be rejected, got {other:?}"),
        }

        // A length one byte past the cap is refused; the cap itself is not a real record here, but
        // the boundary is what the guard turns on.
        let mut at_boundary = Vec::new();
        at_boundary.extend_from_slice(&((MAX_RECORDING_FRAME_LEN as u32) + 1).to_le_bytes());
        assert!(matches!(
            read_recording(&at_boundary[..]),
            Err(NetError::RecordingFrameTooLong { .. })
        ));
    }
}
