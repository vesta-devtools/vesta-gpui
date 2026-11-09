use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use vesta_gpui_webview::AssetRoot;

fn bundle() -> (tempfile::TempDir, Bundle) {
    let directory = tempfile::tempdir().expect("asset root");
    std::fs::write(directory.path().join("index.html"), "Official UI fixture").expect("fixture");
    let bundle = Bundle {
        root: AssetRoot::new(directory.path()).expect("root"),
        files: BTreeSet::from(["index.html".into()]),
    };
    (directory, bundle)
}
fn request(
    server: &Server,
    method: &str,
    path: &str,
    body: &[u8],
    extra: &str,
) -> (u16, String, Vec<u8>) {
    let address = server.origin.trim_start_matches("http://");
    let mut stream = TcpStream::connect(address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Length: {}\r\n{extra}\r\n",
        body.len()
    )
    .expect("headers");
    stream.write_all(body).expect("body");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("response");
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("HTTP headers");
    let headers = String::from_utf8(response[..split].to_vec()).expect("text headers");
    let status = headers
        .split_whitespace()
        .nth(1)
        .expect("status")
        .parse()
        .expect("numeric status");
    (status, headers, response[split + 4..].to_vec())
}
#[test]
fn loopback_import_is_inert_preserves_evidence_and_rolls_back_failed_replacement() {
    let (_directory, bundle) = bundle();
    let mut server = Server::start(bundle, None, None, Vec::new()).expect("server");
    let json = crate::tests::fixture().to_string();
    let html = format!(
        "<!doctype html><script>throw 'NEVER EXECUTE'</script><script type=\"application/json\" id=\"gpui-data\">{json}</script>"
    );
    let capture_path = format!("{}/capture", server.prefix);
    let (code, _, body) = request(&server, "POST", &capture_path, html.as_bytes(), "");
    assert_eq!(code, 200);
    let status: Value = serde_json::from_slice(&body).expect("status");
    assert_eq!(
        status["capture"]["source"]["sha256"],
        format!("{:x}", Sha256::digest(html.as_bytes()))
    );
    assert_eq!(status["capture"]["events"], 4);
    assert!(
        !status["capture"]["diagnostics"]
            .as_array()
            .expect("warnings")
            .is_empty()
    );
    let (code, _, body) = request(
        &server,
        "GET",
        &format!("{}/capture/1", server.prefix),
        b"",
        "",
    );
    assert_eq!(code, 200);
    assert_eq!(body, json.as_bytes());
    let (code, _, _) = request(&server, "POST", &capture_path, b"invalid", "");
    assert_eq!(code, 422);
    let (_, _, body) = request(
        &server,
        "GET",
        &format!("{}/status", server.prefix),
        b"",
        "",
    );
    let status: Value = serde_json::from_slice(&body).expect("status");
    assert_eq!(status["revision"], 1);
    assert_eq!(status["capture"]["events"], 4);
    assert_eq!(status["error"]["code"], "trace_json");
    server.shutdown().expect("worker retired");
    assert!(TcpStream::connect(server.origin.trim_start_matches("http://")).is_err());
}
#[test]
fn loopback_routes_reject_foreign_origins_escaping_paths_and_unbounded_requests() {
    let (_directory, bundle) = bundle();
    let mut server = Server::start(bundle, None, None, Vec::new()).expect("server");
    for (method, path, extra, expected) in [
        (
            "GET",
            "/index.html",
            "Origin: https://untrusted.example\r\n",
            403,
        ),
        ("GET", "/index.html", "Sec-Fetch-Site: cross-site\r\n", 403),
        ("GET", "/../Cargo.toml", "", 404),
        ("GET", "/%2e%2e/Cargo.toml", "", 404),
        ("GET", "/vesta/wrong/status", "", 404),
        ("POST", "/index.html", "", 405),
    ] {
        assert_eq!(request(&server, method, path, b"", extra).0, expected);
    }
    let (code, headers, body) = request(&server, "HEAD", "/index.html", b"", "");
    assert_eq!(code, 200);
    assert!(body.is_empty());
    assert!(headers.contains("Cache-Control: no-store"));
    assert!(headers.contains("connect-src 'self'"));
    assert!(!headers.contains("Access-Control-Allow-Origin"));
    let address = server.origin.trim_start_matches("http://");
    let mut stream = TcpStream::connect(address).expect("connect");
    write!(
        stream,
        "POST {}/capture HTTP/1.1\r\nHost: {address}\r\nContent-Length: 268435457\r\n\r\n",
        server.prefix
    )
    .expect("oversized request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("response");
    assert!(response.starts_with("HTTP/1.1 400"));
    server.shutdown().expect("retired");
}

#[test]
fn loopback_delivers_assets_larger_than_the_windows_socket_send_buffer() {
    let (directory, bundle) = bundle();
    let expected = vec![b'x'; 4 * 1024 * 1024];
    std::fs::write(directory.path().join("index.html"), &expected).expect("large JS fixture");
    let mut server = Server::start(bundle, None, None, Vec::new()).expect("server");
    let (code, _, actual) = request(&server, "GET", "/index.html", b"", "");
    assert_eq!(code, 200);
    assert_eq!(actual.len(), expected.len());
    assert_eq!(Sha256::digest(actual), Sha256::digest(expected));
    server.shutdown().expect("retired");
}
