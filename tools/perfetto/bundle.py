"""Build an offline, pinned Vesta Trace + Perfetto tools directory. Python stdlib only."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
RELEASE = json.loads((Path(__file__).with_name("release.json")).read_text(encoding="utf-8"))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def obtain(url, expected, cache):
    if not cache.exists():
        with urllib.request.urlopen(url, timeout=120) as response:
            data = response.read(64 * 1024 * 1024 + 1)
        if len(data) > 64 * 1024 * 1024 or sha(data) != expected:
            raise ValueError(f"Unexpected upstream content: {url}")
        cache.parent.mkdir(parents=True, exist_ok=True)
        cache.write_bytes(data)
    data = cache.read_bytes()
    if sha(data) != expected:
        raise ValueError(f"Checksum mismatch: {cache}")
    return data


def extract(archive, destination):
    """Validate the complete archive before writing any entry."""
    with zipfile.ZipFile(archive) as source:
        names = set()
        total = 0
        for entry in source.infolist():
            name = PurePosixPath(entry.filename)
            if (entry.orig_filename != entry.filename or name.is_absolute() or "\\" in entry.filename or ":" in entry.filename
                    or any(p in ("", ".", "..") for p in entry.filename.split("/"))
                    or entry.filename in names or entry.is_dir()
                    or (entry.external_attr >> 16) & 0o170000 == 0o120000
                    or entry.file_size > 32 * 1024 * 1024):
                raise ValueError(f"Unsupported archive entry: {entry.filename}")
            names.add(entry.filename)
            total += entry.file_size
        if total > 256 * 1024 * 1024 or len(names) > 1024:
            raise ValueError("Upstream archive exceeds the bundle budget")
        required = {"index.html", f"{RELEASE['ui_directory']}/frontend_bundle.js",
                    f"{RELEASE['ui_directory']}/trace_processor.wasm"}
        if not required <= names:
            raise ValueError("Incomplete upstream UI")
        files = []
        for entry in source.infolist():
            data = source.read(entry)
            target = destination.joinpath(*PurePosixPath(entry.filename).parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            files.append({"path": entry.filename, "bytes": len(data), "sha256": sha(data)})
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New tools directory")
    parser.add_argument("--executable", type=Path, help="Use an already built vesta-trace")
    parser.add_argument("--demo-executable", type=Path, help="Use an already built vesta-motion")
    parser.add_argument("--assets-only", action="store_true", help="Output only perfetto-ui assets")
    parser.add_argument("--archive", type=Path, help="Use a cached official ZIP (checksum enforced)")
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error("Output already exists; choose a new directory")
    cache = ROOT / "target" / "perfetto-download" / RELEASE["version"]
    archive = args.archive or cache / "perfetto-ui.zip"
    obtain(RELEASE["archive_url"], RELEASE["archive_sha256"], archive)
    license_bytes = obtain(RELEASE["license_url"], RELEASE["license_sha256"], cache / "LICENSE")
    executable = args.executable
    if not args.assets_only and executable is None:
        subprocess.run(["cargo", "build", "--locked", "--release", "-p", "vesta-trace", "-p", "vesta-motion", "--features", "vesta-trace/perfetto"],
                       cwd=ROOT, check=True)
        executable = ROOT / "target" / "release" / "vesta-trace.exe"
    demo = args.demo_executable or (executable.with_name("vesta-motion.exe") if executable else None)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="vesta-perfetto-", dir=output.parent) as stage:
        staged = Path(stage) / "bundle"
        assets = staged if args.assets_only else staged / "perfetto-ui"
        assets.mkdir(parents=True)
        files = extract(archive, assets)
        (assets / "LICENSE.txt").write_bytes(license_bytes)
        files.append({"path": "LICENSE.txt", "bytes": len(license_bytes), "sha256": sha(license_bytes)})
        manifest = {"schema_version": 1, **RELEASE, "files": sorted(files, key=lambda f: f["path"])}
        (assets / "vesta-perfetto.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        if not args.assets_only:
            shutil.copy2(executable, staged / "vesta-trace.exe")
            shutil.copy2(demo, staged / "vesta-motion.exe")
            shutil.copy2(ROOT / "apps" / "vesta-trace" / "PERFETTO.md", staged / "README.md")
        staged.rename(output)
    print(json.dumps({"status": "bundled", "version": RELEASE["version"],
                      "archive_sha256": RELEASE["archive_sha256"], "files": len(files),
                      "output": str(output)}))


if __name__ == "__main__":
    main()
