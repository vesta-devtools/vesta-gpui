# Validation scope

## Current candidate

The first independently maintained candidate is `0.1.0-rc.1`; package API
versions remain `0.1.0`. This is a Windows native 2D developer preview, with
optional experimental Workspace, native WebView and viewport capabilities.

Source integrity, Rust compilation, automated contract tests, native operation
and consuming-application integration are separate evidence. A successful source
seal or compiler run cannot certify physical IME input, screen-reader speech,
cross-monitor DPI changes or the absence of all visual defects.

See [the release procedure](RELEASING.md) for the exact independent commands.
Actual local logs are written under `target/qualification/0.1.0-rc.1`; the
distribution sidecar records their outcomes and the exact source identity.
Absence of an outcome means it has not been qualified. This candidate is not
tagged, publicly hosted or adopted by the Vesta product yet.

## Historical evidence

Links described as historical context in the imported API inventories refer to
the earlier source-workspace acceptance programme. The original reports remain
with the maintainers at their recorded dates and hashes. They are not bundled
here, cannot be fetched from a private product repository by public users, and
are not independent verification of this candidate. The import receipt hash in
`IMPORT.json` preserves the exact original inventory identity.

Those reports include text/focus/menu, Markdown, layout, basic controls, numeric
editing, scrolling, collections, image/icon resources, toolbar/breadcrumb,
temporary editors, suggestions and command-center work. API behavior is described
in each package's current `SUPPORTED-API.md` and its implementation/tests.
The old product's Handoff and broad widget roadmap are not release requirements
or promises of unimplemented product functionality in this repository.

The earlier product accepted some remaining manual interaction checks by an
explicit human waiver. That decision is retained as historical acceptance; it
does not create new physical measurements or implement unsupported capabilities.
Known software defects, independent build failures and unsupported APIs cannot
be waived by source packaging.
