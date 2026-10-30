# CPU preparation and bounded scene views

Status: Proposed implementation candidate, not qualified or adopted.
Date: 2026-10-30.
Base: public source commit f7c5168e484adfd1ea89c95a11bb96c9721c2fd5.

The existing CPU painter is the production dispatcher for clip, shape, text and
image preparation. Expose `prepare_paint` and an opaque `PreparedPaint` with its
existing RenderOutput and actual CPU batch/vertex counts. Window and offscreen
rendering consume this same result. No Device, OS window or GPU loader is needed
to call it; font/image resources remain caller supplied. Counts describe CPU
prepared geometry, not completed GPU work or final atlas draw-call counts.

Add borrowed `SceneView { camera, scene, viewport, clip }`, at most 32 per frame,
and `render_with_scene_views`, `render_with_scene_views_and_target_mode` and
`capture_offscreen_with_scene_views`. `validate_scene_views` exposes the same
CPU admission used by these paths. Viewports/clips are physical pixels; a batch
viewport must fit within the target. A clip intersects that viewport without
changing its camera aspect or projection. Empty clips do not draw.

Visible clip rectangles must be pairwise disjoint. Viewport rectangles may
overlap when their clips do not. Touching boundaries are permitted. Overlap,
overflow/out-of-target viewports, non-finite derived camera uniforms and excess
view count return a structured scene-batch rejection before acquisition. This is
an explicit first batch policy, not a claim of arbitrary overlapping 3D layers.
Input rejection in the new presented entry points does not retire the renderer.
Supporting true overlapping layers later needs a separate depth/composition
contract; do not silently reuse another view's depth.

Prepare and retain every view's label descriptors/atlas ownership, camera and
clip until recording. A single scene-capable framebuffer clears color/depth
once. Draw all admitted views in input order using their independent viewports
and scissors. Disjoint scissors isolate the shared depth attachment. Transition
once to the existing label/UI overlay pass, record every prepared label set with
its own clip, then draw ordinary UI exactly once. The shared label policy also
includes that physical clip; anchors outside their own visible scene are omitted
before depth sampling, so a partially visible glyph cannot borrow another view's
depth. This explicitly tightens the old single-scene edge-anchor behavior. Keep one command buffer,
submission and present (or offscreen readback), along the existing recovery and
retirement owners. There are no per-pane renderers, swapchains or readback images.

Legacy optional single-scene methods keep their signatures and original
target-edge rectangle clamp, then enter the same normalization and recording
chain. Scene capability is inferred from requested content unless explicitly
retained by TargetMode; a UI-only target rejects actual scene content. Offscreen
scene capture checks this capacity before allocation instead of silently
dropping its scene.

Extend existing painter/image preparation and target-capacity comprehensive
contracts. Protect independent cameras/clips/order, borrowed scene identity,
empty/overflow/overlap/capacity handling, legacy clamping and true production
CPU geometry. Typecheck/tests/strict need a newly coordinated bounded Cargo slot.
No GUI, Device, native measurement or Vesta source/adoption is implied.
