# CPU render preparation and payload ownership

Core `vesta_gpui::render` prepares backend-neutral CPU data. The Vulkan package
owns GPU resources/submission; the application owns its Elements, image sources,
fonts, native target and frame lifetime. This module creates no window, renderer,
worker, author document or independent scene runtime.

Use the [family boundary](SUPPORTED-API.md), [Core contracts](../crates/vesta-gpui/SUPPORTED-API.md)
and [renderer contract](../crates/vesta-gpui-vulkan/SUPPORTED-API.md) for their
respective qualification limits. Exact current evidence is in
[the refinement ledger](REFINEMENT.md), not inferred from a `pub` declaration.

## Value, paint and prepared geometry

The Host owns `Element`/`Style` data. `Polyline` is the existing public,
nameable `{ points, width }` value used by `Style.polyline` and `Style::polyline`.
Points and filled-quad vertices are local to the element's layout origin.
`PaintList` derives window-coordinate `PaintCommand` copies from the same tree
and layout; CPU preparation derives temporary vertices from those commands.
None of these copies owns another author model or commits an application action.

Shape background precedence is filled quad, then polyline, then rounded rectangle;
the optional rectangular border still follows. Text/image content is independent
of that background choice. Rotation applies to rounded-rectangle fill/border
preparation; it does not rotate polyline/quad points, text, layout, hit or semantic
bounds. Hosts supply any transformed points themselves. A line drawn beyond a
tiny layout anchor still needs appropriate clipping and Host-owned hit policy.

| API | Result and preconditions |
| --- | --- |
| `ShapeVertex`, `TextureVertex`, `SHAPE_SHADER`, `TEXTURE_SHADER` | Typed CPU vertex records and matching repository shader strings. Positions are normalized device coordinates; shape local/size/radius/color and texture UV fields must match that family shader. They are not an independently versioned binary/plugin GPU ABI. |
| `shape_vertices` | Six vertices for a rounded/rotated rectangle. Finite valid geometry, color and viewport are required. Radius is bounded by half the smaller extent and negative border width is normalized. The existing flat-fill marker preserves full coverage at abutting unrotated, unstroked rectangular seams. |
| `valid_quad`, `quad_vertices` | Finite, strictly convex four-point perimeter in either winding. A crossed/concave/degenerate quad produces no mesh. A valid viewport and finite color remain caller preconditions; valid quads retain the supplied winding. |
| `polyline_vertices` | The connected stroke policy below. It additionally admits all input and output numerics before returning a complete finite mesh. |
| `texture_vertices`, `texture_vertices_uv` | Six vertices for a valid finite rectangle and viewport, using full or explicit top-left-origin UV corners. Image fitting/cropping is a separate value operation; no image source is loaded. |
| `ndc`, `rotate` | Direct logical-coordinate math; the caller supplies finite values and a valid viewport. Rotation uses radians about the supplied center. Neither operation changes hit or layout data. |
| `scissor` | Intersects a valid logical rectangle with the viewport, then floor/ceil clips to rounded physical extents. Empty/outside/invalid rectangle input has no scissor. The Host supplies a valid viewport whose scaled dimensions fit the actual u32 native target. It is a value calculation, not a native window query. |
| `linear_color`, `srgb_to_linear` | Finite straight-alpha sRGB values become clamped linear RGB and clamped straight alpha. This is not HDR color management or premultiplication. |

Raw constructors and fixed-array helpers do not certify a frame. Core layout
owns valid published logical geometry; the Host supplies a coherent current tree,
paint, text context, clips and native target. Direct callers keep the stated
preconditions and input budgets. These helpers do not offer global memory or
platform isolation for arbitrary caller-created payloads.

## Connected strokes

`polyline_vertices` retains its signature and the shared ShapeVertex stream.
Open paths have butt endpoints and bevel joins. Consecutive duplicate points
add no segment. An exact repeated first point closes the endpoint join; no new
cap/join setting, path model or curve-sampling policy is introduced.

At an ordinary bend the two bodies share the inner offset intersection and one
outer bevel triangle. The original outer hole is filled, and that local inner
join no longer receives duplicate coverage from the two segment bodies. f64
intermediates preserve width on long finite lines. If an inner intersection
would lie beyond an adjacent segment, the bounded segment ends remain and a
center-based outer wedge joins them. Short runs, reversals and self-intersecting
paths may therefore overlap. This is not a global path-union fill rule, SVG
implementation or guarantee of single coverage for every arbitrary path.

Preparation is deterministic O(n), with at most nine output vertices per
nonzero segment. It creates no retained cache or GPU state. The Host owns source
point-count/frame budgets. Nonfinite points/width/color, nonpositive width,
invalid viewport or unrepresentable output returns an empty mesh; no partial or
nonfinite vertex stream is published. Input payloads are borrowed and unchanged.
These are CPU topology contracts, not physical pixel or display-time claims.

## Images and static text

`ImageData` holds original width/height and RGBA bytes. `ImageResolver::resolve`
is the compatibility value-returning entry point; the default `resolve_state`
wraps a successful result in `Arc` or returns Missing. Hosts override
`resolve_state` to share immutable pixels through `ImageResolution::{Loading,
Ready, Missing, Failed}`. Loading, file/network I/O, retries and redraw scheduling
belong to the Host; a resolver called during preparation should perform bounded
lookup work. No loader or second resource authority is hidden in Core.

`ImageIssue`/`ImageDiagnostic` describe local pending/missing/failed/invalid/budget/
dimension fallbacks, retaining element/source identity and typed issue data.
`code()` supplies existing category codes; Host failure code/message remain
payloads. These are observations, not exceptions, approvals or a new asset API.
The renderer validates dimensions/byte lengths and applies its documented
resource limits. Image fitting owns geometry/UV, not loading policy.

`static_text_view` uses the supplied `TextInputContext` and the same cached
shaping source to return an immutable snapshot plus `TextInputViewport`, or None
on shaping failure. It accepts finite bounds/clip and valid text/font inputs.
Wrapping uses the supplied width; physical horizontal alignment and vertical
alignment/baseline placement follow the shared text contract. Logical RTL Start/
End conversion already belongs to the paint projection. It creates no editor,
undo history, platform IME object, font discovery or file I/O. The Host reconciles
any changed width/height before publishing consistent input/hit/text geometry.

`RenderOutput` contains frame-local draw/upload/reuse counts, image diagnostics
and text viewports. `record_bitmap` accounts for one optional upload/reuse result.
It is neither an image cache nor proof of GPU completion or physical display.
Hosts must keep CPU input geometry coherent through acquisition deferral and
publish presented observations only through their existing successful-submission
authority. See [the host recipe](HOSTING.md) and [architecture](ARCHITECTURE.md).

## Current use and compatibility

Shared controls/chrome/tree/navigation and Motion consume the same polyline
projection; Agent composer icons and Builder graph wires remain product payloads.
Vulkan calls the one shared vertex function. Re-exporting `Polyline` makes the
already public Style field nameable without adding a parallel constructor.
The visible bend correction and explicit short/self-overlap limits are recorded
in [decision 0021](decisions/0021-Core-Render-Payloads-Strokes-And-Owner-Updates.md).

The complete Core public family audit, other native/renderer resource reviews,
remote CI and immutable product adoption remain separate programme exits.
No new physical-input, reader, mixed-monitor or GPU-display claim follows from
the pure CPU preparation contract.
