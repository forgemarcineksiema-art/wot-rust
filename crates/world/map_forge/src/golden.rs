//! The review gate, as DATA (the same philosophy as the vehicle/world forge goldens): one
//! FNV walk over the whole compiled battlefield. A change here is a deliberate map change,
//! reviewed — never an accident from a compiler, ops or blueprint edit.

use terrain::BattlefieldMap;

/// The golden compile hash per shipped map — DATA (`blueprints/goldens.ron`, hex strings),
/// so the editor can bless a deliberate map change by rewriting one data file instead of
/// editing code. The diff review stays exactly as strong.
pub fn map_golden_hashes() -> Vec<(String, u64)> {
    let entries: Vec<(String, String)> =
        ron::from_str(include_str!("../blueprints/goldens.ron")).expect("goldens.ron parses");
    entries
        .into_iter()
        .map(|(name, hex)| {
            let hash = u64::from_str_radix(&hex, 16)
                .unwrap_or_else(|_| panic!("golden for {name} is not a hex u64: {hex}"));
            (name, hash)
        })
        .collect()
}

pub fn battlefield_hash(map: &BattlefieldMap) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    str_bytes(&mut hash, &map.id);
    str_bytes(&mut hash, &map.name);
    for value in map.size_m {
        f32_bits(&mut hash, value);
    }
    for &sample in map.heightmap.samples() {
        f32_bits(&mut hash, sample);
    }
    if let Some(water) = map.water {
        f32_bits(&mut hash, water.surface_level_m);
    }
    // Standing sheets fold in AFTER the table; an empty list adds nothing, so every
    // single-table golden stands exactly where it was.
    for sheet in &map.standing_water {
        for value in sheet.rect {
            f32_bits(&mut hash, value);
        }
        f32_bits(&mut hash, sheet.surface_level_m);
    }
    if let Some(river) = map.river {
        for value in [
            river.base_x_m,
            river.axis_z_m,
            river.bow_sigma_m,
            river.bow_amp_m,
            river.wiggle_amp_m,
            river.wiggle_wave_m,
            river.corridor_half_width_m,
        ] {
            f32_bits(&mut hash, value);
        }
    }
    for zone in &map.spawn_zones {
        word(&mut hash, u64::from(zone.team));
        for value in zone.center {
            f32_bits(&mut hash, value);
        }
        f32_bits(&mut hash, zone.radius_m);
        f32_bits(&mut hash, zone.facing_yaw_rad);
    }
    for point in &map.strategic_points {
        str_bytes(&mut hash, &point.id);
        str_bytes(&mut hash, &point.name);
        word(&mut hash, point.role as u64);
        for value in point.position {
            f32_bits(&mut hash, value);
        }
        f32_bits(&mut hash, point.radius_m);
    }
    for feature in &map.features {
        word(&mut hash, feature.kind as u64);
        str_bytes(&mut hash, &feature.name);
        for value in feature.center {
            f32_bits(&mut hash, value);
        }
        f32_bits(&mut hash, feature.radius_m);
        str_bytes(&mut hash, &feature.note);
    }
    for cover in &map.static_cover {
        str_bytes(&mut hash, &cover.id);
        str_bytes(&mut hash, &cover.name);
        word(&mut hash, cover.kind as u64);
        for value in cover.center {
            f32_bits(&mut hash, value);
        }
        for value in cover.half_extents_m {
            f32_bits(&mut hash, value);
        }
        // X1: a turned box changes the map; an unturned one keeps every golden as it was.
        if cover.yaw_rad != 0.0 {
            f32_bits(&mut hash, cover.yaw_rad);
        }
    }
    for instance in &map.scenery {
        word(&mut hash, instance.kind as u64);
        for value in instance.position {
            f32_bits(&mut hash, value);
        }
        f32_bits(&mut hash, instance.yaw_rad);
        f32_bits(&mut hash, instance.scale);
        // X5: the pair's seed, hashed only when the scatter gave one — a hand-placed instance
        // hashes as it always did.
        if instance.seed != 0 {
            word(&mut hash, instance.seed);
        }
    }
    for zone in &map.capture_zones {
        str_bytes(&mut hash, &zone.id);
        for value in zone.center {
            f32_bits(&mut hash, value);
        }
        f32_bits(&mut hash, zone.radius_m);
    }
    for road in &map.roads {
        str_bytes(&mut hash, &road.id);
        word(&mut hash, road.surface as u64);
        for point in &road.points {
            for value in point {
                f32_bits(&mut hash, *value);
            }
        }
        f32_bits(&mut hash, road.width_m);
    }
    // W1: the topology folds in AFTER everything else, so a map without it keeps its golden.
    for lane in &map.lanes {
        str_bytes(&mut hash, &lane.id);
        for point in &lane.points {
            for value in point {
                f32_bits(&mut hash, *value);
            }
        }
        f32_bits(&mut hash, lane.width_m);
    }
    for path in &map.rotation_paths {
        str_bytes(&mut hash, &path.id);
        for point in &path.points {
            for value in point {
                f32_bits(&mut hash, *value);
            }
        }
        f32_bits(&mut hash, path.width_m);
    }
    for crossfire in &map.crossfires {
        str_bytes(&mut hash, &crossfire.id);
        str_bytes(&mut hash, &crossfire.a);
        str_bytes(&mut hash, &crossfire.b);
        str_bytes(&mut hash, &crossfire.lane);
    }
    hash
}

fn str_bytes(hash: &mut u64, text: &str) {
    for &byte in text.as_bytes() {
        word(hash, u64::from(byte));
    }
    word(hash, 0xff);
}

fn f32_bits(hash: &mut u64, value: f32) {
    word(hash, u64::from(value.to_bits()));
}

fn word(hash: &mut u64, value: u64) {
    *hash ^= value;
    *hash = hash.wrapping_mul(0x100_0000_01b3);
}
