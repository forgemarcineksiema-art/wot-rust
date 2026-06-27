use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadBatch {
    pub label: String,
    pub offset: usize,
    pub byte_len: usize,
}

#[derive(Debug, Clone)]
pub struct FrameUploadArena {
    capacity: usize,
    bytes: Vec<u8>,
    batches: Vec<UploadBatch>,
}

impl FrameUploadArena {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, bytes: Vec::with_capacity(capacity), batches: Vec::new() }
    }

    pub fn stage(&mut self, label: impl Into<String>, data: &[u8]) -> UploadBatch {
        assert!(self.pending_bytes() + data.len() <= self.capacity);
        let batch =
            UploadBatch { label: label.into(), offset: self.bytes.len(), byte_len: data.len() };
        self.bytes.extend_from_slice(data);
        self.batches.push(batch.clone());
        batch
    }

    pub fn pending_batches(&self) -> &[UploadBatch] {
        &self.batches
    }

    pub fn pending_bytes(&self) -> usize {
        self.bytes.len()
    }

    pub fn reset_frame(&mut self) {
        self.bytes.clear();
        self.batches.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicUniformAllocation {
    pub offset: usize,
    pub size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicUniformRingBuffer {
    capacity: usize,
    alignment: usize,
    cursor: usize,
}

impl DynamicUniformRingBuffer {
    pub fn new(capacity: usize, alignment: usize) -> Self {
        assert!(alignment.is_power_of_two());
        Self { capacity, alignment, cursor: 0 }
    }

    pub fn allocate(&mut self, size: usize) -> Option<DynamicUniformAllocation> {
        let offset = align_up(self.cursor, self.alignment);
        (offset + size <= self.capacity).then(|| {
            self.cursor = offset + size;
            DynamicUniformAllocation { offset, size }
        })
    }

    pub fn begin_frame(&mut self) {
        self.cursor = 0;
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstanceBatchKind {
    Tanks,
    Foliage,
    Props,
    Particles,
    TrackMarks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceBatch {
    pub kind: InstanceBatchKind,
    pub instance_count: usize,
    pub byte_len: usize,
}

#[derive(Debug, Default, Clone)]
pub struct InstanceBufferAllocator {
    batches: BTreeMap<InstanceBatchKind, InstanceBatch>,
}

impl InstanceBufferAllocator {
    pub fn push_instances(&mut self, kind: InstanceBatchKind, count: usize, stride: usize) {
        let batch = self.batches.entry(kind).or_insert(InstanceBatch {
            kind,
            instance_count: 0,
            byte_len: 0,
        });
        batch.instance_count += count;
        batch.byte_len += count * stride;
    }

    pub fn batch(&self, kind: InstanceBatchKind) -> Option<&InstanceBatch> {
        self.batches.get(&kind)
    }

    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }
}
