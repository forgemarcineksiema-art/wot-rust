use renderer_api::RenderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadbackRequestId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadbackRequest {
    pub id: ReadbackRequestId,
    pub label: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadbackResult {
    pub id: ReadbackRequestId,
    pub label: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct GpuReadbackQueue {
    next_id: u64,
    pending: Vec<ReadbackRequest>,
    completed: Vec<ReadbackResult>,
}

impl GpuReadbackQueue {
    pub fn request(&mut self, label: impl Into<String>, byte_len: usize) -> ReadbackRequestId {
        let id = ReadbackRequestId(self.next_id);
        self.next_id += 1;
        self.pending.push(ReadbackRequest { id, label: label.into(), byte_len });
        id
    }

    pub fn pending_requests(&self) -> &[ReadbackRequest] {
        &self.pending
    }

    pub fn complete(&mut self, id: ReadbackRequestId, data: Vec<u8>) -> Result<(), RenderError> {
        let index = self
            .pending
            .iter()
            .position(|request| request.id == id)
            .ok_or_else(|| RenderError::new(format!("unknown readback request: {id:?}")))?;
        let request = self.pending.remove(index);
        self.completed.push(ReadbackResult { id, label: request.label, data });
        Ok(())
    }

    pub fn take_completed(&mut self, id: ReadbackRequestId) -> Option<ReadbackResult> {
        let index = self.completed.iter().position(|result| result.id == id)?;
        Some(self.completed.remove(index))
    }
}
