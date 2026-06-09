# GPU Upload Policy

GPU uploads are a renderer backend system, not scattered draw-call code. The renderer collects data by frame and resource class, uploads it in predictable batches, and draws with instancing.

## Upload Systems

- `FrameUploadArena`: per-frame staging for transient bytes that can be reset after submission.
- `DynamicUniformRingBuffer`: aligned dynamic uniform allocations, currently 256-byte friendly.
- `InstanceBufferAllocator`: grouped instance batches for tanks, foliage, props, particles, and track marks.
- `TextureUploadQueue`: queued texture uploads with explicit extent, format, mip count, and payload.
- `GpuReadbackQueue`: separates readback requests from completed CPU-visible results.

## Rules

- Do not call `Queue::write_buffer` once per tank, prop, particle, tree, or track mark.
- Collect tank instances, foliage, props, particles, and effects into batch uploads.
- Dynamic uniforms use ring-buffer style offsets, not ad hoc small buffers.
- Texture uploads are queued and drained by the backend.
- GPU readbacks are queued and completed asynchronously from renderer-owned code.
- Gameplay, simulation, networking, physics, and server crates never see upload arenas or `wgpu` handles.

## wgpu Notes

`Queue::write_buffer` is convenient for low-frequency or coarse uploads, but many small writes can create short-lived staging allocations. Real backend upload code should use `wgpu::util::StagingBelt`, persistent mapped staging buffers, or explicit upload buffers behind these renderer-owned queues.

This document defines the project-level contract before the full renderer exists: draw code consumes prepared batches; upload code prepares batches.
