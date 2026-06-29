use vehicle_geometry::MaterialRole;

pub(crate) const BARREL_STEEL: [f32; 3] = [0.15, 0.16, 0.14];
pub(crate) const TRACK_METAL: [f32; 3] = [0.16, 0.16, 0.175];
pub(crate) const RUBBER: [f32; 3] = [0.07, 0.07, 0.075];

pub(crate) fn shade_color(color: [f32; 3], shade: f32) -> [f32; 3] {
    [color[0] * shade, color[1] * shade, color[2] * shade]
}

pub(crate) fn lighten(color: [f32; 3], amount: f32) -> [f32; 3] {
    [(color[0] + amount).min(1.0), (color[1] + amount).min(1.0), (color[2] + amount).min(1.0)]
}

pub(crate) fn material_color(material: MaterialRole, hull_color: [f32; 3]) -> [f32; 3] {
    match material {
        MaterialRole::RolledArmor => hull_color,
        MaterialRole::CastArmor => lighten(hull_color, 0.06),
        MaterialRole::BarrelSteel => BARREL_STEEL,
        MaterialRole::TrackMetal => TRACK_METAL,
        MaterialRole::Rubber => RUBBER,
    }
}
