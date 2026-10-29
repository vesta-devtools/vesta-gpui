use super::WebError;
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub const MAX_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct AssetRoot {
    path: PathBuf,
    dir: Arc<cap_std::fs::Dir>,
}
pub struct Resource {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}
impl AssetRoot {
    pub fn new(root: &Path) -> Result<Self, WebError> {
        let root = root
            .canonicalize()
            .map_err(|e| WebError::new("web_root_unavailable", e.to_string()))?;
        if !root.is_dir() {
            return Err(WebError::new(
                "web_root_invalid",
                "Asset root must be a directory",
            ));
        }
        let dir = cap_std::fs::Dir::open_ambient_dir(&root, cap_std::ambient_authority())
            .map_err(|e| WebError::new("web_root_unavailable", e.to_string()))?;
        Ok(Self {
            path: root,
            dir: Arc::new(dir),
        })
    }
    pub fn relative_entry(&self, path: &Path) -> Result<String, WebError> {
        let path = path
            .canonicalize()
            .map_err(|e| WebError::new("web_resource_unavailable", e.to_string()))?;
        let relative = path.strip_prefix(&self.path).map_err(|_| {
            WebError::new(
                "web_resource_outside_root",
                "Page is outside the selected asset root",
            )
        })?;
        if !path.is_file() {
            return Err(WebError::new("web_resource_invalid", "Page must be a file"));
        }
        let relative = relative
            .to_str()
            .ok_or_else(|| WebError::new("web_resource_invalid", "Path must be UTF-8"))?
            .replace('\\', "/");
        Ok(relative)
    }
    /// Call from an I/O worker, never from the native window's event thread.
    pub fn read(&self, encoded_path: &str) -> Result<Resource, WebError> {
        if encoded_path.len() > 8192 {
            return Err(WebError::new(
                "web_resource_invalid",
                "Resource path is too long",
            ));
        }
        let decoded = percent_encoding::percent_decode_str(encoded_path.trim_start_matches('/'))
            .decode_utf8()
            .map_err(|_| WebError::new("web_resource_invalid", "Path is not UTF-8"))?;
        self.read_relative(&decoded, MAX_RESOURCE_BYTES)
    }
    /// A picker-provided relative path is not a percent-encoded browser URL.
    /// Keep its literal filename and the caller's smaller read budget.
    pub fn read_relative(&self, relative: &str, maximum: u64) -> Result<Resource, WebError> {
        if maximum == 0 || maximum > MAX_RESOURCE_BYTES {
            return Err(WebError::new(
                "web_resource_limit",
                "Invalid resource read budget",
            ));
        }
        if relative.is_empty()
            || relative.len() > 8192
            || relative.contains(['\\', ':', '\0'])
            || Path::new(relative)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(WebError::new(
                "web_resource_invalid",
                "Resource path must be relative",
            ));
        }
        let path = Path::new(relative);
        let content_type = match path
            .extension()
            .and_then(|v| v.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("html" | "htm") => "text/html; charset=utf-8",
            Some("js" | "mjs") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("json" | "map") => "application/json",
            Some("wasm") => "application/wasm",
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            Some("svg") => "image/svg+xml",
            Some("woff") => "font/woff",
            Some("woff2") => "font/woff2",
            Some("ttf") => "font/ttf",
            Some("otf") => "font/otf",
            Some("txt") => "text/plain; charset=utf-8",
            _ => {
                return Err(WebError::new(
                    "web_resource_type_disallowed",
                    "Unsupported web resource type",
                ));
            }
        };
        // The directory capability confines symlink/reparse-point resolution at
        // open time, including a replacement racing a prior path check.
        let file = self
            .dir
            .open(path)
            .map_err(|e| WebError::new("web_resource_unavailable", e.to_string()))?;
        if !file
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() <= maximum)
        {
            return Err(WebError::new(
                "web_resource_limit",
                "Resource exceeds the size limit or is not a regular file",
            ));
        }
        let mut bytes = Vec::new();
        file.take(maximum + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| WebError::new("web_resource_unavailable", e.to_string()))?;
        if bytes.len() as u64 > maximum {
            return Err(WebError::new(
                "web_resource_limit",
                "Resource grew beyond the size limit",
            ));
        }
        Ok(Resource {
            bytes,
            content_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_resources_and_navigation_do_not_escape_their_explicit_boundary() {
        let root = tempfile::tempdir().expect("root");
        std::fs::write(root.path().join("index.html"), "<h1>sample</h1>").expect("page");
        std::fs::write(root.path().join("module.wasm"), [0, 97, 115, 109]).expect("wasm");
        let assets = AssetRoot::new(root.path()).expect("assets");
        assert_eq!(
            assets.read("/module.wasm").expect("wasm").content_type,
            "application/wasm"
        );
        assert_eq!(
            assets.read("/index.html").expect("html").bytes,
            b"<h1>sample</h1>"
        );
        assert!(assets.read_relative("index.html", 2).is_err());
        let literal_url = crate::project_page_url("literal%20name.json");
        std::fs::write(root.path().join("literal%20name.json"), b"{}").expect("literal name");
        assert_eq!(
            assets
                .read_relative("literal%20name.json", 2)
                .expect("literal read")
                .bytes,
            b"{}"
        );
        assert_eq!(
            assets
                .read(literal_url.path())
                .expect("literal browser URL")
                .bytes,
            b"{}"
        );
        for path in [
            "../index.html",
            "%2e%2e/index.html",
            "C%3a/index.html",
            "a%5cb.html",
            "%00.html",
        ] {
            assert!(assets.read(path).is_err(), "{path}");
        }
        for url in [
            "file:///C:/private/key",
            "javascript:alert(1)",
            "http://example.com",
            "https://user:password@example.com",
        ] {
            assert!(crate::validate_url(url).is_err(), "{url}");
        }
        for url in [
            "https://example.com",
            "http://localhost:5173",
            "http://127.0.0.1:3000",
            "http://[::1]:8080",
        ] {
            assert!(crate::validate_url(url).is_ok(), "{url}");
        }
    }
}
