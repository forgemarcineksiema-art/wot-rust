# Battle HUD region map

The in-battle HUD is one flat `Vec<HudVertex>` in clip space (`[-1, 1]`, +y up),
assembled by `hud::build_battle_hud(&BattleHudModel, aspect)` and drawn in a
single pass over the 3D scene. Every element owns a fixed screen region so they
never collide. Visual language is the "military instrument" art direction
(`hud/theme.rs`): graphite panels, amber accent, warm off-white stencil
readouts. A staged reference frame is produced by
`cargo run -p client --example battle_hud` (both reticle modes).

## Regions (clip space)

| Element | Anchor | Where | Source |
| --- | --- | --- | --- |
| Reticle (marker, ring, gun marker, reload arc, blocked, impact X, distance, mm) | screen centre `[0, 0]` | centre | `hud/reticle_overlay.rs`, `hud/reticle_marks.rs`, `hud/reticle_readouts.rs` |
| Self HP bar + number | top-left | `x ≈ -0.95..-0.61`, `y ≈ 0.9` | `hud.rs` |
| Reload bar + seconds | bottom-centre | `x ≈ -0.16..0.16`, `y ≈ -0.9` | `hud.rs` |
| Ammo panel (3 slots) | bottom-centre-right, right of the reload bar | `x ≈ 0.27..0.48`, `y ≈ -0.885` | `hud/ammo_panel.rs` |
| Sniper zoom "X6.9" | just under the reticle | `[-0.03, -0.16]` | `hud.rs` |
| FPS | top-right | `[0.97, 0.97]` | `hud.rs` |
| Speed + KM/H | bottom-left | `[-0.78, -0.76]` | `hud.rs` |
| Damage log (dealt / taken rows) | left edge, mid-low | `x ≈ -0.97`, `y ≈ -0.25` downward | `hud/damage_log.rs` |
| Incoming-hit direction arcs | ring around centre | radius `0.30` at attacker bearing | `hud/hit_direction.rs` |
| Enemy floating HP bars | world-anchored over live enemies | projected | `hud/health_bar.rs` |
| Outgoing damage numbers | world-anchored at the hit point | projected | `hit_indicator.rs` |

## Reticle honesty (hybrid)

Which armor information the reticle shows depends on the camera mode — see
`docs/aiming-model-policy.md` for the full matrix. Third person is neutral
(central marker + fading gun marker + dispersion ring, no armor talk); sniper
mode adds the pen verdict by color, the pen/armor millimetres, and the
real-impact X. The BLOCKED form, distance and reload arc draw in both.

## Colors

Feature tints alias `hud::theme::color` tokens, each carrying a unique alpha
tag because the HUD tests identify features by exact vertex-color equality.
Semantic combat colors (penetration green / no-pen red, health ramp, signal
red) live with their features, not the theme.

## Budget

The HUD buffer holds 8192 vertices (`renderer_wgpu` `HUD_VERTEX_CAPACITY`);
`set_hud` truncates to whole triangles with a warning rather than blanking the
frame if a regression ever exceeds it.
