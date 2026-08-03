use std::path::PathBuf;
use std::time::{Duration, Instant};

use battle_host::remote::RemoteBattleServer;
use battle_host::{BattleSeed, RandomBattleConfig, ServerTickConfig};
use clap::Parser;
use net::transport::UdpTransport;
use serde::Deserialize;
use sim::{DEFAULT_SERVER_TICK_HZ, DEFAULT_SNAPSHOT_HZ};
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "server", about = "Headless authoritative tank battle server")]
struct Args {
    /// Load every setting from a RON file instead of flags (an operator's checked-in server
    /// definition). When given, the other flags are IGNORED — the file is the single source of
    /// truth, so there is no half-flag/half-file ambiguity to reason about on a VPS.
    #[arg(long)]
    config: Option<PathBuf>,
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
    /// How many battles to host before the process exits. 0 (the default) hosts battles forever:
    /// when one ends, a fresh lobby opens for the next, so a rented server keeps running instead
    /// of dying after a single match. Set 1 to serve one battle and stop (the old behaviour).
    #[arg(long, default_value_t = 0)]
    max_battles: u64,
}

/// The dedicated server's whole configuration, from flags or a RON file. `serde(default)` lets a
/// config name only the fields it changes; `deny_unknown_fields` turns a typo into a startup
/// error instead of a silently ignored setting.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ServerConfig {
    bind: String,
    tick_rate: u32,
    snapshot_rate: u32,
    lobby_wait_s: u64,
    seed: u64,
    map: Option<String>,
    max_ticks: Option<u64>,
    max_battles: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:40000".to_string(),
            tick_rate: DEFAULT_SERVER_TICK_HZ,
            snapshot_rate: DEFAULT_SNAPSHOT_HZ,
            lobby_wait_s: 30,
            seed: 0,
            map: None,
            max_ticks: None,
            max_battles: 0,
        }
    }
}

impl From<&Args> for ServerConfig {
    fn from(args: &Args) -> Self {
        Self {
            bind: args.bind.clone(),
            tick_rate: args.tick_rate,
            snapshot_rate: args.snapshot_rate,
            lobby_wait_s: args.lobby_wait_s,
            seed: args.seed,
            map: args.map.clone(),
            max_ticks: args.max_ticks,
            max_battles: args.max_battles,
        }
    }
}

/// Whether to open another battle after `battles_done` have finished. `max_battles` of 0 means
/// host forever (a rented server); any positive value stops after that many.
fn should_host_another_battle(battles_done: u64, max_battles: u64) -> bool {
    max_battles == 0 || battles_done < max_battles
}

/// The seed for battle number `index` (0-based). A configured seed advances deterministically per
/// round so a rotation is reproducible yet never replays the same battle; seed 0 draws a fresh one
/// from the clock each round.
fn battle_seed(config_seed: u64, index: u64) -> BattleSeed {
    if config_seed == 0 {
        BattleSeed::fixed(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(1)
                .wrapping_add(index),
        )
    } else {
        BattleSeed::fixed(config_seed.wrapping_add(index))
    }
}

/// Parse a RON server config, surfacing a readable error for a bad field or a typo.
fn parse_config(ron_text: &str) -> anyhow::Result<ServerConfig> {
    ron::from_str(ron_text).map_err(|error| anyhow::anyhow!("server config: {error}"))
}

/// The effective config: the RON file when `--config` is given, else the flags.
fn resolve_config(args: &Args) -> anyhow::Result<ServerConfig> {
    match &args.config {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|error| anyhow::anyhow!("--config {}: {error}", path.display()))?;
            parse_config(&text)
        }
        None => Ok(ServerConfig::from(args)),
    }
}

