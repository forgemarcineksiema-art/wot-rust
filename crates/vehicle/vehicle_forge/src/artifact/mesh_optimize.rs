pub(super) fn optimize_indices_for_gpu(indices: &[u32], vertex_count: usize) -> Vec<u32> {
    if indices.is_empty() {
        return Vec::new();
    }
    meshopt::optimize_vertex_cache(indices, vertex_count)
}
