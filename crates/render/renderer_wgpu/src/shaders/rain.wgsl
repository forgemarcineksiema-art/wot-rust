// Stateless GPU rain: every streak's position is a pure function of (instance index, the
// tick-domain presentation clock, the camera position) — no CPU particle state, no buffers.
// Streaks live in a cylinder around the camera and wrap vertically as they fall; each is a
// thin camera-lateral quad slanted by the wind. Drawn after the world (depth-tested, no
// write) so rain never falls through a roof in view, additively faint.
// Composed after camera_common.wgsl (the shared camera uniform declaration).

const CYLINDER_RADIUS_M: f32 = 26.0;
const HALF_HEIGHT_M: f32 = 14.0;
const FALL_SPEED_MPS: f32 = 16.0;
const STREAK_LENGTH_M: f32 = 0.55;
const STREAK_WIDTH_M: f32 = 0.014;
// The wind every streak leans into (normalized in the shader).
const WIND: vec3<f32> = vec3<f32>(0.22, -1.0, 0.1);

fn hash11(seed: f32) -> f32 {
    return fract(sin(seed * 127.1) * 43758.5453);
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) alpha: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> VsOut {
    let seed = f32(instance);
    let h1 = hash11(seed + 0.13);
    let h2 = hash11(seed + 7.77);
    let h3 = hash11(seed + 31.3);
    let time = camera.time_params.x + camera.weather_params.w;

    // Cylinder placement around the camera, denser near the lens (sqrt keeps it uniform by
    // area; the near bias comes from perspective alone).
    let radius = sqrt(h1) * CYLINDER_RADIUS_M + 0.6;
    let angle = h2 * 6.2831853;
    let fall = normalize(WIND);
    // Vertical wrap: the streak falls FALL_SPEED and re-enters the cylinder top.
    let cycle = (HALF_HEIGHT_M * 2.0) / FALL_SPEED_MPS;
    let phase = fract(h3 + time / cycle);
    let drop = vec3<f32>(
        camera.camera_pos.x + cos(angle) * radius,
        camera.camera_pos.y + HALF_HEIGHT_M - phase * HALF_HEIGHT_M * 2.0,
        camera.camera_pos.z + sin(angle) * radius,
    );

    // Camera-lateral thin quad along the (wind-slanted) fall direction.
    let to_eye = drop - camera.camera_pos;
    let lateral = normalize(cross(fall, to_eye));
    let along = fall * STREAK_LENGTH_M;
    let side = lateral * STREAK_WIDTH_M;
    // Six vertices, two triangles: (-s,-a) (s,-a) (s,a) / (-s,-a) (s,a) (-s,a).
    var corner: vec3<f32>;
    switch vertex {
        case 0u: { corner = drop - side - along; }
        case 1u: { corner = drop + side - along; }
        case 2u: { corner = drop + side + along; }
        case 3u: { corner = drop - side - along; }
        case 4u: { corner = drop + side + along; }
        default: { corner = drop - side + along; }
    }

    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(corner, 1.0);
    // Fade with distance so the cylinder wall never reads as a curtain edge.
    out.alpha = (1.0 - radius / (CYLINDER_RADIUS_M + 1.0)) * 0.35 + 0.05;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    // Premultiplied faint grey-blue; alpha 0 keeps it purely additive-ish and soft.
    let tone = vec3<f32>(0.55, 0.60, 0.68) * input.alpha * 0.5;
    return vec4<f32>(tone, input.alpha * 0.25);
}
