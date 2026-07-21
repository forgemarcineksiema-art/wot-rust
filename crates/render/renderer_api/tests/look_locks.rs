//! Locks for the art-direction bible (`docs/art-direction-policy.md`): the global rules every
//! lighting profile must obey — value structure, the ground-saturation window, and the
//! warm-key/cool-fill colour axis. These are the CPU half of the look harness (statistical,
//! always-on); the GPU half is the opt-in `look_goldens` test in the client crate. Each test
//! names the rule it locks. Widening any bound is a deliberate art-direction diff, never a
//! tuning accident.

use renderer_api::SceneLighting;

fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// HSV-style saturation of a linear colour: 0 = grey, 1 = fully chromatic.
fn saturation(rgb: [f32; 3]) -> f32 {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    if max <= 0.0 { 0.0 } else { (max - min) / max }
}

/// Colour temperature proxy: red over blue. > 1 reads warm, < 1 reads cool.
fn warmth(rgb: [f32; 3]) -> f32 {
    rgb[0] / rgb[2].max(1.0e-6)
}

fn outdoor_profiles() -> [(&'static str, SceneLighting); 6] {
    [
        ("battlefield_default", SceneLighting::battlefield_default()),
        ("bystra_clear_afternoon", SceneLighting::bystra_clear_afternoon()),
        ("bystra_rain", SceneLighting::bystra_rain()),
        ("bystra_dawn_fog", SceneLighting::bystra_dawn_fog()),
        ("prokhorovka_golden_evening", SceneLighting::prokhorovka_golden_evening()),
        ("prokhorovka_overcast", SceneLighting::prokhorovka_overcast()),
    ]
}

/// An overcast lid (rain, dry overcast) compresses the light axis; the clear-sky looks must
/// keep it decisive. The threshold matches the cloud tests: bias >= 0.3 closes the lid.
fn is_overcast_lid(l: &SceneLighting) -> bool {
    l.cloud_coverage_bias >= 0.3
}

/// The reference mid-tone ground albedo the value-structure bands are measured against: a
/// dusty field tone, inside the ground-saturation window. Policy data, quoted in the bible.
const REFERENCE_MID_ALBEDO: [f32; 3] = [0.28, 0.27, 0.24];

/// Fraction of the key a typically inclined sunlit surface receives (cos of incidence).
const KEY_LIT_FRACTION: f32 = 0.75;

/// Graded display luma of deep shade: the reference albedo lit by ambient + fill only (the key
/// is occluded — a cast shadow, the shaded flank of a hull).
fn shade_luma(l: &SceneLighting) -> f32 {
    let radiance = [
        REFERENCE_MID_ALBEDO[0] * (l.ambient_rgb[0] + l.fill_rgb[0]),
        REFERENCE_MID_ALBEDO[1] * (l.ambient_rgb[1] + l.fill_rgb[1]),
        REFERENCE_MID_ALBEDO[2] * (l.ambient_rgb[2] + l.fill_rgb[2]),
    ];
    luminance(l.grade_reference(radiance))
}

/// Graded display luma of the sunlit mid field: the reference albedo lit by ambient + raked key.
fn field_luma(l: &SceneLighting) -> f32 {
    let radiance = [
        REFERENCE_MID_ALBEDO[0] * (l.ambient_rgb[0] + l.key_rgb[0] * KEY_LIT_FRACTION),
        REFERENCE_MID_ALBEDO[1] * (l.ambient_rgb[1] + l.key_rgb[1] * KEY_LIT_FRACTION),
        REFERENCE_MID_ALBEDO[2] * (l.ambient_rgb[2] + l.key_rgb[2] * KEY_LIT_FRACTION),
    ];
    luminance(l.grade_reference(radiance))
}

/// Graded display luma of the sky at the horizon — the brightest plane of the picture.
fn sky_luma(l: &SceneLighting) -> f32 {
    luminance(l.grade_reference(l.sky_horizon_rgb))
}

/// RULE 1 (value structure): every outdoor look reads in three separated value planes —
/// dark shade mass below the mid field below the bright sky. A profile whose shade floats up
/// to the field, or whose field washes into the sky, has lost the painting's armature.
#[test]
fn every_outdoor_look_reads_in_three_separated_value_planes() {
    for (name, l) in outdoor_profiles() {
        let shade = shade_luma(&l);
        let field = field_luma(&l);
        let sky = sky_luma(&l);
        assert!(
            field >= shade + 0.06,
            "{name}: the sunlit field must sit clear of the shade: shade {shade:.3} field {field:.3}"
        );
        assert!(
            sky >= field + 0.10,
            "{name}: the sky must stay the brightest plane: field {field:.3} sky {sky:.3}"
        );
    }
}

/// RULE 1, absolute bands for the clear-sky looks (an overcast lid legitimately compresses the
/// range): shade is real shade, the field is a readable mid, the horizon glows.
#[test]
fn clear_sky_looks_land_their_value_bands() {
    for (name, l) in outdoor_profiles() {
        if is_overcast_lid(&l) {
            continue;
        }
        let shade = shade_luma(&l);
        let field = field_luma(&l);
        let sky = sky_luma(&l);
        // The deepest shade (ambient+fill only) may legitimately reach true black — the black
        // point exists so shade reads as shade. The lock is the CEILING: shade never floats
        // into the mid band. (Anti-crush is locked separately: mid grey survives the grade.)
        assert!(shade <= 0.25, "{name}: deep shade must stay in the dark band, got {shade:.3}");
        assert!(
            (0.28..=0.58).contains(&field),
            "{name}: the sunlit field must land in the mid band, got {field:.3}"
        );
        assert!(
            (0.60..=0.95).contains(&sky),
            "{name}: the horizon must land in the bright band, got {sky:.3}"
        );
    }
}

/// RULE 2 (saturation window): the ground plane is muted — the reference palette the terrain
/// authors against stays under the ceiling. Chroma lives in the sky and the light, not in the
/// dirt; grass is grey-green, never lawn-green.
#[test]
fn the_ground_palette_stays_inside_the_saturation_window() {
    // The policy ground swatches (linear albedo): steppe grass, dry straw, tilled dirt, chalk
    // break, wet mud. Terrain material work (Terrain 2.0) binds its layer albedos to this
    // discipline; the swatches are quoted in the bible.
    let swatches: [(&str, [f32; 3]); 5] = [
        ("steppe_grass", [0.30, 0.33, 0.22]),
        ("dry_straw", [0.45, 0.40, 0.26]),
        ("tilled_dirt", [0.32, 0.27, 0.21]),
        ("chalk_break", [0.55, 0.54, 0.50]),
        ("wet_mud", [0.20, 0.17, 0.14]),
    ];
    for (name, albedo) in swatches {
        let sat = saturation(albedo);
        assert!(sat <= 0.45, "ground swatch {name} exceeds the saturation window: {sat:.3} > 0.45");
    }
}

/// RULE 2, the grade side: no outdoor profile may push saturation harder than the outdoor
/// ceiling (tighter than the generic display envelope — a 1.5 lift belongs to no battlefield),
/// and the grade never *invents* chroma: a grey input leaves the display transform grey.
#[test]
fn outdoor_grades_lift_saturation_gently_and_never_tint_grey() {
    for (name, l) in outdoor_profiles() {
        assert!(
            l.saturation <= 1.30,
            "{name}: outdoor saturation lift {} exceeds the 1.30 ceiling",
            l.saturation
        );
        for value in [0.05, 0.18, 0.5, 1.5] {
            let graded = l.grade_reference([value; 3]);
            assert!(
                saturation(graded) < 1.0e-6,
                "{name}: the grade tinted a neutral grey {value}: {graded:?}"
            );
        }
    }
}

/// RULE 2, the content side: the ground palette envelope — saturation window, albedo range,
/// detail/gloss lanes, and the value separation (straw over grass, rock over dirt). The
/// SHIPPED palettes live in the map blueprints now and the map report enforces this same
/// envelope as compile Errors (`map_forge::report`); what renderer_api still owns is the
/// neutral fallback set, held to the identical discipline here.
#[test]
fn terrain_material_sets_stay_inside_the_saturation_window() {
    use renderer_api::TerrainMaterialSet;
    for (name, set) in [("default", TerrainMaterialSet::default())] {
        for (i, layer) in set.layers.iter().enumerate() {
            let sat = saturation(layer.albedo);
            assert!(
                sat <= 0.45,
                "{name} layer {i} albedo {:?} exceeds the saturation window: {sat:.3}",
                layer.albedo
            );
            for c in layer.albedo {
                assert!((0.05..=0.7).contains(&c), "{name} layer {i}: albedo {c} out of range");
            }
            assert!(
                (0.0..=1.2).contains(&layer.detail),
                "{name} layer {i}: detail {} out of envelope",
                layer.detail
            );
            assert!((0.0..=0.3).contains(&layer.gloss), "{name} layer {i}: ground gloss envelope");
        }
        assert!(
            (0.0..=1.0).contains(&set.macro_normal_strength),
            "{name}: macro normal strength out of range"
        );
        // Rule 1 support: the straw layer out-lumes the grass (the patchwork must keep value
        // separation, not just hue), and rock is the brightest ground (chalk break reads).
        let luma_of = |i: usize| luminance(set.layers[i].albedo);
        assert!(luma_of(1) > luma_of(0), "{name}: straw must read lighter than grass");
        assert!(luma_of(3) > luma_of(2), "{name}: broken rock must read lighter than dirt");
    }
}

/// RULE 3 (warm key / cool fill): the light axis of the whole art direction. The sun key is
/// always warmer than the sky fill; an overcast lid compresses the split but never inverts it,
/// and the clear-sky looks keep it decisive (a genuinely warm key against a genuinely cool fill).
#[test]
fn the_key_is_always_warmer_than_the_fill() {
    for (name, l) in outdoor_profiles() {
        let key = warmth(l.key_rgb);
        let fill = warmth(l.fill_rgb);
        assert!(
            key > fill,
            "{name}: the key must stay warmer than the fill: key {key:.3} vs fill {fill:.3}"
        );
        if !is_overcast_lid(&l) {
            assert!(key > 1.0, "{name}: a clear-sky key reads warm, got {key:.3}");
            assert!(fill < 1.0, "{name}: a clear-sky fill reads cool, got {fill:.3}");
        }
    }
}

/// RULE 4 (atmospheric depth): every outdoor look carries real aerial perspective, and the fog
/// always fades toward the horizon colour — the depth planes and the sky are one atmosphere.
/// (The 400 m spotting-fairness bound on the fog amount is locked map-wide in the client's
/// weather tests; this locks the structural half.)
#[test]
fn every_outdoor_look_carries_aerial_perspective_into_the_horizon() {
    for (name, l) in outdoor_profiles() {
        assert!(l.fog_density > 0.0, "{name}: an outdoor look needs air in it");
        assert!(
            luminance(l.sky_horizon_rgb) > luminance(l.sky_zenith_rgb),
            "{name}: the horizon haze must out-lume the zenith or depth reads upside down"
        );
    }
}

/// RULE 6 (light is the only bling): bloom is a small, energy-proportional composite and the
/// vignette never reads as a lens. Every profile - outdoor and garage alike - stays inside the
/// bible's ceilings; a dream-sequence bloom or a tunnel vignette is a locked-out accident.
#[test]
fn bloom_and_vignette_stay_inside_the_bible_ceilings() {
    let all = [
        ("battlefield_default", SceneLighting::battlefield_default()),
        ("bystra_clear_afternoon", SceneLighting::bystra_clear_afternoon()),
        ("bystra_rain", SceneLighting::bystra_rain()),
        ("bystra_dawn_fog", SceneLighting::bystra_dawn_fog()),
        ("prokhorovka_golden_evening", SceneLighting::prokhorovka_golden_evening()),
        ("prokhorovka_overcast", SceneLighting::prokhorovka_overcast()),
        ("garage_studio", SceneLighting::garage_studio()),
        ("garage_workshop", SceneLighting::garage_workshop()),
        ("garage_hero", SceneLighting::garage_hero()),
    ];
    for (name, l) in all {
        assert!(
            (0.0..=0.10).contains(&l.bloom_weight),
            "{name}: bloom weight {} outside the [0, 0.10] ceiling",
            l.bloom_weight
        );
        assert!(
            (0.0..=0.15).contains(&l.vignette),
            "{name}: vignette {} outside the [0, 0.15] ceiling",
            l.vignette
        );
    }
    // Every outdoor look carries SOME bloom - the sun disc and tracers must glow everywhere.
    for (name, l) in outdoor_profiles() {
        assert!(l.bloom_weight > 0.0, "{name}: outdoor looks carry a live bloom weight");
    }
}

/// RULE 4/6 envelopes for Atmosphere 2.0: the valley haze pools but never floods (the 400 m
/// fairness sweep in the client bounds the SUM of both layers), crepuscular rays stay painted
/// accents, the second cloud layer stays a veil, and the storm front belongs to the squalls.
#[test]
fn atmosphere_two_point_oh_stays_inside_its_envelopes() {
    for (name, l) in outdoor_profiles() {
        assert!(
            (0.0..=0.0005).contains(&l.valley_haze_density),
            "{name}: valley haze density {} out of envelope",
            l.valley_haze_density
        );
        assert!(
            (0.0..=30.0).contains(&l.valley_haze_height_m),
            "{name}: valley haze height {} out of envelope",
            l.valley_haze_height_m
        );
        assert!(
            (0.0..=0.25).contains(&l.god_ray_strength),
            "{name}: god-ray strength {} out of envelope",
            l.god_ray_strength
        );
        assert!(
            (0.0..=0.6).contains(&l.cloud_sheet_opacity),
            "{name}: cloud sheet opacity {} out of envelope",
            l.cloud_sheet_opacity
        );
        assert!(
            (0.0..=0.8).contains(&l.storm_front_strength),
            "{name}: storm front strength {} out of envelope",
            l.storm_front_strength
        );
    }
    // The features live where they were authored: dawn mist pools in the valley, the low sun
    // days carry rays, and only the squalls advance a front.
    assert!(SceneLighting::bystra_dawn_fog().valley_haze_density > 0.0);
    assert!(SceneLighting::prokhorovka_golden_evening().god_ray_strength > 0.0);
    assert!(SceneLighting::bystra_rain().storm_front_strength > 0.0);
    assert_eq!(SceneLighting::prokhorovka_overcast().storm_front_strength, 0.0);
}

/// The two-layer fog model: the valley haze thickens the FLOOR without touching high ground —
/// and a profile with no valley layer evaluates bit-identically to the single-layer model.
#[test]
fn valley_haze_pools_in_the_floor_and_zero_layer_is_the_old_model() {
    let dawn = SceneLighting::bystra_dawn_fog();
    // Same distance: the valley floor fogs harder than a ridge at 20 m.
    let floor = dawn.fog_factor(400.0, 0.0);
    let ridge = dawn.fog_factor(400.0, 20.0);
    assert!(
        floor > ridge + 0.05,
        "dawn mist must pool in the floor: floor {floor:.3} vs ridge {ridge:.3}"
    );
    // Above the fade-out height the layer contributes nothing: the base model alone remains.
    let mut base_only = dawn;
    base_only.valley_haze_density = 0.0;
    base_only.valley_haze_height_m = 0.0;
    for h in [12.0, 20.0, 40.0] {
        assert_eq!(
            dawn.fog_factor(500.0, h),
            base_only.fog_factor(500.0, h),
            "above the valley the two models are bit-identical at {h} m"
        );
    }
    // A profile with no valley layer is the exact single-layer model everywhere.
    let noon = SceneLighting::battlefield_default();
    for (d, h) in [(100.0, 0.0), (400.0, 3.0), (900.0, 25.0)] {
        let manual = 1.0 - (-d * noon.fog_density * (-h * noon.fog_height_falloff).exp()).exp();
        assert!((noon.fog_factor(d, h) - manual.clamp(0.0, 1.0)).abs() < 1.0e-6);
    }
}
