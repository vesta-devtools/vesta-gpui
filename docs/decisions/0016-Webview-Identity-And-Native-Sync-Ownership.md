# WebView identity and native synchronization ownership

Status: Accepted; local family implementation and qualification completed.
Date: 2026-10-31.

## Findings and reviewed scope

Review the complete WebView package public surface: page/profile inputs,
creation identity/events, native synchronization/navigation/evaluation, errors,
mounted resources and the Windows adapter/worker. Existing native Agent and
Builder callers retain their own generation and dispose a child before its
parent. The carrier must not become a second Host authority or author bridge.

`WebView.owner` is a redundant public mutable copy. The Windows callbacks capture
the create argument by value and never read that field. Changing it therefore
does not change emitted identity. Current SDK and Vesta callers do not read or
write the field. Remove the copy, with an explicit preview migration: retain the
creation argument and use `WebEvent.owner`; rotate identity by recreating a view,
not mutating a field. No replacement getter/wrapper or identity allocator.

`sync` currently caches visibility/bounds only after all native calls succeed.
If hide succeeds and moving fails, it reports visible and can skip a later show.
If moving succeeds and showing fails, it forgets the accepted geometry. Native
effects are not an atomic transaction and cannot be treated as one.

## Decision

Keep one private synchronization body, borrowing the existing cache fields and
the two narrow native operations. This is the actual side-effect admission
boundary, not another backend trait, window model or runtime. Reproduce the old
partial-effect failure there before fixing it. Record each successful hide,
move and show immediately; an error preserves those known completed operations.
Maintain operation ordering and current geometry admission, with no fabricated
rollback, new tri-state cache or public synchronization API.

Document the entire family's real limits: Windows WebView2 and event-thread
affinity; synchronous native creation; child-before-parent disposal; ephemeral
versus Host-owned installed profiles; fixed creation event tags and Host stale
event rejection; bounded native callbacks/scripts/URLs and read-only resource
capabilities. Preview navigation is not installed-app origin policy. There is no
automatic author permission, UI capture of native-child pixels, async creation
promise or cancellation/join promise for a draining resource queue.

Qualify the existing resource and origin contracts and a finite public native
consumer with local content, bounded callbacks and ordered disposal. Use normal
Cargo targets and actual own process identities. Any newly observed lifecycle
defect remains open until resolved; do not hide it behind documentation.

No extra production dependency, public wrapper, scheduler, event loop or Core
semantics. Retain supported/experimental boundaries and original evidence.
Remote CI and immutable Vesta adoption remain separate programme exits.

## Measured profile-retirement correction

The first native page cases passed, but the observer initially mixed UTC/local
timestamps and its empty process sample was rejected. Corrected native02 sampled
20 actual descendants, observed their exit, and still found both SDK temporary
profiles on disk. WebView2 appends `EBWebView` below the supplied profile root;
the first classifier also missed that suffix. Preserve both original receipts
and the direct `profile-residue.json` evidence. Immediate TempDir destruction on
the UI thread races native browser file locks and silently leaves residue.

Use the existing per-view resource I/O role for both bounded resource responses
and temporary-profile retirement. Html/URL views also need this real I/O owner;
do not add a second cleanup executor or global queue. Move the profile into that
worker, finish any current read and discard queued work after close, retry only the
owned temporary path for a bounded period, then release all I/O capabilities.
Installed application profiles remain Host-owned and are never removed.

Add one explicit Host-created `WebRetirement` observation group and require it
at `WebView::create`. Every worker takes a private group lease before startup,
including a failed native creation; spawn failure releases the lease. The group
holds only CPU count/error observations, not a sender, view, parent or profile.
It lets a Host dispose children on the event thread and observe completion only
after its event loop returns. A timeout observes unfinished ownership; it is not
cancellation or permission to destroy resources. Keep the existing WebError
shape and report cleanup failure through that error contract.
Keep the count and first error in one mutex-protected state; one condition variable
wakes off-loop observers, without a polling timer or another execution thread.

This additional experimental signature migration is necessary for a concrete
leak; retaining an unobservable old creation path would preserve the wrong end
state. Document the required lifetime argument and Host drain. The old product
pin is not silently migrated; current immutable consumer adoption remains GF06.

The native fixture is a genuine Cargo example in the existing Standalone package,
using a development-only dependency on this already present family member.
Complete the workspace's existing dependency declaration for that member and let
Cargo update the lock's internal dependency edge; no third-party version or
production package edge is added. Fixture files/profiles are created only in an
explicit new directory, retained as owned evidence, with no user profile/login.

## Local result

2026-10-31: the partial-effect red and actual two-profile residue are retained.
Six focused contracts cover the API/resource boundary, including unavailable
parent creation and a real Windows file lock; the final whole SDK passed 220
contracts / 36 harnesses plus CPU smoke, docs, both strict configurations, tool
checks, formatting and ordinary builds. Full01's four fixture style warnings
are retained and fixed without suppressions. Final native04, actual example
`2d82cfd9…` / PID 9768, completed all four page cases, normal exit, retirement,
twenty observed descendant exits and both temporary-root removals; the Host
profile remained. Thirteen report/corruption checks reject incomplete evidence.

The initially unobserved native01 temporary roots cannot be identified from its
receipt and were not guessed or glob-deleted. Both specifically identified
native02 residues were removed only after exact process identity and absolute
path checks; the failed resource qualification remains failed. No physical
input, user assistance, remote CI or product adoption is claimed. Vesta remains
on its published old revision until GF06 migrates both callers and qualifies
one immutable revision. This decision stays active until that migration.
