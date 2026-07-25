// Depth-only occluder pass for the focused sun shadow map. Renders the whole world (static terrain +
// baked buildings/trees, the dynamic mesh, and the vehicle fleet) from the sun's point of view into
// a depth texture; the scene and vehicle shaders sample it to shade the key light. Only the position
// and the per-instance model matrix are read — no colour, no fragment — so one vs_main serves both
// the scene and vehicle vertex formats (each leads with position).
// Composed after camera_common.wgsl (the shared camera uniform declaration).

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(6) model_0: vec4<f32>,
    @location(7) model_1: vec4<f32>,
    @location(8) model_2: vec4<f32>,
    @location(9) model_3: vec4<f32>,
    @location(13) damage_index: u32,
};

struct VsDepthOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) @interpolate(flat) damage_index: u32,
};

fn depth_out(clip: vec4<f32>, world_pos: vec3<f32>, damage_index: u32) -> VsDepthOut {
    var out: VsDepthOut;
    out.clip = clip;
    out.world_pos = world_pos;
    out.damage_index = damage_index;
    return out;
}

@vertex
fn vs_main(input: VsIn) -> VsDepthOut {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return depth_out(camera.light_view_proj * world, world.xyz, input.damage_index);
}

// The far-cascade occluder pass: same inputs, transformed by the far cascade's light matrix.
// Only terrain and static/dynamic scene geometry run through it — at the far map's ~0.56 m
// texels a vehicle's shadow does not resolve, so the fleet stays out of this pass.
@vertex
fn vs_far(input: VsIn) -> VsDepthOut {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return depth_out(camera.light_view_proj_far * world, world.xyz, input.damage_index);
}

// Camera depth prepass for SSAO: same inputs, but transformed by the CAMERA view-projection into
// the screen-sized depth texture the SSAO pass reads.
@vertex
fn vs_prepass(input: VsIn) -> VsDepthOut {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return depth_out(camera.view_proj * world, world.xyz, input.damage_index);
}

@fragment
fn fs_depth(input: VsDepthOut) {
    if (armor_fragment_is_cut(input.world_pos, input.damage_index)) {
        discard;
    }
}

// --- The cutout depth path (Imported Flora 2.0, FL-2) --------------------------------------
// SCENE casters carry the UV lane, and their depth fragments sample the foliage atlas: texels
// under the cutout threshold cast NO depth — a leaf's shadow (and its AO) is exactly its
// mask. Procedural content carries uv (0, 0), where the atlas is opaque white, so it costs
// one cached texel fetch and never discards.

@group(1) @binding(0) var foliage_atlas: texture_2d<f32>;
@group(1) @binding(1) var foliage_sampler: sampler;

struct VsInCutout {
    @location(0) position: vec3<f32>,
    @location(12) uv: vec2<f32>,
    @location(6) model_0: vec4<f32>,
    @location(7) model_1: vec4<f32>,
    @location(8) model_2: vec4<f32>,
    @location(9) model_3: vec4<f32>,
    @location(13) damage_index: u32,
};

struct VsDepthOutCutout {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) @interpolate(flat) damage_index: u32,
    @location(2) uv: vec2<f32>,
};

fn depth_out_cutout(clip: vec4<f32>, world_pos: vec3<f32>, damage_index: u32, uv: vec2<f32>) -> VsDepthOutCutout {
    var out: VsDepthOutCutout;
    out.clip = clip;
    out.world_pos = world_pos;
    out.damage_index = damage_index;
    out.uv = uv;
    return out;
}

@vertex
fn vs_main_cutout(input: VsInCutout) -> VsDepthOutCutout {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return depth_out_cutout(camera.light_view_proj * world, world.xyz, input.damage_index, input.uv);
}

@vertex
fn vs_far_cutout(input: VsInCutout) -> VsDepthOutCutout {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return depth_out_cutout(camera.light_view_proj_far * world, world.xyz, input.damage_index, input.uv);
}

@vertex
fn vs_prepass_cutout(input: VsInCutout) -> VsDepthOutCutout {
    let model = mat4x4<f32>(input.model_0, input.model_1, input.model_2, input.model_3);
    let world = model * vec4<f32>(input.position, 1.0);
    return depth_out_cutout(camera.view_proj * world, world.xyz, input.damage_index, input.uv);
}

@fragment
fn fs_depth_cutout(input: VsDepthOutCutout) {
    if (armor_fragment_is_cut(input.world_pos, input.damage_index)) {
        discard;
    }
    if (textureSample(foliage_atlas, foliage_sampler, input.uv).a < 0.5) {
        discard;
    }
}
