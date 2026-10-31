# Workspace invariant and mutation authority

Status: Accepted; local implementation and qualification completed; product migration pending.
Date: 2026-10-31.

## Findings and scope

Review the complete `vesta-workspace` family against actual Standalone, Agent
and Builder consumers. Its recursive model is the sole pane/tab/split authority;
Dock projection and native windows borrow that model. The following concrete
gaps violate that boundary:

- Same-pane merge unconditionally subtracts a slot, so moving a later tab toward
  an earlier slot lands before the requested insertion point.
- Failed `add_item` advances its allocator. Saturating allocators can reuse the
  maximum ID, and a compound dock can mutate before discovering a failure.
- `with_tree` accepts repeated leaves, orphan panes and invalid active/ratio
  state that the separate session validator rejects.
- Public raw model Deserialize and `floating_window_mut` admit unvalidated
  topology/identity edits outside the model's operations.
- Session validation repeats model invariants in a second error vocabulary.

These are existing GF02/GF04 findings, not a new workspace runtime or an expansion
of physical cross-monitor acceptance. Preserve the existing qualified/waived
native scenarios and the product's current SDK pin until immutable adoption.

## Decision

Keep one private workspace state representation, containing the existing fields
exactly once. `WorkspaceModel` owns validated live state; `WorkspaceSession`
owns a cloned, untrusted serialized snapshot of the same representation. Only
the versioned session restores that snapshot into a live model. Remove raw
model Deserialize, retain Serialize for diagnostics, and preserve the version-1
wire fields. No second DTO field list, migration format or authoritative model.

One private invariant validator returns `WorkspaceError` for construction and
session restore/capture. Session errors only add JSON/version context or wrap
that typed model cause. Reject invalid initial active/ratio/identity state rather
than silently repairing author intent. Document the experimental API migration.

Every failing mutator preserves state, including allocator cursors. Check target,
placement and every needed ID before compound mutation. A checked increment
rejects exhausted cursors; zero stays reserved and no counter wraps/saturates.
An all-empty main tree keeps an existing empty pane, avoiding needless identity
allocation during close. Same-pane insertion adjusts only when removal precedes
the requested pre-move slot.

Replace the mutable floating-window escape with narrow title and validated
placement operations. Native OS creation, scale, visibility, ownership, desktop
input and persistence I/O remain Host responsibilities. Migrate this repository's
callers; record Agent/Builder migration for GF06 without changing their old pin.
`set_item_type` also reports a missing item explicitly. Remove the unreachable
`EmptyWorkspace` error: every recursive tree contains a leaf, including an empty
pane; a missing resize container reports `MissingContainer`.
The shared ratio value rule belongs with the value types, eliminating the
previous types-to-mutable-model helper dependency without a new abstraction.

Qualify public ordering/persistence, rejected construction/mutation, exhausted
compound operations and the admission boundary with focused existing contracts
and compile-fail docs. Keep original failing receipts. Complete the full SDK
checks and ordinary builds before a local checkpoint. The projection family's
remaining geometry/gesture review and final immutable adoption stay separate.

## Local result

The original red01 retained 13 passes / three actual failures and two compile-fail
docs that incorrectly compiled. Focused01 found an error-priority regression;
the common validator now preserves missing-pane precedence. Focused02's test
unwrap/must-use strict failures and full01's two private hook conversion failures
are retained and corrected, without weakening assertions or suppressing checks.
Final focused03 passed the model/Dock families, type-boundary docs and strict
checks. A fixed v1 wire sample and terminal-cursor/empty-main cases protect the
same model and persistence contract; pre-seeded fixtures explicitly select
their active items.

Final full06 passed 223 contracts / 36 harnesses, both boundary docs, CPU smoke,
all-feature/base strict, existing tool checks, formatting and ordinary builds.
The first native run's counting failure remains rejected and is resolved under
[decision 0018](0018-Frame-Probe-Submission-Admission.md). Final ordinary native
PID 39856, actual `9babff9f…`, completed Main/Float 64 successful submissions each
and normal exit after restoring the non-fault build. No physical/DPI/reader gate
or published product migration is claimed. Preserve existing historical evidence
and the finite remaining family reviews.
