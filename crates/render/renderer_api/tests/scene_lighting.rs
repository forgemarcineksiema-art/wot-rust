//! Locks the load-bearing invariants of the atmosphere lighting profile (`docs/atmosphere-policy.md`
//! phase 1): the battle look has a live rim and a real hemispheric ambient, and the ambient blend
//! is grounded — up-facing surfaces read the sky, down-facing surfaces the warmer ground bounce.

use renderer_api::SceneLighting;

fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// CPU mirror of the shaders' `hemi_ambient(n)`: blend ground->sky by the normal's up fraction.
/// This documents and locks the *model* the WGSL implements, so a degenerate profile (flat ambient)
/// or a flipped blend is caught here rather than only by eye.
fn hemi_ambient(l: &SceneLighting, normal_up: f32) -> [f32; 3] {
    let t = (normal_up * 0.5 + 0.5).clamp(0.0, 1.0);
    let mix = |g: f32, s: f32| g + (s - g) * t;
    [
        mix(l.ground_ambient_rgb[0], l.ambient_rgb[0]),
        mix(l.ground_ambient_rgb[1], l.ambient_rgb[1]),
        mix(l.ground_ambient_rgb[2], l.ambient_rgb[2]),
    ]
}

#[test]
fn battle_profile_has_a_live_rim_and_a_raking_side_sun() {
    let l = SceneLighting::battlefield_default();
    // The rim was black (silhouette flat against the sky); phase 1 turns it on.
    assert!(luminance(l.rim_rgb) > 0.05, "battle rim must be live: {:?}", l.rim_rgb);
    // The sun rakes from the side (strong horizontal component) rather than sitting near-overhead,
    // so it sculpts the sides of a low hull. Old key was [0.45, 0.82, 0.35] (mostly up).
    assert!(
        l.key_direction[0].abs() > l.key_direction[1],
        "sun should rake from the side, not top-down: {:?}",
        l.key_direction
    );
}

#[test]
fn hemispheric_ambient_is_real_not_a_flat_constant() {
    for l in [SceneLighting::battlefield_default(), SceneLighting::garage_studio()] {
        assert_ne!(
            l.ambient_rgb, l.ground_ambient_rgb,
            "sky and ground ambient must differ, or the hemisphere is a flat constant"
        );
        // The sky (upper) ambient is the cooler/brighter of the two; the ground bounce is warmer
        // and dimmer. A grounded look needs the ground darker than the sky.
        assert!(
            luminance(l.ambient_rgb) > luminance(l.ground_ambient_rgb),
            "sky ambient should out-lume the ground bounce"
        );
    }
}

#[test]
fn ambient_blend_grounds_up_faces_to_sky_and_down_faces_to_ground() {
    let l = SceneLighting::battlefield_default();
    let up = hemi_ambient(&l, 1.0);
    let down = hemi_ambient(&l, -1.0);
    let flat = hemi_ambient(&l, 0.0);
    // Approximate, not bit-exact: the blend is `g + (s - g) * t`, and at t = 1 float rounding
    // may reconstruct `s` a ULP off (0.10 + (0.24 - 0.10) = 0.23999998). The shader computes
    // the same expression, so the tolerance IS the mirror's precision, not a loosened model.
    for c in 0..3 {
        assert!(
            (up[c] - l.ambient_rgb[c]).abs() < 1.0e-6,
            "a fully up-facing surface takes the sky ambient: {up:?} vs {:?}",
            l.ambient_rgb
        );
        assert!(
            (down[c] - l.ground_ambient_rgb[c]).abs() < 1.0e-6,
            "a fully down-facing surface takes the ground bounce: {down:?} vs {:?}",
            l.ground_ambient_rgb
        );
    }
    // A horizontal-facing surface sits between the two hemispheres.
    assert!(luminance(down) < luminance(flat) && luminance(flat) < luminance(up));
}

