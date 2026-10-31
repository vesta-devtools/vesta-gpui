# Native adapter ownership and admission

The finite Native family review covers the current 17 production modules and
actual Standalone, Agent and Builder consumers. Public exports remain defined
by [`lib.rs`](../crates/vesta-gpui-winit/src/lib.rs), not a second catalogue.
This contract complements [structured UI access](AGENT-ACCESS.md),
[the native recipe](HOSTING.md), [tracing](TRACING.md) and
[decision 0025](decisions/0025-Native-Capture-Admission-And-Platform-Ownership.md).
The [programme](REFINEMENT.md) records qualification and remaining exits.

## Responsibilities

| Responsibility | Single implementation owner | Host boundary |
| --- | --- | --- |
| Text/key/native adaptation | lib, window_text, ime_request, text_pointer, focus_visibility | Register windows; own Core editors/context/session, current focus/capture, composition transitions, clocks and injected clipboard. Native bridge has no author model or event loop. |
| Logical command/observation wire data | ui_input, ui_input_command, ui_input_observation, ui_json | Own transport envelope, current committed root/route/layout, source revision pinning, permissions and dispatch. Normalized events are application input, never OS injection. |
| Capture geometry/redaction | ui_capture | Authorize source identity, current complete snapshot and renderer/target; own GPU allocation, completion, PNG encoding and output. |
| Stale-context precondition | ui_context | Observe real project/session transitions and invalidate leases. The optional generation check is not author permission or an AuthorLease. |
| Process transport/deadlines | ui_stdio, ui_wait | Explicitly start once, enqueue through existing event proxy, bound callback/export work and own process/worker retirement. |
| Trace launch/export | ui_trace | Choose capture/detail/destination, freeze on command and export off the UI thread. No automatic capture or file writes at ordinary startup. |
| Explicit platform services | monitor, window_chrome, document_link | Own native Window, geometry admission, navigation approval and canonical file boundary. Only the monitor query admits documented unsafe platform reads. |

The adapter re-exports the pinned AccessKit/winit integration rather than owning
another semantic tree. Core owns text, layout, focus/capture and snapshots; native
services adapt them. No widget, renderer, account, provider or author runtime is
introduced here.

## Text, events and compatibility

Native source traits project winit data to owned logical events; already
normalized callers borrow it. Host records Native/Command origin. A command key
uses the same logical handler and is deliberately not winit's focus-generated
synthetic key replay; that does not make it physical input.

One registered bridge owns each window's modifiers, requested/received IME state
and bounded click sequence. Duplicate registration rejects; unknown lookup does
not create state. Host ends composition/history before removing the window and
reconciles availability/focus transitions. Read-only arguments suppress writes;
they do not replace Host session transition handling. Clipboard effects occur
only through the injected service on accepted commands; password copy/cut refuse.

Stateful IME publication elides unchanged requests and invalidates on session,
window, caret and scale changes. The legacy stateless `sync_ime` intentionally
publishes each call; it is useful without retained bridge state. Neither is a
second text engine. Text pointer timing is explicit caller time with the existing
450 ms / five-logical-pixel policy, not a claim to mirror every OS setting.

`changes_ui` is an ordinary observation-invalidation hint. A false value never
permits skipping lifecycle dispatch: Host still handles occlusion, renderer
availability, capture cancellation and changed presentation policy. Infallible
native point/wheel/resize helpers require live valid caller geometry; their
existing fallback/cast policies remain. A window move request uses physical
integer coordinates after the existing bounded scalar conversion.

## Source admission and limits

`UiInputRequest::parse` validates its versioned envelope and strict step bodies.
The 64 source-step and 256 expanded UiInputStep limits have one private constant
authority shared with discovery JSON. Text bytes share Core's editor limit.
Expanded steps are not emitted event count: key conversion adds modifier and
press/release bookends. Raw public request DTOs are not parser-issued permission
tokens; parsed bounds do not apply to arbitrary forged vectors.

