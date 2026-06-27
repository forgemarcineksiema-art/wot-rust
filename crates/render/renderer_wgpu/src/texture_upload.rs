#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureExtent {
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8UnormSrgb,
    Bc7RgbaUnormSrgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureUploadId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureUpload {
    pub id: TextureUploadId,
    pub label: String,
    pub extent: TextureExtent,
    pub format: TextureFormat,
    pub mip_level_count: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct TextureUploadQueue {
    next_id: u64,
    uploads: Vec<TextureUpload>,
}

impl TextureUploadQueue {
    pub fn enqueue_2d(
        &mut self,
        label: impl Into<String>,
        extent: TextureExtent,
        format: TextureFormat,
        mip_level_count: u32,
        data: Vec<u8>,
    ) -> TextureUploadId {
        assert!(extent.width > 0 && extent.height > 0);
        assert!(mip_level_count > 0);
        let id = TextureUploadId(self.next_id);
        self.next_id += 1;
        self.uploads.push(TextureUpload {
            id,
            label: label.into(),
            extent,
            format,
            mip_level_count,
            data,
        });
        id
    }

    pub fn pending_uploads(&self) -> &[TextureUpload] {
        &self.uploads
    }
}
