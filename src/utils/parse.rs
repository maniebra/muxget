use std::io::Read;

use crate::models::download::Progress;

/// `[#8a1f2b 12MiB/100MiB(12%) CN:1 DL:5.0MiB ETA:17s]`
pub fn aria2(line: &str) -> Option<Progress> {
    let pct = line.split_once('(')?.1;
    let pct: f32 = pct.split_once('%')?.0.parse().ok()?;
    Some(Progress {
        percent: pct,
        speed: field(line, "DL:").unwrap_or_default(),
        eta: field(line, "ETA:").unwrap_or_default(),
    })
}

/// `[download]  12.3% of   10.00MiB at    5.00MiB/s ETA 00:20`
pub fn ytdlp(line: &str) -> Option<Progress> {
    if !line.starts_with("[download]") {
        return None;
    }
    let mut tokens = line.split_whitespace();
    let pct = tokens.find(|t| t.ends_with('%'))?;
    let pct: f32 = pct.trim_end_matches('%').parse().ok()?;
    let mut tokens = line.split_whitespace().skip_while(|t| *t != "at").skip(1);
    let speed = tokens.next().unwrap_or("").to_string();
    let eta = line
        .split_whitespace()
        .skip_while(|t| *t != "ETA")
        .nth(1)
        .unwrap_or("")
        .to_string();
    Some(Progress {
        percent: pct,
        speed,
        eta,
    })
}

/// Value after `key`, up to the next space or `]`.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.split_once(key)?.1;
    let end = rest.find([' ', ']']).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Both tools redraw progress with `\r`, so `BufRead::lines` would stall.
pub fn for_each_line(mut r: impl Read, mut f: impl FnMut(&str)) {
    let mut buf = [0u8; 4096];
    let mut cur = String::new();
    while let Ok(n) = r.read(&mut buf) {
        if n == 0 {
            break;
        }
        cur.push_str(&String::from_utf8_lossy(&buf[..n]));
        while let Some(i) = cur.find(['\r', '\n']) {
            let line = cur[..i].trim().to_string();
            cur.drain(..i + 1);
            if !line.is_empty() {
                f(&line);
            }
        }
    }
    let last = cur.trim();
    if !last.is_empty() {
        f(last);
    }
}
