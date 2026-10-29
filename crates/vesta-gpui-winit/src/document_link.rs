//! Explicit document navigation policy; it never reads project files.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentLink {
    Web(String),
    ProjectFile(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentLinkError {
    MalformedUrl,
    UnsupportedScheme,
    UnsupportedReference,
    InvalidPath,
    InvalidEncoding,
    OutsideProject,
    MissingProjectDocument,
}

impl std::fmt::Display for DocumentLinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "document_link_{self:?}")
    }
}
impl std::error::Error for DocumentLinkError {}

/// Resolve a link relative to an already accepted project-relative document.
/// The eventual file read must still pass the host's canonical project boundary.
pub fn resolve_document_link(
    destination: &str,
    document_path: Option<&str>,
) -> Result<DocumentLink, DocumentLinkError> {
    if destination.is_empty()
        || destination.chars().any(char::is_control)
        || destination.contains('\\')
    {
        return Err(DocumentLinkError::InvalidPath);
    }
    match url::Url::parse(destination) {
        Ok(url) => {
            if !matches!(url.scheme(), "http" | "https") {
                return Err(DocumentLinkError::UnsupportedScheme);
            }
            if url.host_str().is_none() || !destination.contains("://") {
                return Err(DocumentLinkError::MalformedUrl);
            }
            return Ok(DocumentLink::Web(url.into()));
        }
        Err(url::ParseError::RelativeUrlWithoutBase) => {}
        Err(_) => return Err(DocumentLinkError::MalformedUrl),
    }
    if destination.contains(['?', '#']) {
        return Err(DocumentLinkError::UnsupportedReference);
    }
    if destination.starts_with('/') {
        return Err(DocumentLinkError::OutsideProject);
    }
    let current = document_path.ok_or(DocumentLinkError::MissingProjectDocument)?;
    if current.starts_with('/')
        || current.contains(['\\', ':'])
        || current.chars().any(char::is_control)
    {
        return Err(DocumentLinkError::InvalidPath);
    }
    let mut components = Vec::new();
    for part in current.split('/').filter(|part| !part.is_empty()) {
        if matches!(part, "." | "..") {
            return Err(DocumentLinkError::InvalidPath);
        }
        components.push(part.to_owned());
    }
    if components.pop().is_none() {
        return Err(DocumentLinkError::MissingProjectDocument);
    }
    for part in destination.split('/') {
        let part = decode_component(part)?;
        if part.contains(['/', '\\', ':']) || part.chars().any(char::is_control) {
            return Err(DocumentLinkError::InvalidPath);
        }
        match part.as_str() {
            "" | "." => {}
            ".." => {
                components.pop().ok_or(DocumentLinkError::OutsideProject)?;
            }
            _ => components.push(part),
        }
    }
    if components.is_empty() {
        return Err(DocumentLinkError::InvalidPath);
    }
    Ok(DocumentLink::ProjectFile(components.join("/")))
}

fn decode_component(part: &str) -> Result<String, DocumentLinkError> {
    let mut bytes = part.bytes();
    let mut decoded = Vec::with_capacity(part.len());
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            decoded.push(byte);
            continue;
        }
        let mut digit = || {
            bytes
                .next()
                .and_then(|byte| (byte as char).to_digit(16))
                .ok_or(DocumentLinkError::InvalidEncoding)
        };
        decoded.push((digit()? * 16 + digit()?) as u8);
    }
    String::from_utf8(decoded).map_err(|_| DocumentLinkError::InvalidEncoding)
}

/// Launch only a revalidated HTTP(S) destination. Tests/probes never call this.
/// Detached launch completion is not proof that a browser loaded the page.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum WebLinkOpenError {
    Destination(DocumentLinkError),
    Launch(std::io::Error),
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Display for WebLinkOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Destination(error) => error.fmt(formatter),
            Self::Launch(error) => write!(formatter, "document_link_launch_failed: {error}"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::error::Error for WebLinkOpenError {}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_web_link(destination: &str) -> Result<(), WebLinkOpenError> {
    let DocumentLink::Web(url) =
        resolve_document_link(destination, None).map_err(WebLinkOpenError::Destination)?
    else {
        return Err(WebLinkOpenError::Destination(
            DocumentLinkError::UnsupportedScheme,
        ));
    };
    open::that_detached(url).map_err(WebLinkOpenError::Launch)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_keeps_encoded_links_inside_the_project_and_excludes_executable_schemes() {
        assert_eq!(
            resolve_document_link("../%E5%9C%B0%E5%BD%A2.md", Some("docs/intro.md")),
            Ok(DocumentLink::ProjectFile("地形.md".into()))
        );
        assert_eq!(
            resolve_document_link("./nested/file.rs", Some("docs/intro.md")),
            Ok(DocumentLink::ProjectFile("docs/nested/file.rs".into()))
        );
        assert_eq!(
            resolve_document_link("https://example.com/世界?q=1#intro", None),
            Ok(DocumentLink::Web(
                "https://example.com/%E4%B8%96%E7%95%8C?q=1#intro".into()
            ))
        );
        for path in [
            "../../secret",
            "../%2e%2e/secret",
            "/secret",
            "//host/secret",
        ] {
            assert_eq!(
                resolve_document_link(path, Some("docs/intro.md")),
                Err(DocumentLinkError::OutsideProject)
            );
        }
        for path in [
            "file:///C:/private",
            "javascript:alert(1)",
            "data:text/html,a",
            "C:/secret",
        ] {
            assert_eq!(
                resolve_document_link(path, Some("docs/intro.md")),
                Err(DocumentLinkError::UnsupportedScheme)
            );
        }
        for path in ["a%2fb", "a%5Cb", "a%00b", "a%3ab", "a\\b", "a\nb"] {
            assert_eq!(
                resolve_document_link(path, Some("docs/intro.md")),
                Err(DocumentLinkError::InvalidPath)
            );
        }
        for path in ["a%", "%zz", "%ff"] {
            assert_eq!(
                resolve_document_link(path, Some("docs/intro.md")),
                Err(DocumentLinkError::InvalidEncoding)
            );
        }
        assert_eq!(
            resolve_document_link("other.md#chapter", Some("intro.md")),
            Err(DocumentLinkError::UnsupportedReference)
        );
        assert_eq!(
            resolve_document_link("other.md", None),
            Err(DocumentLinkError::MissingProjectDocument)
        );
        assert_eq!(
            resolve_document_link("other.md", Some("../intro.md")),
            Err(DocumentLinkError::InvalidPath)
        );
    }
}