#[test]
fn battle_profile_has_aerial_perspective_and_the_interior_does_not() {
    let battle = SceneLighting::battlefield_default();
    // Phase 2: the battlefield fades distant surfaces into the horizon haze for 1000 m depth.
    assert!(
        battle.fog_density > 0.0,
        "battle needs aerial-perspective fog: {}",
        battle.fog_density
    );
    // The horizon (also the fog colour) must differ from the zenith, or the gradient sky and the
    // fade-to-sky are degenerate flat colours.
    assert_ne!(
        battle.sky_horizon_rgb, battle.sky_zenith_rgb,
        "gradient sky must have a distinct horizon and zenith"
    );
    // The horizon is the paler, hazier end of the sky the fog fades toward.
    assert!(
        luminance(battle.sky_horizon_rgb) > luminance(battle.sky_zenith_rgb),
        "horizon haze should out-lume the deeper zenith"
    );
    // Interior profiles must not apply aerial perspective (there is no open air/horizon).
    for interior in [
        SceneLighting::garage_studio(),
        SceneLighting::garage_workshop(),
        SceneLighting::garage_hero(),
    ] {
        assert_eq!(interior.fog_density, 0.0, "garage interiors carry no distance fog");
    }
}

fn all_profiles() -> [(&'static str, SceneLighting); 8] {
    [
        ("battlefield_default", SceneLighting::battlefield_default()),
        ("bystra_clear_afternoon", SceneLighting::bystra_clear_afternoon()),
        ("bystra_rain", SceneLighting::bystra_rain()),
        ("bystra_dawn_fog", SceneLighting::bystra_dawn_fog()),
        ("prokhorovka_golden_evening", SceneLighting::prokhorovka_golden_evening()),
        ("prokhorovka_overcast", SceneLighting::prokhorovka_overcast()),
        ("garage_studio", SceneLighting::garage_studio()),
        ("garage_workshop", SceneLighting::garage_workshop()),
    ]
}

/// Local fill pools are an INTERIOR tool: every outdoor profile ships all-off arrays, so the
/// battlefield image is bit-identical to the pre-pool renderer (a zero radius disables the
/// slot before any math runs).
#[test]
fn outdoor_profiles_carry_no_local_lights() {
    for (name, l) in all_profiles() {
        if name.starts_with("garage") {
            continue;
        }
        assert!(
            l.local_lights.iter().all(|light| light.radius_m == 0.0),
            "{name}: outdoor profiles must not carry local lights"
        );
    }
}

/// CPU mirror of the shader falloff: full at the emitter, exactly zero at and past the radius,
/// zero always for a disabled slot — the attenuation model is testable without a GPU.
#[test]
fn local_light_attenuation_is_bounded_and_vanishes_at_radius() {
    let light = renderer_api::LocalLight {
        position: [0.0; 3],
        radius_m: 10.0,
        rgb: [1.0, 0.9, 0.7],
        intensity: 1.5,
    };
    assert!((light.attenuation_at(0.0) - 1.0).abs() < 1.0e-6, "full strength at the emitter");
    let mid = light.attenuation_at(5.0);
    assert!(mid > 0.0 && mid < 1.0, "falls off smoothly, got {mid}");
    assert_eq!(light.attenuation_at(10.0), 0.0, "vanishes exactly at the radius");
    assert_eq!(light.attenuation_at(25.0), 0.0, "stays zero beyond it");
    assert_eq!(renderer_api::LocalLight::OFF.attenuation_at(0.0), 0.0, "disabled slot is dark");
}

/// The garage rig hangs its light where the work is: warm pools over the turntable, every
/// emitter inside the hall, and enough of them that the hall reads worked-in. Readable-light
/// doctrine (2026-08-05): every pool is WARM lamp light — the cool "pane-glow" pool died with
/// the fake panes, because daylight enters this room through the skylight key only. Positions
/// must agree with the lamp housings the hangar mesh hangs.
#[test]
fn garage_hero_pools_the_light_where_the_work_is() {
    let rig = SceneLighting::garage_hero().local_lights;
    let enabled: Vec<_> = rig.iter().filter(|l| l.radius_m > 0.0).collect();
    assert!(enabled.len() >= 4, "the hall is worked-in: got {} lights", enabled.len());

    let warm_by_turntable = enabled
        .iter()
        .filter(|l| {
            l.rgb[0] > l.rgb[2] && (l.position[0].powi(2) + l.position[2].powi(2)).sqrt() < 6.0
        })
        .count();
    assert!(warm_by_turntable >= 2, "warm pools hang over the turntable: {warm_by_turntable}");

    for light in &enabled {
        // Every pool is warm lamp light, with ONE earned exception (B1): the gate beam. Cool
        // is only honest where daylight has an address — the ajar gate opening at the −z end
        // (hand-synced with hangar.rs: gate wall at z = −22, opening under the slat stack).
        // A cool pool anywhere else is still daylight from nowhere.
        let at_the_gate = light.position[2] < -19.0 && light.position[0].abs() < 5.0;
        assert!(
            light.rgb[0] > light.rgb[2] || at_the_gate,
            "a cool pool away from the gate is daylight from nowhere: {:?} at {:?}",
            light.rgb,
            light.position
        );
        // Hand-synced with the A1 nave (hangar.rs: HALF_X 11, HALF_Z 22, truss chord 9) —
        // renderer_api sits under scene_build and cannot read the constants. If the hall
        // changes size, these change with it or this test proves nothing.
        assert!(
            light.position[0].abs() < 11.0
                && light.position[2].abs() < 22.0
                && light.position[1] > 0.0
                && light.position[1] < 9.0,
            "light at {:?} must hang inside the hall",
            light.position
        );
    }
}

/// E2: the bench tube's flicker is a CHARACTER, not a strobe — bounded, mostly lit, with
/// real dips, deterministic, and exactly healthy at the golden harness's frozen second so
/// the locked frame needs no special case.
#[test]
fn the_tube_flickers_like_a_tube_and_holds_at_the_review_second() {
    let mut dips = 0usize;
    let mut sum = 0.0f32;
    let mut samples = 0usize;
    let mut t = 0.0f32;
    while t < 240.0 {
        let factor = renderer_api::fluorescent_flicker(t);
        assert!((0.3..=1.0).contains(&factor), "flicker out of band at {t}: {factor}");
        if factor < 1.0 {
            dips += 1;
        }
        sum += factor;
        samples += 1;
        t += 0.05;
    }
    let mean = sum / samples as f32;
    assert!(mean > 0.9, "a tube is mostly LIT: mean {mean}");
    assert!(dips > 0, "a tube that never dips is a lamp, not a character");
    // The harness freezes the garage at 12.0 s: the tube must be healthy there, which makes
    // `garage_hero_at(12.0)` bit-identical to `garage_hero()` and keeps the goldens exact.
    assert_eq!(renderer_api::fluorescent_flicker(12.0), 1.0);
    assert_eq!(
        SceneLighting::garage_hero_at(12.0),
        SceneLighting::garage_hero(),
        "the locked frame's rig is the resting rig, to the bit"
    );
}

#[test]
fn every_profile_grades_within_the_sane_display_envelope() {
    // The grade is data now, so a fat-fingered preset (exposure 11.0, black point 0.8) would
    // silently crush the image; these bounds are the envelope the image formation was designed
    // for. Widening them is a deliberate diff, not a tuning accident.
    for (name, l) in all_profiles() {
        assert!((0.5..=2.0).contains(&l.exposure), "{name}: exposure {} out of range", l.exposure);
        assert!(
            (0.0..=0.08).contains(&l.black_point),
            "{name}: black point {} out of range",
            l.black_point
        );
        assert!(
            (0.8..=1.5).contains(&l.saturation),
            "{name}: saturation {} out of range",
            l.saturation
        );
        assert!((0.9..=1.4).contains(&l.contrast), "{name}: contrast {} out of range", l.contrast);
    }
}

#[test]
fn a_neutral_grade_only_applies_the_tone_curve() {
    // With exposure 1 / black 0 / saturation 1 / contrast 1, grade_reference reduces to the bare
    // ACES-lite curve — the identity of the grading stage. Locks that no hidden constant remains
    // in the pipeline now that the old hardcoded 1.18/1.10 moved into the profiles.
    let mut l = SceneLighting::battlefield_default();
    l.exposure = 1.0;
    l.black_point = 0.0;
    l.saturation = 1.0;
    l.contrast = 1.0;
    let aces = |x: f32| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    for value in [0.0, 0.02, 0.18, 0.5, 1.0, 4.0] {
        let graded = l.grade_reference([value; 3]);
        for channel in graded {
            assert!(
                (channel - aces(value)).abs() < 1.0e-6,
                "neutral grade must be the bare curve at {value}: {channel} vs {}",
                aces(value)
            );
        }
    }
}

#[test]
fn the_battle_profile_pulls_deep_shade_to_true_black() {
    // The point of the black point: the ACES-lite curve lifts near-blacks (aces(0.02) ≈ 0.017,
    // never 0), which is why cast shadows read milky. The battle grade must map a 0.02 HDR input
    // essentially to black — real shade, not grey.
    let battle = SceneLighting::battlefield_default();
    let graded = battle.grade_reference([0.02; 3]);
    for channel in graded {
        assert!(channel < 0.005, "deep shade must grade to black, got {channel}");
    }
    // And it must not crush everything: mid grey stays mid, highlights stay bright.
    let mid = battle.grade_reference([0.18; 3]);
    assert!(mid[1] > 0.2 && mid[1] < 0.6, "mid grey survives the grade: {}", mid[1]);
    let bright = battle.grade_reference([4.0; 3]);
    assert!(bright[1] > 0.85, "highlights stay bright through the grade: {}", bright[1]);
}

#[test]
fn only_the_black_point_may_produce_pure_black() {
    // THE CONTRAST STEP IS A CURVE, NOT A CLIFF. It used to be `(x - 0.5) * contrast + 0.5`, a
    // straight line of slope `contrast` — which drives everything below `0.5 - 0.5/contrast`
    // negative, where the clamp turns it into pure black. At the shipped 1.12–1.15 that is a
    // dead band reaching up to 0.054–0.065 of the post-curve range, and a hull's shaded flank
    // lives inside it: the backlit review frame graded its median vehicle pixel from 0.068 to
    // 0.016, with its darkest twentieth at exactly 0.000.
    //
    // Deep shade still reaches true black — `the_battle_profile_pulls_deep_shade_to_true_black`
    // above stays green, because that is the BLACK POINT's job and the black point is untouched.
    // What may not happen is a second, undeclared crush stacked on top of it. So: every radiance
    // the black point does not eat has to come out of the grade visible.
    let aces = |x: f32, exposure: f32| {
        let x = x * exposure;
        ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
    };
    for (name, lighting) in [
        ("battlefield", SceneLighting::battlefield_default()),
        ("clear afternoon", SceneLighting::bystra_clear_afternoon()),
        ("golden evening", SceneLighting::prokhorovka_golden_evening()),
        ("overcast", SceneLighting::prokhorovka_overcast()),
        ("garage hero", SceneLighting::garage_hero()),
    ] {
        let mut checked = 0;
        for step in 1..=2000 {
            let hdr = step as f32 * 0.001;
            if aces(hdr, lighting.exposure) <= lighting.black_point + 1.0e-4 {
                continue;
            }
            let graded = lighting.grade_reference([hdr; 3])[1];
            assert!(
                graded > 0.0,
                "{name}: radiance {hdr} clears the black point but the grade still crushes it to \
                 pure black — the contrast step has a cliff again"
            );
            checked += 1;
        }
        assert!(checked > 100, "{name}: the sweep must exercise the band above the black point");
    }
}

#[test]
fn contrast_still_separates_the_midtones_it_is_named_for() {
    // The toe must not have cost the knob its job. `contrast` is the slope at mid grey, so
    // raising it has to push a value below mid down and one above mid up — measured across the
    // step, the separation must widen.
    let mut soft = SceneLighting::battlefield_default();
    soft.contrast = 1.0;
    let mut hard = SceneLighting::battlefield_default();
    hard.contrast = 1.30;
    let separation =
        |l: &SceneLighting| l.grade_reference([0.6; 3])[1] - l.grade_reference([0.1; 3])[1];
    assert!(
        separation(&hard) > separation(&soft) + 0.01,
        "raising contrast must widen midtone separation: {:.4} vs {:.4}",
        separation(&hard),
        separation(&soft)
    );
}

#[test]
fn exposure_brightens_monotonically_before_the_curve() {
    let mut l = SceneLighting::battlefield_default();
    l.exposure = 0.8;
    let dim = l.grade_reference([0.3; 3]);
    l.exposure = 1.4;
    let bright = l.grade_reference([0.3; 3]);
    assert!(
        bright[1] > dim[1] + 0.05,
        "higher exposure must brighten the same radiance: {} vs {}",
        bright[1],
        dim[1]
    );
}

#[test]
fn every_outdoor_profile_keeps_the_hemispheric_ambient_real() {
    for (name, l) in all_profiles() {
        assert_ne!(
            l.ambient_rgb, l.ground_ambient_rgb,
            "{name}: sky and ground ambient must differ, or the hemisphere is a flat constant"
        );
    }
}

#[test]
fn the_golden_evening_sun_is_genuinely_low_and_warm() {
    let l = SceneLighting::prokhorovka_golden_evening();
    // A real golden hour: the NORMALIZED sun elevation sits in the long-shadow band. This is the
    // look the far shadow cascade exists to sell — a near-noon sun here would waste it.
    let d = l.key_direction;
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let elevation = d[1] / len;
    assert!(
        (0.12..=0.35).contains(&elevation),
        "golden-hour sun elevation must rake long shadows, got {elevation}"
    );
    // Amber light: red over green over blue, decisively.
    let [r, g, b] = l.key_rgb;
    assert!(r > g && g > b && r > b * 1.8, "the evening key must be warm amber: {:?}", l.key_rgb);
    // The overcast sibling is the opposite pole: high flat sun, near-neutral cool key.
    let o = SceneLighting::prokhorovka_overcast();
    assert!(o.key_direction[1] > o.key_direction[0].abs(), "overcast light comes from the lid");
    assert_eq!(o.cloud_shadow_strength, 0.0, "no cloud patches under a full lid");
    assert!(o.cloud_coverage_bias > 0.3, "the overcast profile must actually close the lid");
}

#[test]
fn the_prokhorovka_variants_are_three_different_days() {
    let noon = SceneLighting::battlefield_default();
    let evening = SceneLighting::prokhorovka_golden_evening();
    let overcast = SceneLighting::prokhorovka_overcast();
    let signature = |l: &SceneLighting| (l.key_direction, l.key_rgb, l.sky_horizon_rgb);
    assert_ne!(signature(&noon), signature(&evening));
    assert_ne!(signature(&noon), signature(&overcast));
    assert_ne!(signature(&evening), signature(&overcast));
}

#[test]
fn each_sky_look_owns_a_distinct_cloud_layer() {
    // Rain reads as an overcast lid: much thicker coverage than the clear looks, near-full
    // opacity — and NO terrain cloud shade (the lid itself is the shadow, moving patches under a
    // sunless sky would look wrong).
    let rain = SceneLighting::bystra_rain();
    let clear = SceneLighting::bystra_clear_afternoon();
    let dawn = SceneLighting::bystra_dawn_fog();
    assert!(
        rain.cloud_coverage_bias > clear.cloud_coverage_bias + 0.2,
        "rain must push the coverage into a lid: {} vs {}",
        rain.cloud_coverage_bias,
        clear.cloud_coverage_bias
    );
    assert_eq!(rain.cloud_shadow_strength, 0.0, "no cloud shade under an overcast lid");
    // The three Bystra looks are genuinely different skies, not one cloud layer recoloured.
    let params = |l: &SceneLighting| {
        (l.cloud_coverage_bias, l.cloud_scale, l.cloud_opacity, l.cloud_shadow_strength)
    };
    assert_ne!(params(&rain), params(&clear));
    assert_ne!(params(&clear), params(&dawn));
    assert_ne!(params(&rain), params(&dawn));
    // Interiors carry no sky layer at all.
    for interior in [SceneLighting::garage_studio(), SceneLighting::garage_workshop()] {
        assert_eq!(interior.cloud_opacity, 0.0);
        assert_eq!(interior.cloud_shadow_strength, 0.0);
        assert_eq!(interior.fog_sun_scatter, 0.0);
    }
}

#[test]
fn sun_scatter_and_clouds_never_touch_the_fog_amount() {
    // The 400 m spotting-fairness bound rests on fog_factor; the sky phase adds COLOUR only.
    // Wildly different scatter/cloud settings must leave the fog amount bit-identical.
    let base = SceneLighting::battlefield_default();
    let mut wild = base;
    wild.fog_sun_scatter = 1.0;
    wild.cloud_coverage_bias = 0.5;
    wild.cloud_opacity = 1.0;
    wild.cloud_shadow_strength = 1.0;
    for (distance, height) in [(100.0, 0.0), (400.0, 0.0), (600.0, 40.0), (900.0, 5.0)] {
        assert_eq!(
            base.fog_factor(distance, height),
            wild.fog_factor(distance, height),
            "fog amount must not depend on the sky colour params"
        );
    }
}

#[test]
fn sun_softness_is_explicit_profile_data_not_a_fog_side_effect() {
    // The sky pass used to derive the disc's hardness from fog_density * 700 — retuning the air
    // for the 400 m spotting-fairness bound silently retuned the sun. Now every profile owns an
    // explicit softness, and the fog knob must leave it untouched.
    for (name, l) in all_profiles() {
        assert!(
            (0.0..=1.0).contains(&l.sun_softness),
            "{name}: sun_softness is a 0..1 mix factor, got {}",
            l.sun_softness
        );
    }
    let mut retuned = SceneLighting::bystra_rain();
    retuned.fog_density *= 3.0;
    assert_eq!(
        retuned.sun_softness,
        SceneLighting::bystra_rain().sun_softness,
        "retuning the fog for fairness must not move the sun's softness"
    );
    // The looks stay distinct poles: a lead rain sky holds a milkier sun than the clear noon,
    // and interiors (no sky pass worth the name) carry none.
    assert!(
        SceneLighting::bystra_rain().sun_softness
            > SceneLighting::battlefield_default().sun_softness + 0.3,
        "rain must read markedly softer than the clear noon"
    );
    for interior in [SceneLighting::garage_studio(), SceneLighting::garage_workshop()] {
        assert_eq!(interior.sun_softness, 0.0);
    }
}

#[test]
fn garage_hero_lifts_the_subject_off_the_workshop_silhouette() {
    // The hero preset exists to fix a real bug: under `garage_workshop` the parked vehicle rendered
    // near-black because the camera-facing flanks got ambient only. This locks the three intent
    // moves so a future retune can't quietly slide back into the silhouette look.
    let hero = SceneLighting::garage_hero();
    let workshop = SceneLighting::garage_workshop();

    // 1. Brighter hemispheric ambient than the plain workshop, so shaded flanks clear near-black —
    //    but still cooler/dimmer than the studio so the moody room survives.
    assert!(
        luminance(hero.ambient_rgb) > luminance(workshop.ambient_rgb),
        "hero ambient must out-lume the workshop: hero {:?} vs workshop {:?}",
        hero.ambient_rgb,
        workshop.ambient_rgb
    );
    assert!(
        luminance(hero.ambient_rgb) <= luminance(SceneLighting::garage_studio().ambient_rgb),
        "hero ambient should stay at or below the flat studio so the room keeps its mood"
    );
    // Hemisphere invariant: sky ambient still out-lumes the ground bounce.
    assert!(luminance(hero.ambient_rgb) > luminance(hero.ground_ambient_rgb));

    // 2. The key rakes the flanks instead of pouring straight down: a real horizontal component,
    //    unlike the near-vertical workshop key that only lit the decks.
    let hero_horiz = hero.key_direction[0].hypot(hero.key_direction[2]);
    let workshop_horiz = workshop.key_direction[0].hypot(workshop.key_direction[2]);
    assert!(
        hero_horiz > workshop_horiz,
        "hero key must rake more than the top-down workshop key: hero {hero_horiz} vs {workshop_horiz}"
    );

    // 3. The fill lifts the camera-facing flank, but under the readable-light doctrine
    //    (2026-08-05) it may only claim to be floor bounce: near-horizontal (raking, not
    //    top-down or out the back), alive, and a clear step UNDER the key — never a second
    //    sun the room cannot explain. The old studio fill out-lit the workshop's and the
    //    floor shafts read as coming from two suns at once.
    assert!(
        hero.fill_direction[0].abs() > hero.fill_direction[1].abs(),
        "hero fill must rake the flank horizontally, not point down: {:?}",
        hero.fill_direction
    );
    assert!(luminance(hero.fill_rgb) > 0.0, "the fill is alive — flanks must not fall to black");
    assert!(
        luminance(hero.fill_rgb) < luminance(hero.key_rgb) * 0.25,
        "the fill is bounce, not a second sun: fill {:?} vs key {:?}",
        hero.fill_rgb,
        hero.key_rgb
    );

    // 5. The rim is DEAD: a cool light from behind had no source in the room at all. The GI
    //    bake and the lamp pools separate the hull from the back wall now.
    assert_eq!(
        hero.rim_rgb,
        [0.0, 0.0, 0.0],
        "the sourceless rear rim must stay dead — readable light has no light from nowhere"
    );

    // 4. The grade is the MOODY-WORKSHOP formation now (Hala 3.0 B1, replacing the showroom
    //    grade this clause used to lock): neutral-or-below exposure, a real black point — but
    //    BOUNDED, because the 2026 lighting-2.0 regression is still the failure to prevent: a
    //    grade that re-sinks the flanks. The anti-silhouette duty this clause carried lives in
    //    the pixel-side locks now (subject p50/p05 floors and hero-over-room in look_goldens),
    //    which measure the frame instead of trusting the profile.
    assert!(
        (0.95..=1.10).contains(&hero.exposure),
        "hero exposure stays moody-neutral, got {}",
        hero.exposure
    );
    assert!(
        hero.black_point <= workshop.black_point,
        "hero blacks may not out-crush the moody workshop: {} vs {}",
        hero.black_point,
        workshop.black_point
    );
    assert!(
        (0.02..=0.032).contains(&hero.black_point),
        "hero black point is a real, bounded shadow floor, got {}",
        hero.black_point
    );
    assert!(
        hero.contrast <= workshop.contrast,
        "the hero grade must not out-crush the workshop's contrast"
    );
}

#[test]
fn fog_thickens_with_distance_and_thins_with_height() {
    let l = SceneLighting::battlefield_default();
    // No fog at the camera; a distant surface is heavily fogged; density is monotonic in distance.
    assert_eq!(l.fog_factor(0.0, 0.0), 0.0, "no fog at zero distance");
    let near = l.fog_factor(100.0, 0.0);
    let far = l.fog_factor(900.0, 0.0);
    assert!(far > near && near > 0.0, "fog thickens with distance: near={near} far={far}");
    assert!(far <= 1.0, "fog factor saturates at 1: {far}");
    // At the same distance, a surface higher up sits in thinner air and fogs less.
    let low = l.fog_factor(600.0, 0.0);
    let high = l.fog_factor(600.0, 60.0);
    assert!(high < low, "fog thins with height: low={low} high={high}");
    // Density 0 (interior) is fully clear regardless of distance.
    assert_eq!(SceneLighting::garage_studio().fog_factor(900.0, 0.0), 0.0);
}
