use std::time::{Duration, Instant};

use clap::Parser;
use net::ClientInputCommand;
use server::{LocalAuthoritativeServer, ServerTickConfig};
use sim::{DEFAULT_SERVER_TICK_HZ, DEFAULT_SNAPSHOT_HZ, TankCommand};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "server", about = "Headless authoritative tank battle server")]
struct Args {
    #[arg(long, default_value = "0.0.0.0:40000")]
    bind: String,
    #[arg(long, default_value_t = DEFAULT_SERVER_TICK_HZ)]
    tick_rate: u32,
    #[arg(long, default_value_t = DEFAULT_SNAPSHOT_HZ)]
    snapshot_rate: u32,
    #[arg(long)]
    max_ticks: Option<u64>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let args = Args::parse();
    validate_args(&args)?;
    let tick_config = ServerTickConfig::new(args.tick_rate, args.snapshot_rate);
    let timestep = tick_config.timestep();
    let tick_duration = Duration::from_secs_f32(timestep.dt_seconds());
    let mut server = LocalAuthoritativeServer::new(tick_config);
    let player_tank = server.player_tank();
    let mut client_tick = 0;

    info!(
        bind = args.bind,
        tick_rate = args.tick_rate,
        snapshot_rate = args.snapshot_rate,
        "server starting"
    );

    loop {
        // Check before ticking so `--max-ticks N` runs exactly N ticks (N=0 runs none).
        if args.max_ticks.is_some_and(|max_ticks| server.authoritative_tick() >= max_ticks) {
            info!(tick = server.authoritative_tick(), "server stopped after requested max ticks");
            break;
        }

        let start = Instant::now();
        let outcome = server.tick_with_input(ClientInputCommand {
            client_tick,
            tank_id: player_tank,
            command: TankCommand::drive(0.15, 0.0),
        });
        client_tick += 1;

        if let Some(snapshot) = outcome.snapshot {
            info!(tick = snapshot.server_tick, tanks = snapshot.tanks.len(), "snapshot ready");
        }

        let elapsed = start.elapsed();
        if elapsed < tick_duration {
            std::thread::sleep(tick_duration - elapsed);
        }
    }

    Ok(())
}

fn validate_args(args: &Args) -> anyhow::Result<()> {
    if args.tick_rate == 0 {
        anyhow::bail!("--tick-rate must be greater than zero");
    }
    if args.snapshot_rate == 0 {
        anyhow::bail!("--snapshot-rate must be greater than zero");
    }
    if args.snapshot_rate > args.tick_rate {
        anyhow::bail!(
            "--snapshot-rate ({}) must not exceed --tick-rate ({})",
            args.snapshot_rate,
            args.tick_rate
        );
    }
    if !args.tick_rate.is_multiple_of(args.snapshot_rate) {
        anyhow::bail!(
            "--tick-rate ({}) must be an integer multiple of --snapshot-rate ({})",
            args.tick_rate,
            args.snapshot_rate
        );
    }
    Ok(())
}
