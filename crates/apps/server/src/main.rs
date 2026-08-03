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
}

/// How often the server emits its ops heartbeat (ms). Slow on purpose — a running log, not a
/// firehose; the per-tick over-budget warning already catches spikes.
const STATS_PERIOD_MS: u64 = 10_000;

/// Rolling tick-time accumulator over one heartbeat window. Pure arithmetic so it can be
/// locked without a clock; `drain` reports the window and resets it.
#[derive(Debug, Default)]
struct WindowStats {
    count: u64,
    sum_ms: f32,
    max_ms: f32,
}

#[derive(Debug, PartialEq)]
struct WindowSummary {
    count: u64,
    avg_ms: f32,
    max_ms: f32,
}

impl WindowStats {
    fn record(&mut self, elapsed_ms: f32) {
        self.count += 1;
        self.sum_ms += elapsed_ms;
        self.max_ms = self.max_ms.max(elapsed_ms);
    }

    fn drain(&mut self) -> WindowSummary {
        let summary = WindowSummary {
            count: self.count,
            avg_ms: if self.count == 0 { 0.0 } else { self.sum_ms / self.count as f32 },
            max_ms: self.max_ms,
        };
        *self = Self::default();
        summary
    }
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
    let battle = RandomBattleConfig {
        seed,
        player_vehicle: game_core::VehicleKind::BENCHMARK,
        map: terrain::MapId::default(),
    };
    let started = Instant::now();
    let mut host = RemoteBattleServer::new(tick_config, battle, args.lobby_wait_s * 1_000, 0);
    let mut ticks: u64 = 0;
    let mut stats = WindowStats::default();
    let mut next_stats_ms = STATS_PERIOD_MS;

    info!(
        bind = args.bind,
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

        let elapsed = tick_start.elapsed();
        stats.record(elapsed.as_secs_f32() * 1_000.0);
        // A heartbeat an operator can watch on a VPS: tick load and who is connected, on a slow
        // cadence so a quiet server stays quiet. Distinct from the per-tick over-budget warning.
        if now_ms >= next_stats_ms {
            let window = stats.drain();
            info!(
                ticks,
                phase = if host.is_running() { "battle" } else { "lobby" },
                clients = host.tracked_client_count(),
                tick_avg_ms = window.avg_ms,
                tick_max_ms = window.max_ms,
                window_ticks = window.count,
                "server heartbeat"
            );
            next_stats_ms = now_ms + STATS_PERIOD_MS;
        }

        if host.is_finished() {
            info!(ticks, "battle lifecycle delivered; dedicated server exiting");
            break;
        }

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
    use super::{WindowStats, WindowSummary};

    #[test]
    fn window_stats_report_average_and_peak_then_reset() {
        let mut stats = WindowStats::default();
        for ms in [1.0_f32, 3.0, 2.0] {
            stats.record(ms);
        }
        let window = stats.drain();
        assert_eq!(window.count, 3);
        assert!((window.avg_ms - 2.0).abs() < 1.0e-6, "avg of 1/3/2 is 2.0, got {}", window.avg_ms);
        assert!((window.max_ms - 3.0).abs() < 1.0e-6, "peak is 3.0, got {}", window.max_ms);

        // Drain resets: an empty window reports zeros, never a stale peak or a divide-by-zero.
        let empty = stats.drain();
        assert_eq!(empty, WindowSummary { count: 0, avg_ms: 0.0, max_ms: 0.0 });
    }
}
