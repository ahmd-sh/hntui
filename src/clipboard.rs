// Copy text to the system clipboard. Locally that's the platform's own tool
// (pbcopy, wl-copy, xclip, xsel). Over SSH, or when none is around, it's
// OSC 52: an escape sequence asking the terminal itself to set the
// clipboard, which most modern terminals honour (some only once enabled).

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub enum Via {
    Tool,
    Terminal,
}

pub fn copy(text: &str) -> Via {
    let remote = std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some();
    if !remote && tools().into_iter().any(|(cmd, args)| run(cmd, args, text)) {
        return Via::Tool;
    }
    osc52(text);
    Via::Terminal
}

fn tools() -> Vec<(&'static str, &'static [&'static str])> {
    if cfg!(target_os = "macos") {
        return vec![("pbcopy", &[])];
    }
    let mut v: Vec<(&'static str, &'static [&'static str])> = vec![];
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        v.push(("wl-copy", &[]));
    }
    if std::env::var_os("DISPLAY").is_some() {
        v.push(("xclip", &["-selection", "clipboard"]));
        v.push(("xsel", &["--clipboard", "--input"]));
    }
    v
}

// true if the tool ran and took the text
fn run(cmd: &str, args: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    } // stdin closes here, which is the tool's cue to finish
    // xclip and wl-copy fork a server and return at once; don't hang the UI
    // on one that doesn't
    let deadline = Instant::now() + Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => return true,
        }
    }
}

fn osc52(text: &str) {
    let seq = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
    let mut out = std::io::stdout();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64("héllo ✓".as_bytes()), "aMOpbGxvIOKckw==");
    }
}
