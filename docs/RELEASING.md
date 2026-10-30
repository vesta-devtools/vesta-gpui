# Releasing the source family

Use Windows x86-64, Rust 1.89.0 with clippy/rustfmt and Node.js 22 or newer.
The native applications additionally require a compatible Vulkan loader/driver,
Windows host fonts, and WebView2 when that optional carrier is used.

Start in this repository with its ordinary default target. The first local cold
qualification reserves about 25 GiB of disk as an estimate and uses four Cargo
jobs. Actual usage and command exits belong in the qualification record.

```powershell
node --test tools/sdk/gpui_source.test.mjs tools/verify/gpui_profile.test.mjs
node tools/sdk/gpui_source.mjs --verify .
cargo build --locked --workspace --all-targets --all-features -j 4
cargo test --locked --workspace --all-targets --all-features -j 4
cargo test --locked --workspace --doc --all-features -j 4
cargo clippy --locked --workspace --all-targets --all-features -j 4 -- -D warnings
cargo clippy --locked -p vesta-gpui-vulkan --all-targets --no-default-features -j 4 -- -D warnings
cargo fmt --all -- --check
```

After intentional source edits, `node tools/sdk/gpui_source.mjs --seal .` writes a
new SOURCE inventory. It is an integrity manifest, not a signature or a test
result. Re-run affected checks before distributing a changed candidate. The
verifier rejects additions, missing/tampered files and links; Git administration
and the project's generated default target are the only exclusions.
The current seal command creates the next `0.1.0-rc.2` candidate. Verification
also accepts earlier rc inventories with their exact source bytes. Never reseal
or overwrite an archived candidate to make changed code appear previously tested.

With a verified source inventory, create a non-overwriting source ZIP using:

```powershell
powershell -NoProfile -File tools/release/package.ps1 -Candidate 0.1.0-rc.2
```

The output is under `target/distribution/0.1.0-rc.2`. Packaging reads every source
entry back from the ZIP and checks its hash; it does not include local build
artifacts, account metadata, private original import receipts or Git history.
SHA-256 values identify bytes and are not cryptographic publisher signatures.

Before a public release, attach the actual qualification sidecar, review the
release notes, use the configured deliberate public Git author and commit the
qualified bytes to the existing public repository. Verify the immutable revision and
source seal again before tagging. Replace candidate wording only after those
steps have actually completed. A source release and crates.io publishing are
separate processes; do not enable registry publishing as a side effect.

The repository workflow runs the tool contracts, source verification, format,
workspace tests, doctests and strict all/base-feature checks without a GUI.
The resolved boundary check is part of that workflow. With the pinned toolchain,
generate full `cargo metadata --locked --format-version 1 --all-features` output
as a UTF-8 file, then run `node tools/verify/architecture.mjs <metadata.json>`.
`--no-deps` omits the graph and is rejected. Its contract runs with
`node --test tools/verify/architecture.test.mjs`.

CI outcomes must be read from the actual run; the presence of a workflow file is
not a passing CI result. Native profile verification can be run separately:

```powershell
node tools/verify/gpui_profile.mjs <stdout.log> <result.json>
```

The CLI reads existing logs and receipts only. It validates the actual process
outcome, artifact identity and raw timing/association invariants; it never starts
a native process, supplies physical input or turns synthetic unit fixtures into
performance evidence.
