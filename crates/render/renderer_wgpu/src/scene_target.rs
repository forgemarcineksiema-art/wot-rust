/// Where a rendered frame goes, and how big it is.
///
/// Three fields, and every one of them is something only the caller can know. It used to carry the
/// scene's depth buffer, an MSAA colour attachment and a sample count as well — so a window and an
/// offscreen capture each built their own depth texture, and a runtime guard compared the caller's
/// sample count against the renderer's because nothing else could stop them drifting apart.
///
/// Depth now lives beside the colour it is written with, inside the renderer. The guard is gone
/// because there is no longer a second opinion to disagree with.
pub struct SceneRenderTarget<'a> {
    /// The texture the finished picture is written to.
    pub output_view: &'a wgpu::TextureView,
    /// Pixel size of the target — sizes the HDR chain, the depth buffer and the SSAO chain.
    pub width: u32,
    pub height: u32,
}
