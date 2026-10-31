# Public error interoperability

Status: Accepted; external consumers and frozen full software qualification passed.
Date: 2026-10-31.

## Finding

The retained public error families are inconsistent. The text family already
supports Rust's standard error chain, but 17 flat error enums in Core, Widgets,
the native text registry and workspace projection do not. A host using ordinary
`?` propagation must wrap or stringify these failures, even though their existing
variants carry the identities and validation reasons the host needs.
The same inventory found three existing wrapper enums whose standard `source()`
is empty despite carrying a typed error: `MarkdownError`,
`MarkdownProjectionError` and `WebLinkOpenError`.

## Decision

Implement `Display` and `std::error::Error` on those existing enums in their own
modules. Preserve all variants, payloads, constructors, validation, ownership and
failure atomicity. Their variants are the machine-readable contract; display text
is human context, not a new stable code or a string-matching protocol. These flat
enums have no underlying error cause and use the default `source() == None`.

Expose those three wrappers' existing typed causes through `source()`; leaf
variants still have no source. Preserve their payloads and display behavior.
Keep the already cause-preserving wrappers unchanged. In particular,
`TextInputBridgeError<E>` remains a standard error only when its injected
clipboard error satisfies that trait; do not tighten the clipboard service bound.
Do not add an umbrella error, wrapper layer, macro, dependency, conversion table
or alternate validation model. The owning crate remains the sole authority.

## Qualification

Use external consumer contracts: ordinary `?` into a host error, downcast to the
original typed failure, and verify that rejected actions/registration/edits retain
their preexisting state and identity. Extend the existing Core/native tests and
the Widget consumer harness; use one focused workspace consumer contract because
its only existing external harness checks dependency boundaries. Compile bounds
cover every newly interoperable enum without a separate test per variant or
assertions on display wording. Preserve a real baseline compile rejection.
Verify the three missing source chains separately with runtime failures before
repair, including link validation/launch and Markdown anchor/geometry/accessibility
causes. This completes the retained error-family interoperability audit; it does
not redesign legacy string-valued diagnostic payloads.

Run the affected crate contracts, then the frozen workspace, docs, strict feature
checks, tools, format and standard builds before committing. No UI geometry,
input behavior, renderer, application entry point or Vesta source is changed.
Existing native/physical evidence is retained at its original identity; it is not
repeated or expanded for these trait additions. GF02 and the full refinement
programme remain open for their other findings.

## Outcome

The external baseline in `public-errors-20261031/red02` failed to compile in all
four affected crates, with only the expected missing standard-error trait
diagnostics. The first attempt also had an invalid ElementKey fixture argument;
that failure is retained as `red01`, not counted as the clean baseline. After
the flat-enum repair, `causes01` reproduced the native and Markdown wrappers'
empty sources as runtime failures while the Core/workspace consumers passed.

The final four focused consumers passed nine contracts. The full frozen
`refinement-text-host-20261031/errors01` passed 215 contracts across 33 harnesses,
with no failures or ignored tests. Documentation, all/base strict checks, the
three tool groups, format and both standard builds passed without source drift.
The error inventory retains the same 37 named definitions; the conditional
clipboard wrapper and already-standard WorkspaceIdentityOverflow remain intact.

The checks protect ordinary host propagation/downcasting, all six newly exposed
typed causes, retained text, rejected duplicate action routes, an existing window
bridge's click sequence, and window-qualified one-shot edit/split cancellation.
No browser is launched by the link-error contract. The standard examples were
rebuilt; no new native interaction, Vesta adoption, remote CI or programme-wide
completion is claimed. Closure is recorded under
`target/research/public-errors-20261031`.
