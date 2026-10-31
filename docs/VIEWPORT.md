# Viewport data, projection and interaction ownership

Scope: the existing experimental `vesta-viewport` family. The
[refinement ledger](REFINEMENT.md) records qualification; source/API review is
not physical input, GPU pixel, platform or product-adoption evidence.

## Ownership and dependency direction

| Module | Owns | Receives from its caller |
| --- | --- | --- |
| `lib` | Backend-neutral scene/mesh/identity data, transient hit table, CPU semantic picking and one-axis translation intent | Stable identities, immutable display geometry, camera/query coordinates and Host transform policy |
| Private `camera` | Camera/Projection state, camera actions, one projection policy, matrices/uniform packing, point projection and rays | Camera values, aspect and logical viewport coordinates |
| `render` | Mesh/object packing, world-space grid/selection/tool geometry and complete finite primitive admission | Caller-owned scene, optional material/color rows and explicit lighting |
| `display` | Derived wire/point/normal overlays and their display-only switches | Host-projected world-space sources and optional styles |
| `display/markers`, `display/labels` | Constant-screen-size marker projection and numeric annotation data | Physical viewport, explicit style, immutable numeric atlas and exact scalar labels |
| `lighting` and three embedded WGSL files | Explicit lighting data and shared shader/layout contracts | Valid supplied geometry/material/camera resources; backend pipeline construction remains outside this crate |

The family has no native-window, event-loop, GPU-device, application or Core
authoring dependency. Root exports remain the public data/interaction facade.
`render::view_projection` and `render::camera_uniform` re-export the camera owner;
they do not implement another matrix policy. Native Vulkan consumes these
outputs through [its existing renderer](VULKAN.md). Other backend adapters may
consume the same neutral scene and shader inputs.

## Data and stable identities

`Vec3`, `Vertex`, `Mesh`, `Renderable` and `ViewportScene` are display data, not
author documents, execution results or GPU resources. The Host owns source I/O,
world transforms, finite f32 geometry and stable unique Renderable IDs within
each scene. The model currently carries translation, not general object rotation
or scale. Camera/display/selection state must not silently enter a Core cache key.
The Host separately owns rebasing/precision and input-complexity/memory budgets.

`ViewportScene::select` updates only derived preview selection. `SurfaceMaterial`,
per-object RGB rows and display sources remain caller-owned. Missing RGB rows
preserve object color; supplied rows follow Mesh vertex order, not author element
identity. The Host performs stable source-to-display mapping before this boundary.

`HitProxyNamespace` and `HitProxyKey` preserve Host meaning. CPU renderable hits
derive their key from RenderableId; custom invisible proxies carry the supplied
key. `HitProxyDepth::Overlay` gives admitted tool regions precedence over scene
surfaces. The Host reserves namespaces and interprets subparts; no product node,
tool or author-command rules belong here.

`HitProxyTable` maps compact nonzero IDs to semantic keys for one Host revision.
`begin_frame` clears entries; the Host must never reuse a revision for a different
live frame/viewport identity. `resolve` rejects a different revision or removed
entry. This is indirection, not an implemented GPU picking/readback service.
`PickRequest` / `PickResult` are neutral request data. The Host owns request-ID /
viewport/revision correlation, asynchronous lifetime and rejection of stale work.
Table-capacity failure is the existing Display/Error-compatible
`HitProxyTableFull`; it does not change the current entries.

## Camera and projection

`Camera` owns target, yaw/pitch/roll, distance, field of view, near/far and
Perspective/Orthographic choice. Orbit/dolly/pan and `apply_action` operate on
caller-owned transient camera state; raw camera/math helpers require valid finite
inputs. `ResetCamera` restores Camera::default. Scene-dependent framing is Host
policy and is not implemented by a reset action.

One camera module supplies the renderer's original projection conventions:
aspect at least 0.001; orthographic height at least 0.01 and width at least 0.001;
existing orthographic depth-span clamp; the original six-vec4 camera block.
Rolled screen axes and parallel orthographic sight lines remain shared. These
numeric floors do not establish a universal camera validation or precision model.

`project_to_pixel` uses that same matrix and Vulkan depth interval. It returns
None for invalid dimensions, clipped depth or unrepresentable output. XY can lie
outside the viewport; callers still apply current layout clipping. `pick_ray`
retains its raw tuple signature and requires valid finite camera/aspect/NDC.
Perspective starts at the eye; orthographic starts at the projected parallel
offset. Neither helper owns an event loop, layout origin or physical DPI policy.

`pick` and `pick_target` validate positive finite aspect and finite NDC within
[-1,1], then prepare their ray and matrix once. Triangle depth is clipped before
choosing a mesh's nearest hit, so a near-clipped face cannot hide a deeper valid
face in that same mesh. Intersection intermediates use f64 and a scale-relative
parallel/degenerate threshold; returned distances remain finite positive f32.
Queries retain two-sided triangle semantics and caller input order for ties;
they are semantic reference queries, not a promise to reproduce every raster
culling/alpha/overlay pixel. Invalid indices/trailing incomplete triangles cannot
create hits. Backends may accelerate the same identity contract.

