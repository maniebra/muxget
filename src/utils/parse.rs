use std::io::Read;

use crate::models::download::Progress;

/// `[#8a1f2b 12MiB/100MiB(12%) CN:1 SD:2 DL:5.0MiB UL:1.0MiB(30MiB) ETA:17s]`.
/// `SD` and `UL` only appear for torrents, and a torrent prints no `(12%)`
/// until its metadata arrives — the sizes are the reliable part.
pub fn aria2(line: &str) -> Option<Progress> {
    let mut tokens = line.strip_prefix("[#")?.split_whitespace();
    tokens.next()?;
    let (done, rest) = tokens.next()?.trim_start_matches("SIZE:").split_once('/')?;
    let (total, percent) = match rest.split_once('(') {
        Some((total, pct)) => (total, pct.trim_end_matches([')', '%']).parse().ok()),
        None => (rest, None),
    };
    let percent = percent
        .or_else(|| {
            let (d, t) = (bytes(done)?, bytes(total)?);
            (t > 0.0).then_some((d / t * 100.0) as f32)
        })
        .unwrap_or(0.0);
    Some(Progress {
        percent,
        done: done.to_string(),
        total: total.to_string(),
        speed: field(line, "DL:").unwrap_or_default(),
        eta: field(line, "ETA:").unwrap_or_default(),
        // `UL:` carries the rate and, in brackets, the session total.
        upload: field(line, "UL:")
            .map(|v| v.split('(').next().unwrap_or_default().to_string())
            .unwrap_or_default(),
        uploaded: field(line, "UL:")
            .and_then(|v| Some(v.split_once('(')?.1.trim_end_matches(')').to_string()))
            .unwrap_or_default(),
        peers: number(line, "CN:").unwrap_or(0),
        seeders: number(line, "SD:"),
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
        ..Default::default()
    })
}

/// `5.0MiB`, `1.2GiB/s`, `512KB` -> bytes. Both tools use binary prefixes.
pub fn bytes(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches("/s");
    let split = s.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(s.len());
    let n: f64 = s[..split].parse().ok()?;
    let scale = match s[split..].trim().chars().next().map(|c| c.to_ascii_uppercase()) {
        Some('K') => 1024.0,
        Some('M') => 1024f64.powi(2),
        Some('G') => 1024f64.powi(3),
        Some('T') => 1024f64.powi(4),
        _ => 1.0,
    };
    Some(n * scale)
}

/// `5.0MiB` for 5242880.
pub fn human(n: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut n = n;
    let mut unit = 0;
    while n >= 1024.0 && unit < UNITS.len() - 1 {
        n /= 1024.0;
        unit += 1;
    }
    format!("{n:.1}{}", UNITS[unit])
}

fn number(line: &str, key: &str) -> Option<u32> {
    field(line, key)?.parse().ok()
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

/// The output file a backend just named, from either tool's chatter.
pub fn destination(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("[download] Destination:") {
        return Some(rest.trim().to_string());
    }
    if let Some(rest) = line.strip_prefix("[Merger] Merging formats into") {
        return Some(rest.trim().trim_matches('"').to_string());
    }
    if let Some(rest) = line.strip_prefix("[download] ") {
        return rest.strip_suffix(" has already been downloaded").map(str::to_string);
    }
    // aria2c's result table: `gid|OK  |speed|path`.
    if line.contains("|OK") && line.matches('|').count() >= 3 {
        return Some(line.rsplit('|').next()?.trim().to_string());
    }
    None
}
