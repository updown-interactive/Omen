use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::{Component, Path},
    time::Duration,
};
pub struct Web;
impl PlatformRunner for Web {
    fn detect(&self) -> PlatformStatus {
        PlatformStatus::new(
            Platform::Web,
            "WASM + DOM · toolchain detected; browser launch not verified",
            vec![
                (tools::find("cargo").is_some(), "Cargo"),
                (tools::python().is_some(), "Python 3.9+"),
                (
                    tools::installed_targets()
                        .iter()
                        .any(|s| s == "wasm32-unknown-unknown"),
                    "Rust target wasm32-unknown-unknown",
                ),
            ],
        )
    }
    fn build(&self, p: &Project, o: &BuildOptions) -> Result<BuildArtifact> {
        package(p, Platform::Web, o)
    }
    fn run(&self, _: &Project, a: &BuildArtifact, o: &RunOptions) -> Result<()> {
        let root = a.path.canonicalize()?;
        let server = TcpListener::bind(("127.0.0.1", o.port))
            .context("Could not bind browser development server")?;
        let url = format!("http://127.0.0.1:{}/", server.local_addr()?.port());
        if !o.no_open {
            open_browser(&url)?;
        }
        println!("Serving {url}\nPress Ctrl-C to stop.");
        std::io::stdout().flush()?;
        for stream in server.incoming() {
            let mut stream = stream?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            stream.set_write_timeout(Some(Duration::from_secs(10)))?;
            let mut request = [0_u8; 8192];
            let len = match stream.read(&mut request) {
                Ok(n) => n,
                Err(_) => continue,
            };
            let line = String::from_utf8_lossy(&request[..len]);
            let mut parts = line.lines().next().unwrap_or("").split_whitespace();
            let method = parts.next().unwrap_or("");
            let url = parts.next().unwrap_or("");
            let response = if method != "GET" && method != "HEAD" {
                None
            } else {
                safe_path(&root, url).and_then(|p| std::fs::read(&p).ok().map(|b| (p, b)))
            };
            let (status, mime, bytes) = match response {
                Some((path, bytes)) => ("200 OK", mime(&path), bytes),
                None => ("404 Not Found", "text/plain", b"Not found".to_vec()),
            };
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            if stream.write_all(header.as_bytes()).is_ok() && method != "HEAD" {
                let _ = stream.write_all(&bytes);
            }
        }
        Ok(())
    }
}
pub fn safe_path(root: &Path, url: &str) -> Option<PathBuf> {
    let raw = url.split(['?', '#']).next()?;
    let mut decoded = Vec::new();
    let mut bytes = raw.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            decoded.push((high * 16 + low) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let decoded = String::from_utf8(decoded).ok()?;
    if !decoded.starts_with('/') || decoded.contains(['\\', '\0']) {
        return None;
    }
    let relative = Path::new(decoded.trim_start_matches('/'));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let path = root
        .join(if relative.as_os_str().is_empty() {
            Path::new("index.html")
        } else {
            relative
        })
        .canonicalize()
        .ok()?;
    (path.starts_with(&root) && path.is_file()).then_some(path)
}
fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "mjs" | "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
pub fn opener_available() -> bool {
    if cfg!(target_os = "macos") {
        tools::find("open").is_some()
    } else if cfg!(windows) {
        tools::find("cmd").is_some()
    } else {
        tools::find("xdg-open").is_some()
    }
}
fn open_browser(url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new(tools::required("open")?);
        c.arg(url);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else {
        let mut c =
            Command::new(tools::required("xdg-open").context("No browser opener; use --no-open")?);
        c.arg(url);
        c
    };
    tools::run(&mut command).context("Could not open the browser; use --no-open to serve the URL")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_encoded_traversal_and_symlinks_outside_the_bundle() {
        let root = std::env::temp_dir().join(format!("omen-web-path-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("index.html"), "ok").unwrap();
        assert!(safe_path(&root, "/").is_some());
        assert!(safe_path(&root, "/%2e%2e/secret").is_none());
        assert!(safe_path(&root, "/%5csecret").is_none());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("outside")).unwrap();
            assert!(safe_path(&root, "/outside/index.html").is_none());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
