# Repository cleanup and qualification chronology

Status: Accepted; implementation and checks recorded separately.
Date: 2026-11-01.
Base: 40b8be17aef619547d13dfcbed45e64bef161162.

The user requests a complete repository analysis and cleanup. Review the eight
libraries, public entry points and callers, applications, validation tools,
documentation, source distribution and generated evidence. A large example or
an uncalled public symbol is not sufficient evidence of dead framework code.
Keep the existing ownership graph and supported contracts unless a concrete
duplicate authority or unused responsibility is demonstrated.

Use ARCHITECTURE.md for state and dependency ownership, SUPPORTED-API.md and the
package inventories for API support, and REFINEMENT.md for the completed finite
programme. Record the dated repository assessment in REPOSITORY.md. It explains
retention and remaining maintenance work without becoming another API inventory,
capability matrix or executable dependency policy.

Reconcile the completed GF01–GF06 delivery at revision 40b8be17 with its exact CI
and final consuming-product audit. Retain the earlier failures and historical
checkpoints. Documentation maintenance after that revision is a new source
candidate, not a retrospective extension of its CI or native qualification.
Product adoption stays on its existing immutable revision until a separately
coordinated update. The final 0.1.0 release and original physical/Reader/DPI and
unbound table report retain their distinct status.

Replace Motion's duplicate SDK guide with a short link to its README, preserving
the existing entry URL. Make Standalone's test-only Dock imports explicit with
cfg(test), removing its broad unused-import allowance. Correct stale comments
and release instructions against the actual maintained tools and workflow.
Do not add a new crate, maintenance framework, test counter or periodic process.

Keep native examples, workload fixtures, structured UI access and opt-in fault
injection with their documented purpose. Do not delete generated research
evidence or build caches as a substitute for source cleanup. Verify affected
software paths, documentation links and source integrity; a source reorganization
does not require restarting previously qualified physical acceptance scenes.
