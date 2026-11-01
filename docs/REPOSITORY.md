# Repository review and maintenance

Reviewed: 2026-11-01, starting from revision 40b8be17.
Decision: [0033](decisions/0033-Repository-Cleanup-And-Qualification-Chronology.md).

The eight-library structure remains useful. The concrete cleanup findings are
duplicate Motion instructions, stale programme status and references, and a
Standalone import allowance that obscured the test boundary. This pass resolves
those findings without changing supported library contracts. Runtime, native
acceptance and public release retain their separate qualification identities.

## Repository contents

The starting tree contained 495 tracked files, including 384 Rust files and 488
UTF-8 text files. All checked text decoded strictly without replacement characters.
One pair of whole files had identical content: Motion's README and SDK guide.
No tracked scratch, backup or alternate source-tree paths were found.

| Area | Purpose and retention |
| --- | --- |
| `crates/` | Eight reusable libraries. Their production dependency graph and state owners are defined in [ARCHITECTURE](ARCHITECTURE.md); package/API support lives in [SUPPORTED-API](SUPPORTED-API.md). |
| `apps/vesta-gpui-standalone/` | Controls, documents, collections, Dock and multi-window acceptance host, plus CPU workloads and small lifecycle examples. Its roughly 49,000 Rust lines include tests, examples and benchmarks. It is a substantial test consumer, not the minimum code needed to embed GPUI. |
| `apps/vesta-motion/` | Approximately 3,100 Rust lines demonstrating viewport-backed motion content. It validates an independent application composition; author save/undo/export are outside its documented scope. |
| Library `examples/` | Focused CPU, control-session and native integration recipes. The two renderer reference binaries share one private host instead of duplicating it. |
| `tools/sdk/` | One maintained-source enumeration and seal/verifier, with corruption tests. Import identities stay historical; no extraction command or second editable source tree is retained. |
| `tools/verify/` | Resolved dependency policy and read-only verification of frame, viewport, performance and lifecycle evidence. These tools inspect recorded outputs; they do not create physical acceptance. |
| `docs/` and `release/` | Current contracts, maintenance/release procedure, dated decisions and preserved qualification history. Historical receipts retain their original source identities. |
| `target/` | Ignored Cargo output, generated documentation, source distributions and local research evidence. It is excluded from source sealing and distribution. Evidence is retained; cache deletion is not part of source cleanup. |

For a new consumer, begin with the [small native host](HOSTING.md), then use the
capability contracts. Copying the entire Standalone host would also copy its
gallery policy and acceptance orchestration into the application.

## Review of the library boundaries

| Family | Assessment |
| --- | --- |
| Core | Identity, layout, text, input, semantics and bounded trace share no application model or OS event loop. No second solver or text engine is justified by the review. |
| Widgets | The broad widget set contains projections and controlled interaction helpers. Similar-looking collection, popup and editing helpers serve different ownership or interaction contracts; names alone do not establish redundant implementations. |
| Native adapter | Window/IME/accessibility/clipboard and explicitly started UI transport remain platform responsibilities. The stdio process-lifetime limitation is documented rather than hidden behind a new executor. |
| Vulkan renderer | Preparation, submission, capture, recovery and resource retirement have distinct lifecycle duties. Existing render entry points share the internal implementation. Single-scene rendering is still used by Standalone, Motion and Vesta's facial consumer; explicit target mode is used by Builder. Preserve their documented admission differences. |
| Workspace model | Stable IDs, checked mutations and versioned sessions form a real renderer-independent model boundary. |
| Workspace projection | Dock projects the existing model and provides geometry/gesture contracts; native window policy belongs to the host. Merging it with the model would remove a useful dependency boundary. |
| WebView | The optional carrier owns browser children, mounted assets and per-view retirement. It is separate from Core and renderer consumption. |
| Viewport | Optional camera/scene/geometry snapshots remain backend-neutral. Fixed scenes belong to examples or private test fixtures. |

The resolved architecture check includes optional, target-specific, normal,
build and transitive paths. Source/API review also inspected the library roots,
the retained contract inventories and actual callers of the render compatibility
entry points. Public functions without a repository caller are not automatically
dead: a published contract can serve external consumers.

All 62 direct dependency declarations across the ten workspace packages have
Rust source references. This lexical screen found no removal candidate; it is
not compiler proof that every dependency or feature is minimal. The maintained
verification tools likewise have documented commands or module/test callers.

A normalized 12-consecutive-line scan of 179 non-test library source files found
no cross-file clone candidates in their production prefixes. Inline test tails
were excluded from that heuristic. This detects obvious copied blocks, not all
semantic duplication. No `todo!()` or `unimplemented!()` production placeholder
was found. These checks support a bounded assessment, not a claim of zero defects.

## Cleanup decisions

| Finding | Resolution |
| --- | --- |
| Motion instructions maintained twice | README remains authoritative; SDK.md becomes a short forwarding guide so existing links keep working. |
| Motion's source guide linked to a missing README anchor; its concept image was unexplained | The guide links to the actual examples section. README identifies the retained image as a design reference, separate from native screenshots and implemented authoring features. |
| Completed programme still described as active | [REFINEMENT](REFINEMENT.md), its decision status and current navigation now identify the completed 40b8 delivery and final audit. Earlier drifted builds remain failures; changed maintenance bytes require new checks. |
| Broad Standalone unused-import allowance | Production Dock imports and the five test-only imports are separated with `cfg(test)`. The existing regression suite protects both configurations. |
| Renderer boundary-test comment referenced removed product-only tooling | The comment now describes the actual resolved production graph and native-example development dependencies. Assertions and behavior are unchanged. |
| Manual release command listed fewer Node test suites than CI | [RELEASING](RELEASING.md) lists the same five test files as the maintained workflow. |
| Source enumerator still described a retired trusted-worker consumer | Its comment now names its current source-seal and verification callers; enumeration behavior is unchanged. |

The renderer examples' local `allow(dead_code)` remains necessary because each
reference binary selects one variant of their shared two-mode private host.
Fault-injection state stays behind its explicit feature. The `temporary_editor`
module implements a real window-owned temporary editing session; its name does
not identify unfinished development code.

## Future maintenance

Keep one owner for each concern: architecture in ARCHITECTURE, API support in the
capability inventories, finite programme evidence in REFINEMENT, and new defects
with their source and affected contract. Preserve historical records without
presenting their old remaining work as a current backlog.

Standalone remains the largest maintenance surface. Organize a new regression
beside its interaction owner and existing scenario. Consolidate tests only when
they protect the same failure mode with the same inputs and scope; do not delete
edge-capture, focus, expiry, restoration or loss cases just because they look
similar. A file's length alone does not justify another crate or public helper.

This cleanup does not change Vesta's immutable SDK pin. Library runtime bytes
remain those of the completed delivery; the Standalone import configuration and
test comment receive directed software checks and a normal standard build.
Local records for this pass are under `target/research/repository-audit-20261101/`.
They qualify these maintenance inputs separately from the earlier exact CI and
native evidence.

Final 0.1.0 publication, native preview limits, original Reader/cross-monitor DPI
constraints and the unbound UI-DOC-TABLE-001 host/input report remain as recorded
in [VALIDATION](VALIDATION.md) and the consuming product's acceptance ledger.