## Gesture and display geometry

`TranslationDrag` stages a stable target/part/space and original display
translation. It hit-tests visible, projected X/Y/Z translation segments and
returns Begin/Update intent; it never writes a Host document. One private Gizmo
axis length supplies drawn segments, hit geometry and world-delta scaling.
The current translated-only scene uses world-aligned axes; space metadata is
carried to the Host. Plane/rotation/scale author gestures are not implemented by
this one-axis helper. The Host owns capture, cancel/commit, stale-target rejection,
Undo/Redo and conversion to its authoritative typed author commands. Update
coordinates must be finite and remain within the Host's interaction budget.

Display switches describe preview only. `grid_lines=None` retains the generic
grid; Some(empty) deliberately supplies no grid. An override requires complete
finite line pairs and returns an error otherwise. Wire/point/normal source
overrides preserve their existing fallback/Some(empty) distinctions.

Legacy Vec-returning overlay helpers omit invalid complete primitives while
preserving valid order: line pairs, nine-vertex normal arrows and six-vertex
marker billboards. Expansion overflow cannot leave half a primitive or invalid
vertex grouping. Line/arrow buffers compact in place; markers truncate their
current rejected billboard. No additional allocator, retained model or worker is
created. These checks do not bound caller source complexity or perform I/O.

Marker style is in physical pixels; normal ribbons use supplied world dimensions
and explicit reference distance. Numeric labels carry exact u32 scalars and Host
world positions, separate from author IDs. The optional immutable ten-cell RGBA
atlas comes from the Host. Missing atlas means no labels; font discovery/shaping,
resource availability and backend atlas/descriptors/depth lifetime are outside
this leaf. The renderer admits actual style, capacity and atlas dimensions.

## Shader payload boundary

`mesh_vertices` returns the nine-f32 position/normal/RGB layout and checks finite
positions/normals and supplied color-row cardinality/values. `object_uniform`
returns the seven-vec4 block and checks finite translation/color/range plus the
shared explicit lighting contract. Finite HDR color, zero normals and finite
flat/inverted terrain ranges retain their previous input semantics. The Host
still supplies geometry appropriate for lighting and representable world ranges;
this is not a general geometry repair or normalization service.

These low-level helpers retain their existing Result/`&'static str` signatures.
The Vulkan adapter projects failure to RendererError at `viewport_vertices` or
`viewport_material`; it owns the structured backend failure and resource policy.
Numeric-label anchors must have positive clip.w and depth within [0, clip.w]
before their overlay output is moved to depth zero. Occlusion tolerance does not
extend camera near/far clipping. The WGSL camera/object/vertex/numeric layouts
remain shared data contracts;
shader compilation, checked draw, depth/MSAA and GPU ownership remain backend-
owned. A compiled shader alone does not establish correct pixels.

## Pre-release migration and current qualification

- Root Camera/Projection/action/projection/ray paths and render camera paths are
  preserved by re-export after ownership moves.
- `ViewportAction::FrameAll` becomes `ResetCamera`, matching its actual reset
  behavior. There is no misleading alias or automatic scene-fit implementation.
  Standalone's existing action identity and shortcut retain their effect with
  the corrected presentation; Builder/Facial's real Host framing stays separate.
- Remove `render::RenderStats`, which has no producer, consumer or supported
  contract. No replacement 3D count service is claimed; actual renderer
  observations retain their separately documented scope.
- Packing failure stage `viewport_vertex_colors` becomes `viewport_vertices`.
  Code/fault/signature are unchanged; stage-matching consumers must migrate.

[Decision 0028](decisions/0028-Viewport-Projection-And-Geometry-Boundaries.md)
and `target/research/viewport-family-20261031/` retain the original CPU/API
failures and native01's actual near/far label pixel defect. Final full02 passes
236 contracts/39 harnesses, CPU smoke, two compile-fail docs, all/base strict,
seven Node groups, format and both ordinary standard builds with unchanged inputs.
On that exact final executable, native03 completes eight actual PNG readbacks:
each projection retains the 144-pixel visible control and all four outside-depth
controls match the no-atlas baseline. The immutable synthetic atlas verifies
visibility, not font legibility. Reflow02 separately qualifies Main64/Float64
submission and two public offscreen readbacks after private Reflow/Viewport
workload separation. Both hidden/nonactive workloads exit normally with zero OS
input. The maintained scene verifier checks source/process/phase/PNG identities;
actual-report corruption checks and its CI contract reject false evidence.

This closes the local Viewport source-family review. Physical/reader/DPI/display
guarantees, remaining programme gates and current immutable Vesta adoption keep
their separate exits. Full01's existing trace unit count failure and red04's
deterministic reproduction remain failed records; its corrected composite test
does not change recorder production semantics.
