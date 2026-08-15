use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const BANNER: &str = "n/ OODP node serving at port ";

pub struct ServedNode {
    pub child: Child,
    pub port: u16,
    pub log: PathBuf,
}

/// Start an OODP node without reserving or guessing a port. The child asks
/// the OS for one, then its own banner is the authoritative address.
pub fn serve(mut command: Command, log: impl AsRef<Path>) -> ServedNode {
    let log = log.as_ref().to_path_buf();
    let file = fs::File::create(&log).unwrap();
    let mut child = command
        .args(["node", "serve", "--port", "0"])
        .stdout(Stdio::from(file.try_clone().unwrap()))
        .stderr(Stdio::from(file))
        .spawn()
        .unwrap();

    for _ in 0..60 {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("`oo node serve` exited ({status}): {}", read_log(&log));
        }
        if let Some(port) = banner_port(&read_log(&log)) {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return ServedNode { child, port, log };
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("`oo node serve` never came up: {}", read_log(&log));
}

pub fn read_log(log: &Path) -> String {
    fs::read_to_string(log).unwrap_or_default()
}

fn banner_port(text: &str) -> Option<u16> {
    let i = text.find(BANNER)?;
    let digits: String = text[i + BANNER.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}
