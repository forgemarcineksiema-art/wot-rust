use std::time::{Duration, Instant};

use battle_host::remote::RemoteBattleServer;
use battle_host::{BattleSeed, RandomBattleConfig, ServerTickConfig};
use clap::Parser;
use net::transport::UdpTransport;
use sim::{DEFAULT_SERVER_TICK_HZ, DEFAULT_SNAPSHOT_HZ};
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
    /// Seconds the lobby waits before starting with bots in the empty seats.
    #[arg(long, default_value_t = 30)]
    lobby_wait_s: u64,
    /// Deterministic battle seed (0 derives one from the clock).
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Map slug to host (e.g. "bystra-valley", "prokhorovka", "ostrogorsk", "orliny-pereval").
    /// Omitted, the catalog default is used. Before this flag the dedicated host could ONLY ever
    /// serve the default map — an operator could not run a rotation without editing the binary.
    #[arg(long)]
    map: Option<String>,
}

/// Resolve the `--map` slug against the shipped catalog, or the default when unset. A bad slug
/// fails at startup with the full list, not with a silent fallback to the wrong world.
fn resolve_map(slug: Option<&str>) -> anyhow::Result<terrain::MapId> {
    let Some(slug) = slug else {
        return Ok(terrain::MapId::default());
    };
    terrain::MapId::from_slug(slug).ok_or_else(|| {
        let catalog = terrain::MapId::ALL.iter().map(|id| id.slug()).collect::<Vec<_>>().join(", ");
        anyhow::anyhow!("--map {slug}: unknown map. Available: {catalog}")
    })
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
    // N2: `--bind` finally means what it says — the dedicated host opens the socket and serves
    // real clients; every empty seat is a bot after the lobby wait.
    let bind = args.bind.parse().map_err(|e| anyhow::anyhow!("--bind {}: {e}", args.bind))?;
    let mut transport =
        UdpTransport::bind(bind).map_err(|e| anyhow::anyhow!("bind {}: {e}", args.bind))?;
    let seed = if args.seed == 0 {
        BattleSeed::fixed(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(1),
        )
    } else {
        BattleSeed::fixed(args.seed)
    };
    let map = resolve_map(args.map.as_deref())?;
    let battle =
        RandomBattleConfig { seed, player_vehicle: game_core::VehicleKind::BENCHMARK, map };
    let started = Instant::now();
    let mut host = RemoteBattleServer::new(tick_config, battle, args.lobby_wait_s * 1_000, 0);
    let mut ticks: u64 = 0;

    info!(
        bind = args.bind,
        map = map.slug(),
        tick_rate = args.tick_rate,
        snapshot_rate = args.snapshot_rate,
        lobby_wait_s = args.lobby_wait_s,
        "dedicated server listening"
    );

    loop {
        if args.max_ticks.is_some_and(|max_ticks| ticks >= max_ticks) {
            info!(ticks, "server stopped after requested max ticks");
            break;
        }
        let tick_start = Instant::now();
        let now_ms = started.elapsed().as_millis() as u64;
        host.pump(now_ms, &mut transport);
        host.tick(now_ms, &mut transport);
        ticks += 1;
        if host.is_finished() {
            info!(ticks, "battle lifecycle delivered; dedicated server exiting");
            break;
        }

        let elapsed = tick_start.elapsed();
        if elapsed > tick_duration {
            tracing::warn!(
                elapsed_ms = elapsed.as_secs_f32() * 1_000.0,
                budget_ms = tick_duration.as_secs_f32() * 1_000.0,
                "tick over budget"
            );
        } else {
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

#[cfg(test)]
mod tests {
    use super::resolve_map;

    #[test]
    fn no_map_flag_serves_the_catalog_default() {
        assert_eq!(resolve_map(None).expect("default resolves"), terrain::MapId::default());
    }

    #[test]
    fn every_catalog_slug_resolves_to_its_map() {
        for id in terrain::MapId::ALL {
            assert_eq!(resolve_map(Some(id.slug())).expect("catalog slug resolves"), id);
        }
    }

    #[test]
    fn an_unknown_map_fails_at_startup_and_names_the_catalog() {
        let error = resolve_map(Some("no-such-map")).expect_err("unknown slug must fail");
        let message = error.to_string();
        assert!(message.contains("no-such-map"), "the error names the bad slug: {message}");
        // The failure lists real alternatives instead of a silent fallback to the wrong world.
        assert!(
            message.contains(terrain::MapId::default().slug()),
            "the error lists the catalog: {message}"
        );
    }
}
