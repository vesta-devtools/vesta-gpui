# Independent repository ownership

Decision date: 2026-10-29.

This repository maintains the eight GPUI libraries and the Standalone and Motion
examples as one versioned family. The initial source import is identified by
`IMPORT.json`. The original import receipt and source archive are preserved by
the maintainers; they are historical records, not a second live source tree.

The former product-workspace extraction command is retired here.
`tools/sdk/gpui_source.mjs` now seals or verifies this repository's source only.
It cannot import from another checkout, filter out product dependencies or
substitute files during packaging. Source files, notices and lock resolution are
maintained directly in this repository.

Use this project's ordinary Cargo `target` directory. Do not share another
project's compiled target, copy executables around a file lock, or inherit an
external `CARGO_TARGET_DIR`. Package/runtime qualification and the consuming
application's integration qualification are separate results.

All eight packages keep the same release version. A consuming application must
select one immutable Git revision for the family once the public repository
exists. During local development it may use a deliberate path override; this
does not constitute a published release. Changing the Vesta application's
workspace, lockfile or package ownership is a separately coordinated migration.

Future commits require an explicitly chosen public author address. Account
verification, repository creation, public push, release tagging and crates.io
publication remain separate actions. Package manifests currently disable
registry publication; a source release does not imply registry availability.
