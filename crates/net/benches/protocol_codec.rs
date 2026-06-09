use criterion::{Criterion, criterion_group, criterion_main};
use game_core::TankId;
use net::{ClientInputCommand, ProtocolMessage, decode_message, encode_message};
use sim::TankCommand;

fn protocol_codec_benchmark(c: &mut Criterion) {
    let message = ProtocolMessage::Input(ClientInputCommand {
        client_tick: 7,
        tank_id: TankId(42),
        command: TankCommand::drive_with_turret(1.0, 0.25, 0.5),
    });

    c.bench_function("protocol_input_encode_decode", |b| {
        b.iter(|| {
            let bytes = encode_message(&message).expect("benchmark message should encode");
            decode_message(&bytes).expect("benchmark message should decode")
        });
    });
}

criterion_group!(benches, protocol_codec_benchmark);
criterion_main!(benches);
