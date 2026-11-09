use crate::session::TraceError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use vesta_gpui_webview::AssetRoot;

pub(super) const VERSION: &str = "v59.1";
const ARCHIVE_SHA: &str = "df4433f5d8d98d2c490249870944c819ff85206f61e55e5983bbcc2338c83904";

#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    version: String,
    archive_sha256: String,
    files: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    bytes: usize,
    sha256: String,
}

pub(super) struct Bundle {
    pub root: AssetRoot,
    pub files: BTreeSet<String>,
}
impl Bundle {
    pub fn open(path: &Path) -> Result<Self, TraceError> {
        let root = AssetRoot::new(path).map_err(|error| {
            TraceError::new("trace_perfetto_bundle_missing", format!(
                "{}: {error}. Build tools with python tools/perfetto/bundle.py --output <new-directory>, or use --native", path.display()))
        })?;
        let resource = root
            .read_relative("vesta-perfetto.json", 1024 * 1024)
            .map_err(|e| TraceError::caused("trace_perfetto_manifest", e))?;
        let manifest: Manifest = serde_json::from_slice(&resource.bytes)
            .map_err(|e| TraceError::caused("trace_perfetto_manifest", e))?;
        if manifest.schema_version != 1
            || manifest.version != VERSION
            || manifest.archive_sha256 != ARCHIVE_SHA
            || manifest.files.len() > 1024
        {
            return Err(TraceError::new(
                "trace_perfetto_version",
                "Unsupported Perfetto tools bundle",
            ));
        }
        let mut files = BTreeSet::new();
        for entry in manifest.files {
            let resource = root
                .read_relative(&entry.path, 32 * 1024 * 1024)
                .map_err(|e| TraceError::caused("trace_perfetto_asset", e))?;
            if !files.insert(entry.path)
                || resource.bytes.len() != entry.bytes
                || format!("{:x}", Sha256::digest(&resource.bytes)) != entry.sha256
            {
                return Err(TraceError::new(
                    "trace_perfetto_checksum",
                    "Perfetto asset checksum mismatch",
                ));
            }
        }
        for required in [
            "index.html",
            "LICENSE.txt",
            "v59.1-1e9c61e98/frontend_bundle.js",
            "v59.1-1e9c61e98/trace_processor.wasm",
        ] {
            if !files.contains(required) {
                return Err(TraceError::new(
                    "trace_perfetto_asset",
                    "Incomplete Perfetto tools bundle",
                ));
            }
        }
        Ok(Self { root, files })
    }
}
