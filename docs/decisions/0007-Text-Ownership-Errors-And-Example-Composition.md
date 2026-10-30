# Text ownership, standard errors and example composition

Status: Accepted; implementation and qualification tracked in REFINEMENT.md.
Date: 2026-10-31.

## Findings

The public text editing error is a typed enum but cannot be used with ordinary
`?` conversion into a host's standard boxed error. The native bridge formats it
with Debug and implements Error without returning its underlying cause. Hosts
must invent adapters for a failure that already has an authoritative typed owner.
Text input's font/cache context and editor state also live in one large module,
although their lifetimes and responsibilities differ.

The example's root module still contains committed demonstration state, action
dispatch, pointer processing and UI construction. Existing startup/event/focus
modules already establish their owners. A caller must currently read unrelated
construction and gesture code in the entry module to find one of those paths.

## Decision

Keep TextInputContext and TextMeasurementRevision in a private context module,
with the same public re-exports. Preserve its font source identity, bounded
caches, explicit host registration and existing invalidation algorithm. Editor
state/history stays in the text editor module. Field visibility is limited to
the existing text subtree; add no context facade or duplicated state.

Provide standard Display/Error interoperability for the existing text failure
types and preserve underlying causes in wrapped shaping/native bridge errors.
Keep enum variants and admission semantics. Messages contain no editor contents;
typed variants remain the machine contract. Protect generic host error conversion
and source traversal using actual failure paths, without freezing exact labels.

Move the example's existing bodies to their responsibility owners: demonstration
state/defaults to application state, action dispatch to demo_actions, pointer
handling to a pointer module, text/window adapters to their window modules, and
UI construction to presentation. The entry retains the single App owner,
identity declarations and composition. Existing functions and branches remain
unchanged; do not create a second state model or add new public SDK API for demo
convenience. Move only the visibility needed inside this binary.

Compare the moved bodies against their prior source and run existing contracts,
documentation, strict checks and ordinary standard example builds. Pure moves
do not require restarting the user's Agent or rerunning historical native scenes.
Production semantics or newly discovered defects require focused extra evidence.

## Outcome

The frozen run05 passed the existing family checks and two external error-chain
contracts (212 total), documentation, strict checks and both standard builds.
All 88 original root function bodies match after migration. Status and evidence
limits remain in the programme and VALIDATION.md; no new native scene is claimed.
