pub struct SceneRenderTarget<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub resolve_target: Option<&'a wgpu::TextureView>,
    pub depth_view: &'a wgpu::TextureView,
    pub sample_count: u32,
}

pub(crate) fn store_op_for_target(resolve_target: Option<&wgpu::TextureView>) -> wgpu::StoreOp {
    if resolve_target.is_some() { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store }
}
