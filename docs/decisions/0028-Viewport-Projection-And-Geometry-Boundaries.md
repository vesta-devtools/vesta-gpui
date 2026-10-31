# Viewport projection and geometry boundaries

Status: Accepted; local source-family, software and affected native qualification complete.
Date: 2026-10-31.

Continue the final complete GF02 source-family review. Viewport owns backend-
neutral scene/display data and CPU camera/interaction math, never native windows,
GPU lifetimes, product author state or a second execution runtime.

## Findings to reproduce

- CPU project-to-pixel and pick-ray code duplicate renderer projection policy.
  The minimum aspect and orthographic width differ for narrow viewports.
- CPU triangle queries ignore camera near/far clipping and use a fixed absolute
  determinant threshold that can reject small but otherwise valid triangles.
- Result-returning mesh/object packing accepts non-finite geometry/translation /
  color/range. Display expansion can also emit non-finite complete primitives
  after otherwise finite arithmetic overflows.
- HitProxyTableFull lacks the standard error contract used by external hosts.

These are source leads until the corresponding original reds are retained.
Red01 preserves a closure type-inference error in the fixture. After its explicit
f32 annotation, original red02 records four runtime failures covering all of the
projection, clipping/small-triangle, payload and expanded-primitive findings.
Red03 separately records the public capacity error's missing standard Error trait.
Original native01 also reproduces numeric-label shader depth leakage: near/far
anchors outside the camera interval produce the same 144 changed pixels as the
visible control in both projections. All eight original PNGs, decoded hashes,
trace, source/artifact/process identities and normal exit are retained.

## Decision

One private camera module owns Camera/Projection, interaction camera actions,
projection matrices, CPU projection and rays. Preserve all existing root and
render-module public paths by re-export, not forwarding implementations. Preserve
the existing GPU projection clamps and uniform layout. CPU queries use that same
projection and reject unrepresentable output. A point projection may still lie
outside viewport XY; near/far depth and finite representability are enforced.

Prepare one private ray/projection per CPU pick and test each triangle against
the actual depth clip before reducing hits. Preserve two-sided triangle semantics
and overlay precedence among admitted hits. Use f64 intersection intermediates
and a scale-relative parallel/degenerate test, retaining the positive finite f32
distance contract. Invalid aspect/NDC queries cannot create semantic hits.

Keep raw scene/display types caller-owned. Existing fallible mesh/object packing
rejects non-finite shader input without adding an independent renderer validator.
Legacy Vec-returning display helpers omit invalid whole primitives while
preserving valid order and complete line/triangle groups; compact their current
buffers without a new allocator, service or retained state. Preserve finite HDR
color values, zero normals, flat/inverted finite terrain ranges and existing
optional display resources. Add Display/Error interoperability to the existing
table-capacity error without changing its identity or signature.
The renderer's packing failure stage becomes `viewport_vertices` rather than
`viewport_vertex_colors`, reflecting the complete input it now admits. Its code,
fault classification and Rust signature stay unchanged; consumers matching the
old stage string must accept the corrected stage with this SDK adoption.

The complete source/caller search also finds two unsupported legacy surfaces.
`ViewportAction::FrameAll` receives no scene and only resets Camera::default;
rename it to `ResetCamera`, preserving that actual effect and updating the
Standalone action/catalog/label. Content-dependent framing remains Host policy,
as in Builder/Facial. Remove `render::RenderStats`: it has no producer, consumer
or supported contract, and is not the renderer's actual frame observation.
Do not retain a misleading alias or invent a replacement count service. These
pre-release source changes require explicit migration notes and common adoption;
they do not change Root's independently owned real Frame All actions.
The visual axis length, hit segment and returned drag scale also use one private
Gizmo constant. The existing drag test now obtains the actual drawn X segment
from overlay output, so a future drawing/interaction divergence is observable.

## Qualification and limits

Stable projection/picking/identity tests are extended; two composite shader-
payload/primitive contracts protect the CPU/backend boundary. All seven Rust
modules, three embedded WGSL files and actual SDK/Root callers have an explicit
ownership and failure map in the Viewport contract. No dependency, public
service, renderer, author model, feature application or target is added.

The existing private probe has one common start/completion lifetime and an
explicit Reflow/Viewport workload enum. The Viewport case creates only its
hidden/nonactive Main, prepares owned scene inputs after ordinary public capture
admission and advances phase only when the renderer returns a ticket. Two later
successful Main frames and a 500 ms drain grace precede normal exit under the
unchanged 100-second bound. The existing 128-step cases retain their verifier.

After preserving native01's original pixel failure, the shader is corrected to
admit numeric-label anchors only when clip.w is positive and clip.z is within
[0, clip.w] before moving their overlay output to depth zero. Occlusion tolerance,
pixel placement, atlas/layout and resource lifetime remain unchanged. The
synthetic atlas tests visibility only, not glyph legibility. The maintained scene
verifier decodes bounded PNGs and checks pixels, source/artifact/process/phase
identity and later successful frame submission. Nine actual-report checks and
one composite CI contract reject substituted or corrupt evidence.

Final full02 passes 236 contracts/39 harnesses, CPU smoke, two compile-fail docs,
all/base strict, seven Node groups, format and both ordinary standard builds with
unchanged inputs. Native03 on that final ordinary executable retains the visible
control and clips all four outside-depth controls across eight actual readbacks.
Reflow02 separately passes Main64/Float64 submission and two public captures.
Both workloads use zero OS input and exit normally. Their exact identities,
retirement and executable-lock release are retained in the final checkpoint.
The local family is closed. Existing physical/input/reader/DPI evidence is
preserved; remaining programme and immutable product adoption exits stay open.

Full01 also preserves a pre-existing native trace unit failure: its test assumes
the process-wide capture contains exactly one event while other instrumented
unit work runs concurrently. First retain actual stats with a deterministic
second legitimate recording thread, then qualify the owned frame names in the
frozen export rather than a global exact count. No recorder or transport policy
changes. Keep this in the existing composite lifecycle/export test; use owned
test artifacts under the workspace default target instead of system Temp.
Red04 records exactly two legitimate frame events against the old exact-one
assertion. The corrected composite test retains both named frames in the frozen
report, checks active stats without excluding other threads, preserves the
successor and racing-file guarantees, and joins its workers. Focused05 passes
all 12 native unit contracts and strict checks. The production prefix of
ui_trace.rs is unchanged; full01 and red04 are not reclassified as green.
