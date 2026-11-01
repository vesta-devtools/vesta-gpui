# Explicit architecture and refinement gates

Status: Accepted; programme completion is recorded in [REFINEMENT.md](../REFINEMENT.md).
Date: 2026-10-31.
Base: 9e31f07d8dc4fa351418bc7321b4afff45783d0d.

The user requests a concise, reliable UI framework with unambiguous architecture.
Every retained layer must have a necessary responsibility, explicit state owner,
dependency direction, lifecycle and error contract. Remove demonstrated redundant
wrappers and conflicting authorities; do not treat a short file or an extra trait
as proof of good architecture. Public changes require a migration when relevant.

Keep the current eight-library direction until a concrete review demonstrates
that a boundary should change. Record the current responsibilities and host
composition in ARCHITECTURE.md, and use resolved Cargo metadata to check normal
and build dependency paths, including optional/target-specific edges. Development
dependencies are excluded from production reachability. External dependencies
must not hide a forbidden native-framework path into backend-neutral layers.
The check is a boundary policy, not a second executable dependency configuration.

Maintain a finite programme in REFINEMENT.md. Separate architecture/API reduction,
frame/layout integration, interaction/lifecycle correctness, performance and
delivery gates. A task is complete only with its stated evidence. Preserve prior
acceptance identities and disclose evidence limits; never call this programme
complete because an audit, source seal or software-only check succeeds.

The recent Agent divider fix belongs to the consuming native host. Review the
Standalone host and existing examples before proposing a common preparation API.
Do not move application state or the event loop into the UI core simply to share
a loop, and do not build a second renderer, text engine or input authority.

Default target/toolchain and source authority rules remain unchanged. Do not
restart the user's Agent or another owner's application during an SDK-only audit.
Publishing, platform expansion and registry release remain explicit later work.
