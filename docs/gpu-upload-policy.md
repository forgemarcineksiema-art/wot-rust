# GPU Upload Policy

> **STATUS (2026-08-03): built, not wired.** The upload systems below exist only in
> `renderer_wgpu/src/{upload_buffers,texture_upload,readback_queue}.rs` and
> `tests/upload_system.rs`; no production frame flows through them. The shipping renderer uploads
> through `Queue::write_buffer` in `scene_renderer/{resources,draw,armor_damage}.rs` — the exact
> call the first rule below forbids. This document is the contract those paths adopt when the
> machinery is wired; it does not describe today's renderer.

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

This document defines the project-level contract the shipping renderer has not yet adopted: draw code consumes prepared batches; upload code prepares batches.
