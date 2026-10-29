# Releasing the source family

Use Windows x86-64, Rust 1.89.0 with clippy/rustfmt and Node.js 22 or newer.
The native applications additionally require a compatible Vulkan loader/driver,
Windows host fonts, and WebView2 when that optional carrier is used.

Start in this repository with its ordinary default target. The first local cold
qualification reserves about 25 GiB of disk as an estimate and uses four Cargo
jobs. Actual usage and command exits belong in the qualification record.

```powershell
node --test tools/sdk/gpui_source.test.mjs
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

With a verified source inventory, create a non-overwriting source ZIP using:

```powershell
powershell -NoProfile -File tools/release/package.ps1 -Candidate 0.1.0-rc.1
```

The output is under `target/distribution/0.1.0-rc.1`. Packaging reads every source
entry back from the ZIP and checks its hash; it does not include local build
artifacts, account metadata, private original import receipts or Git history.
SHA-256 values identify bytes and are not cryptographic publisher signatures.

Before a public release, attach the actual qualification sidecar, review the
release notes, configure a deliberate public Git author, create the public
repository and commit the qualified bytes. Verify the immutable revision and
source seal again before tagging. Replace candidate wording only after those
steps have actually completed. A source release and crates.io publishing are
separate processes; do not enable registry publishing as a side effect.
