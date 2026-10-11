//! Shared file helpers for the built-in tools.

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const SKIP_DIR_NAMES: &[&str] = &[".git", "target", "node_modules", ".hg", ".svn"];

pub fn req_str<'a>(input: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    input[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing '{key}'"))
}

pub fn opt_usize(input: &serde_json::Value, key: &str) -> Option<usize> {
    input[key]
        .as_u64()
        .map(|n| n as usize)
        .or_else(|| input[key].as_str().and_then(|s| s.parse().ok()))
}

pub fn opt_bool(input: &serde_json::Value, key: &str) -> bool {
    input[key].as_bool().unwrap_or(false)
}

pub fn resolve(path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub fn detect_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

pub fn to_ending(text: &str, ending: &str) -> String {
    let n = text.replace("\r\n", "\n");
    if ending == "\r\n" {
        n.replace('\n', "\r\n")
    } else {
        n
    }
}

pub fn strip_bom(bytes: &[u8]) -> (bool, &[u8]) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (true, &bytes[3..])
    } else {
        (false, bytes)
    }
}

pub fn is_binary(bytes: &[u8]) -> bool {
    let n = bytes.len().min(8_192);
    bytes[..n].contains(&0)
}

pub fn skip_dir(name: &str) -> bool {
    SKIP_DIR_NAMES.contains(&name)
}

pub fn did_you_mean(missing: &Path) -> Option<String> {
    let dir = missing.parent()?;
    let want = missing.file_name()?.to_string_lossy().to_ascii_lowercase();
    let mut hits: Vec<String> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l.contains(&want) || want.contains(&l)
        })
        .collect();
    if hits.is_empty() {
        return None;
    }
    hits.sort();
    Some(format!(
        "Did you mean:\n{}",
        hits.iter()
            .map(|h| format!("  {}", dir.join(h).display()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

pub fn read_text(path: &Path) -> Result<(bool, String), String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|mut f| f.read_to_end(&mut bytes))
        .map_err(|e| not_found_or(path, e))?;
    if is_binary(&bytes) {
        return Err(format!(
            "{} is binary; refuse to read as text",
            path.display()
        ));
    }
    let (bom, rest) = strip_bom(&bytes);
    let text = String::from_utf8_lossy(rest).into_owned();
    Ok((bom, text))
}

fn not_found_or(path: &Path, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        match did_you_mean(path) {
            Some(hint) => format!("{}: not found\n{hint}", path.display()),
            None => format!("{}: not found", path.display()),
        }
    } else {
        format!("{}: {e}", path.display())
    }
}

/// Write atomically: parent dirs, temp file, rename. Preserve UTF-8 BOM.
pub fn write_atomic(path: &Path, text: &str, bom: bool) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let mut body = Vec::new();
    if bom {
        body.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    body.extend_from_slice(text.as_bytes());
    let tmp = {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
    };
    fs::write(&tmp, &body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("rename onto {}: {e}", path.display())
    })
}

/// One numbered line, whole: a long line is never cut.
pub fn format_line(n: usize, line: &str) -> String {
    format!("{n}→{line}")
}

/// Identity of a directory: `(device, inode)` from one `metadata()` call.
#[cfg(unix)]
type DirId = (u64, u64);
#[cfg(not(unix))]
type DirId = PathBuf;

#[cfg(unix)]
fn dir_id(path: &Path) -> Option<DirId> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

#[cfg(not(unix))]
fn dir_id(path: &Path) -> Option<DirId> {
    fs::canonicalize(path).ok()
}

/// Every file under `root`, with no depth or count limit.
///
/// Real directories are always walked, each once, by the tree itself, before
/// any symlinked directory is looked at, so a symlink can never hide a real
/// directory. A symlinked directory is then followed at most once per target
/// (one global `(dev, ino)` set shared with the real walk): a target the walk
/// already covers, an ancestor of the link and a symlink cycle are all
/// skipped. Work is O(real directories + symlinks), however the links are
/// wired.
pub fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    let mut seen = HashSet::new();
    if let Some(id) = dir_id(root) {
        seen.insert(id);
    }
    let mut links = std::collections::VecDeque::new();
    walk_real(root, &mut seen, &mut links, &mut out);
    while let Some(link) = links.pop_front() {
        if let Some(id) = dir_id(&link)
            && seen.insert(id)
        {
            walk_real(&link, &mut seen, &mut links, &mut out);
        }
    }
    out
}

/// Walk `dir` (already in `seen`) and the real directories below it; queue
/// symlinked directories.
fn walk_real(
    dir: &Path,
    seen: &mut HashSet<DirId>,
    links: &mut std::collections::VecDeque<PathBuf>,
    out: &mut Vec<PathBuf>,
) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        match entry.file_type() {
            Ok(t) if t.is_dir() => {
                if !skip_dir(&name) && dir_id(&path).is_none_or(|id| seen.insert(id)) {
                    walk_real(&path, seen, links, out);
                }
            }
            Ok(t) if t.is_symlink() => {
                if path.is_dir() {
                    if !skip_dir(&name) {
                        links.push_back(path);
                    }
                } else if path.is_file() {
                    out.push(path);
                }
            }
            Ok(t) if t.is_file() => out.push(path),
            _ => {}
        }
    }
}

/// Glob with `*`, `?`, and `**`. `/` is the only separator (normalized).
pub fn glob_match(pat: &str, path: &str) -> bool {
    let pat = pat.replace('\\', "/");
    let path = path.replace('\\', "/");
    glob_rec(pat.as_bytes(), path.as_bytes())
}

fn glob_rec(pat: &[u8], path: &[u8]) -> bool {
    if pat.is_empty() {
        return path.is_empty();
    }
    if pat.starts_with(b"**/") {
        if glob_rec(&pat[3..], path) {
            return true;
        }
        for i in 0..=path.len() {
            if (i == 0 || path[i - 1] == b'/') && glob_rec(&pat[3..], &path[i..]) {
                return true;
            }
        }
        return glob_rec(&pat[3..], b"");
    }
    if pat == b"**" {
        return true;
    }
    match pat.first().copied() {
        Some(b'*') => {
            let rest = &pat[1..];
            if rest.starts_with(b"*") {
                return glob_rec(&pat[1..], path);
            }
            if path.is_empty() {
                return glob_rec(rest, path);
            }
            for i in 0..=path.len() {
                if path.get(i).copied() == Some(b'/') {
                    break;
                }
                if glob_rec(rest, &path[i..]) {
                    return true;
                }
            }
            glob_rec(
                rest,
                &path[path.iter().position(|&c| c == b'/').unwrap_or(path.len())..],
            ) || (path.iter().all(|&c| c != b'/') && glob_rec(rest, b""))
        }
        Some(b'?') => {
            if path.first().copied().is_some_and(|c| c != b'/') {
                glob_rec(&pat[1..], &path[1..])
            } else {
                false
            }
        }
        Some(c) => path.first().copied() == Some(c) && glob_rec(&pat[1..], &path[1..]),
        None => path.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_star_and_double() {
        assert!(glob_match("*.rs", "tools.rs"));
        assert!(!glob_match("*.rs", "src/tools.rs"));
        assert!(glob_match("**/*.rs", "src/tools.rs"));
        assert!(glob_match("src/**", "src/tools/edit.rs"));
        assert!(glob_match("src/tools/?.rs", "src/tools/a.rs"));
    }
}