/// Resolve the map slug against the shipped catalog, or the default when unset. A bad slug
/// fails at startup with the full list, not with a silent fallback to the wrong world.
fn resolve_map(slug: Option<&str>) -> anyhow::Result<terrain::MapId> {
    let Some(slug) = slug else {
        return Ok(terrain::MapId::default());
    };
    terrain::MapId::from_slug(slug).ok_or_else(|| {
        let catalog = terrain::MapId::ALL.iter().map(|id| id.slug()).collect::<Vec<_>>().join(", ");
        anyhow::anyhow!("map {slug}: unknown map. Available: {catalog}")
    })
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
    let config = resolve_config(&args)?;
    validate_config(&config)?;
    let tick_config = ServerTickConfig::new(config.tick_rate, config.snapshot_rate);
    let timestep = tick_config.timestep();
    let tick_duration = Duration::from_secs_f32(timestep.dt_seconds());
    // N2: `--bind` finally means what it says — the dedicated host opens the socket and serves
    // real clients; every empty seat is a bot after the lobby wait.
    let bind = config.bind.parse().map_err(|e| anyhow::anyhow!("bind {}: {e}", config.bind))?;
    let mut transport =
        UdpTransport::bind(bind).map_err(|e| anyhow::anyhow!("bind {}: {e}", config.bind))?;
    let map = resolve_map(config.map.as_deref())?;
    let started = Instant::now();
    let mut ticks: u64 = 0;
    let mut battles_done: u64 = 0;
    // The heartbeat window spans the whole process, not one battle: an operator watching a
    // rotation wants tick load across the rounds, not a counter that resets every lobby.
    let mut stats = WindowStats::default();
    let mut next_stats_ms = STATS_PERIOD_MS;

    info!(
        bind = config.bind,
        map = map.slug(),
        tick_rate = config.tick_rate,
        snapshot_rate = config.snapshot_rate,
        lobby_wait_s = config.lobby_wait_s,
        max_battles = config.max_battles,
        "dedicated server listening"
    );

    // Outer loop: one iteration hosts one battle. When it ends, a fresh lobby opens for the next
    // (a rented server keeps running) unless a total tick budget or a battle cap says otherwise.
    // Clients from the finished battle received BattleOver and must reconnect for the next one;
    // in-client auto-reconnect is a later step (N4).
    'rotation: while should_host_another_battle(battles_done, config.max_battles) {
        let now_ms = started.elapsed().as_millis() as u64;
        let battle = RandomBattleConfig {
            seed: battle_seed(config.seed, battles_done),
            player_vehicle: game_core::VehicleKind::BENCHMARK,
            map,
        };
        let mut host =
            RemoteBattleServer::new(tick_config, battle, config.lobby_wait_s * 1_000, now_ms);
        info!(battle = battles_done, "lobby open");

        loop {
            if config.max_ticks.is_some_and(|max_ticks| ticks >= max_ticks) {
                info!(ticks, "server stopped after requested max ticks");
                break 'rotation;
            }
            let tick_start = Instant::now();
            let now_ms = started.elapsed().as_millis() as u64;
            host.pump(now_ms, &mut transport);
            host.tick(now_ms, &mut transport);
            ticks += 1;

            let elapsed = tick_start.elapsed();
            stats.record(elapsed.as_secs_f32() * 1_000.0);
            // A heartbeat an operator can watch on a VPS: tick load and who is connected, on a
            // slow cadence so a quiet server stays quiet. Distinct from the per-tick over-budget
            // warning. It carries the round number too, so a rotation reads as a sequence in the
            // log rather than as one undifferentiated stream.
            if now_ms >= next_stats_ms {
                let window = stats.drain();
                info!(
                    ticks,
                    battle = battles_done,
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
                battles_done += 1;
                info!(ticks, battles_done, "battle lifecycle delivered");
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
    }

    info!(battles_done, ticks, "dedicated server exiting");
    Ok(())
}

fn validate_config(config: &ServerConfig) -> anyhow::Result<()> {
    if config.tick_rate == 0 {
        anyhow::bail!("tick_rate must be greater than zero");
    }
    if config.snapshot_rate == 0 {
        anyhow::bail!("snapshot_rate must be greater than zero");
    }
    if config.snapshot_rate > config.tick_rate {
        anyhow::bail!(
            "snapshot_rate ({}) must not exceed tick_rate ({})",
            config.snapshot_rate,
            config.tick_rate
        );
    }
    if !config.tick_rate.is_multiple_of(config.snapshot_rate) {
        anyhow::bail!(
            "tick_rate ({}) must be an integer multiple of snapshot_rate ({})",
            config.tick_rate,
            config.snapshot_rate
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ServerConfig, WindowStats, WindowSummary, battle_seed, parse_config, resolve_map,
        should_host_another_battle, validate_config,
    };

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

    #[test]
    fn max_battles_zero_hosts_forever_and_a_cap_stops_after_its_count() {
        // 0 = a rented server that keeps opening lobbies.
        assert!(should_host_another_battle(0, 0));
        assert!(should_host_another_battle(1_000_000, 0));
        // A cap of 1 serves exactly one battle (the old single-battle behaviour).
        assert!(should_host_another_battle(0, 1));
        assert!(!should_host_another_battle(1, 1));
        // A cap of 3 stops once three have finished.
        assert!(should_host_another_battle(2, 3));
        assert!(!should_host_another_battle(3, 3));
    }

    #[test]
    fn a_configured_seed_advances_per_round_so_no_two_battles_repeat() {
        let a = battle_seed(42, 0);
        let b = battle_seed(42, 1);
        let c = battle_seed(42, 2);
        assert_ne!(a, b, "consecutive rounds must not replay the same battle");
        assert_ne!(b, c);
        // Reproducible: the same round of the same configured seed is identical.
        assert_eq!(a, battle_seed(42, 0));
    }

    #[test]
    fn a_partial_config_fills_the_rest_from_defaults() {
        let config = parse_config(r#"(bind: "0.0.0.0:41000", map: Some("ostrogorsk"))"#)
            .expect("a partial config parses");
        assert_eq!(config.bind, "0.0.0.0:41000");
        assert_eq!(config.map.as_deref(), Some("ostrogorsk"));
        // Everything unnamed keeps the default the flags would have used.
        assert_eq!(
            config,
            ServerConfig {
                bind: config.bind.clone(),
                map: config.map.clone(),
                ..ServerConfig::default()
            }
        );
    }

    #[test]
    fn a_typo_in_the_config_is_a_startup_error_not_a_silent_ignore() {
        let error = parse_config(r#"(tick_rat: 30)"#).expect_err("an unknown field must fail");
        assert!(error.to_string().contains("server config"), "the error is attributed: {error}");
    }

    #[test]
    fn the_config_carries_the_same_validation_as_the_flags() {
        let bad = parse_config("(snapshot_rate: 40, tick_rate: 30)").expect("parses");
        assert!(
            validate_config(&bad).is_err(),
            "snapshot faster than tick is refused in a file too"
        );
    }

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