Named keys use pinned winit serde as their authority, with one-scalar character
fallback. There is no hand-copied named-key catalogue. Host validates old source
proof against the current snapshot, then current route/modal/sensitivity/hit
admission before emitting events. Observation retains immutable editor snapshots
so an edit beyond a truncated query prefix still rejects stale text/hit input.
Host bounds retained observations and releases them with their window/context.

Window commands require the source root. Close returns the existing close event;
RetryRender returns a legacy F5 intent that only an opted-in Host may interpret
as renderer recovery. Other variants use the supplied winit Window. The helper
does not perform protocol authorization or know a renderer's recovery policy.

## Capture owns pixels; Host owns authority and resources

Capture parsing validates body shape, including exclusive rect/element choice;
the Host owns envelope dispatch and instance/window/revision admission before
relayout. `geometry` intentionally remains reusable against later snapshots to
revalidate a pending capture. It is not source authorization or rendering.

`UiCaptureGeometry` validates Core geometry, scale 0.25–4, positive representable
target dimensions and the existing 4096 edge / 4M pixel readback budget. A crop
budgets its own image; an offscreen allocation must additionally budget the whole
target before creating it. A small presented crop can come from a larger window.
Native child surfaces and OS compositor pixels remain outside this interface.

The renderer's existing target convention stays ceil(f32 viewport × scale).
Inner crop edges use f64 outer floor/ceil, clamped to that target before checked
integer conversion. `logical` remains Core's clipped Rect, including its stored
edge normalization; it is not guaranteed byte-equal to an unnormalized request.

Sensitivity is checked against the complete physical pixels actually read,
including the fractional fringe beyond the requested logical Rect. Half-open
touching edges remain disjoint. Incomplete node/depth coverage rejects before
readback. This is conservative admission against Host-marked redacted visible
bounds, not arbitrary content classification. Host must supply complete current
sensitivity metadata and revalidate it/geometry before completing a pending frame.

Standalone now uses this same geometry/redaction entry for initial capture and
binding revalidation; its parallel private region/requested-rect/redaction rules
are removed. GPU target/ticket lifetime, retries, current resource resolution,
PNG encoding and presentation identity remain Host/renderer responsibilities.

## Explicit I/O lifetime

Stdio owns one process-lived stdin/stdout lock and an 8–256 KiB input-line bound.
It has no cancellation/join handle; a closed window cannot unblock stdin. Embedded
consumers reuse the containing process connection. Ordinary replies wait up to
ten seconds for Host response; condition waits share one 1–5000 ms budget across
polls/replies/sleeps and return timeout without retiring a healthy connection.
Late replies use their own dropped channel, never a successor request's channel.

Serialization runs on the I/O worker. Host may perform bounded GPU waiting and
encoding there; dispatch must enqueue promptly. The adapter cannot preempt a
blocking callback or stdout write, nor enforce a whole-operation deadline after
the Host reply. Host owns output/work budgets and cooperative cancellation. No
additional transport worker, hidden GUI task or general scheduler is added.

Trace stop freezes data without I/O; export uses create-new and rejects a racing
file. Drop discards an unfinished recording. A failed write can leave its own
partial output; it never replaces an existing file. Host retains the original run
error when using `finish_after_run`, and chooses output authority. Web-link launch
is explicit HTTP(S), detached and not proof of page load. Project-relative link
resolution is lexical; eventual file reads still require Host canonical checks.

## Qualification scope

Original runtime reds cover the lost conservative pixel, fractional sensitive
fringe and Standalone's independent admission. Existing composites additionally
protect pending capture revalidation and readback/allocation budget separation.
Two public capture regressions protect cross-module privacy and numeric/resource
admission; they are not snapshots or getter tests.

Full SDK/feature/document/tool checks and affected native capture integration
must bind current frozen source, standard binary and owned process identities.
The existing hidden Main/Float frame probe remains local by default; explicit
existing stdio flags can expose the same public query/capture interface during
that owned probe. This creates no new flag, example or executable entry. Captures
and preparation commands do not sign physical gestures, reader speech, new DPI
exits, Vulkan full-family completion, speed improvement or product adoption.
