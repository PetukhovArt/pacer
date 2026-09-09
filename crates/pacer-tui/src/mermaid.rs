//! Mermaid diagrams in the tree browser preview, rendered to text art by an
//! external command (`mermaid-ascii -f -` by default, the `mermaid_renderer`
//! setting). One entry point, [`render`]: the preview text goes in, the text
//! with every mermaid block replaced by its art comes out, plus the line
//! ranges the highlighter must leave alone. Extraction rules, the subprocess
//! call, the cache and the fallback all live here so the tree browser never
//! learns what a fence is.

use pacer_core::spawn::NoWindow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::ops::Range;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_RENDERER: &str = "mermaid-ascii -f -";

/// How long one diagram may take before the block stays as source.
const TIMEOUT: Duration = Duration::from_secs(3);
/// Rendered blocks kept; the cache is emptied past this, not evicted.
const CACHE_CAP: usize = 256;

pub struct Rendered {
    /// The preview text with every block replaced by its rendering.
    pub text: String,
    /// Line ranges of `text` that are art (or the install hint), which the
    /// syntax highlighter must not colour.
    pub plain: Vec<Range<usize>>,
}

/// Replace every mermaid block in `text` with its rendering. `command` is
/// the renderer program plus arguments, whitespace-separated; empty means
/// off. A missing renderer leaves the text as it is, with one hint line
/// above the first block; a failing one puts its stderr in place of the
/// block; a hanging one is killed and the block stays exactly as written.
pub fn render(text: &str, path: &Path, command: &str) -> Rendered {
    let unchanged = || Rendered {
        text: text.to_string(),
        plain: Vec::new(),
    };
    let command = command.trim();
    if command.is_empty() {
        return unchanged();
    }
    let lines: Vec<&str> = text.lines().collect();
    let blocks = extract(&lines, path);
    if blocks.is_empty() {
        return unchanged();
    }
    let Some(argv) = available(command) else {
        let mut out = lines.clone();
        let hint = format!(
            "(install `{}` to render mermaid diagrams here)",
            command.split_whitespace().next().unwrap_or(command)
        );
        let at = blocks[0].lines.start;
        out.insert(at, &hint);
        return Rendered {
            text: out.join("\n"),
            plain: std::iter::once(at..at + 1).collect(),
        };
    };

    let mut out: Vec<String> = Vec::new();
    let mut plain = Vec::new();
    let mut cursor = 0;
    for block in &blocks {
        out.extend(
            lines[cursor..block.lines.start]
                .iter()
                .map(|l| l.to_string()),
        );
        let art = match cached(&argv, &block.source) {
            Ok(art) => art,
            Err(RenderError::Failed(msg)) => msg,
            Err(RenderError::TimedOut) => {
                out.extend(lines[block.lines.clone()].iter().map(|l| l.to_string()));
                cursor = block.lines.end;
                continue;
            }
        };
        let start = out.len();
        out.extend(art.lines().map(|l| l.replace('\t', "    ")));
        plain.push(start..out.len());
        cursor = block.lines.end;
    }
    out.extend(lines[cursor..].iter().map(|l| l.to_string()));
    Rendered {
        text: out.join("\n"),
        plain,
    }
}

struct Block {
    /// Lines of the input the block occupies, fences included.
    lines: Range<usize>,
    source: String,
}

fn extract(lines: &[&str], path: &Path) -> Vec<Block> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("mmd" | "mermaid") if !lines.is_empty() => vec![Block {
            lines: 0..lines.len(),
            source: lines.join("\n"),
        }],
        Some("md" | "markdown") => fenced(lines),
        _ => Vec::new(),
    }
}

/// Fenced blocks whose info string is exactly `mermaid`: a fence of three
/// or more backticks or tildes, indented by at most three spaces, closed by
/// a fence of the same character at least as long.
fn fenced(lines: &[&str]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((ch, len, info)) = fence(lines[i]) else {
            i += 1;
            continue;
        };
        if info != "mermaid" {
            // Skip to the closing fence so a mermaid fence inside another
            // code block is not taken for one.
            i += 1;
            while i < lines.len() && !closes(lines[i], ch, len) {
                i += 1;
            }
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        let body_start = i;
        while i < lines.len() && !closes(lines[i], ch, len) {
            i += 1;
        }
        if i == lines.len() {
            break; // Unclosed: leave as source.
        }
        blocks.push(Block {
            lines: start..i + 1,
            source: lines[body_start..i].join("\n"),
        });
        i += 1;
    }
    blocks
}

fn fence(line: &str) -> Option<(char, usize, &str)> {
    let stripped = line.trim_start_matches(' ');
    if line.len() - stripped.len() > 3 {
        return None;
    }
    let ch = stripped.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = stripped.chars().take_while(|c| *c == ch).count();
    if len < 3 {
        return None;
    }
    let info = stripped[len..].trim();
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some((ch, len, info))
}

fn closes(line: &str, ch: char, len: usize) -> bool {
    matches!(fence(line), Some((c, l, info)) if c == ch && l >= len && info.is_empty())
}

#[derive(Clone)]
enum RenderError {
    Failed(String),
    TimedOut,
}

/// Every outcome is remembered, timeouts included: a hanging diagram costs
/// its `TIMEOUT` once per session, not once per visit.
static CACHE: LazyLock<Mutex<HashMap<u64, Result<String, RenderError>>>> =
    LazyLock::new(Default::default);

fn cached(argv: &[String], source: &str) -> Result<String, RenderError> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    argv.hash(&mut hasher);
    source.hash(&mut hasher);
    let key = hasher.finish();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = cache.get(&key) {
        return hit.clone();
    }
    let result = run(argv, source);
    if cache.len() >= CACHE_CAP {
        cache.clear();
    }
    cache.insert(key, result.clone());
    result
}

static RESOLVED: LazyLock<Mutex<HashMap<String, Option<String>>>> = LazyLock::new(Default::default);

/// The command as argv with the program resolved, or `None` when it cannot
/// be found. Probed once per command per process, so a missing renderer
/// costs one lookup.
fn available(command: &str) -> Option<Vec<String>> {
    let mut argv: Vec<String> = command.split_whitespace().map(str::to_string).collect();
    let mut known = RESOLVED.lock().unwrap_or_else(|e| e.into_inner());
    let program = known
        .entry(command.to_string())
        .or_insert_with(|| resolve(&argv[0]));
    argv[0] = program.clone()?;
    Some(argv)
}

#[cfg(windows)]
fn resolve(program: &str) -> Option<String> {
    pacer_core::spawn::resolve_program(program).map(|p| p.to_string_lossy().into_owned())
}

#[cfg(unix)]
fn resolve(program: &str) -> Option<String> {
    let p = Path::new(program);
    if p.components().count() > 1 {
        return p.is_file().then(|| program.to_string());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .any(|dir| dir.join(program).is_file())
        .then(|| program.to_string())
}

/// Feed `source` to the renderer's stdin and collect stdout. Output is
/// drained on threads so a chatty renderer cannot block on a full pipe
/// while we wait on it.
fn run(argv: &[String], source: &str) -> Result<String, RenderError> {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_window()
        .spawn()
        .map_err(|e| RenderError::Failed(format!("(mermaid renderer failed to start: {e})")))?;
    let mut stdin = child.stdin.take().expect("piped");
    let source = source.to_string();
    std::thread::spawn(move || {
        let _ = stdin.write_all(source.as_bytes());
    });
    fn drain(mut pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(p) = pipe.as_mut() {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RenderError::TimedOut);
            }
            Err(e) => return Err(RenderError::Failed(format!("(mermaid renderer: {e})"))),
        }
    };
    let out = String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned();
    if status.success() {
        Ok(out)
    } else {
        let err = String::from_utf8_lossy(&stderr.join().unwrap_or_default()).into_owned();
        let msg = if err.trim().is_empty() { out } else { err };
        Err(RenderError::Failed(format!(
            "(mermaid renderer exited with {status})\n{}",
            msg.trim_end()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A renderer that prefixes every stdin line with `art:`. No spaces in
    /// the script, since the setting is split on whitespace.
    fn stub_renderer() -> &'static str {
        #[cfg(unix)]
        {
            "sed s/^/art:/"
        }
        #[cfg(windows)]
        {
            "powershell.exe -NoProfile -NonInteractive -Command $input|%{\"art:\"+$_}"
        }
    }

    const DOC: &str = "\
# Title

```mermaid
graph TD
A-->B
```

```rust
let x = 1;
```

    ```mermaid
    indented, a code block not a fence
    ```

~~~mermaid
sequenceDiagram
~~~
end";

    #[test]
    fn extraction_finds_only_mermaid_fences() {
        let lines: Vec<&str> = DOC.lines().collect();
        let blocks = fenced(&lines);
        let got: Vec<(Range<usize>, &str)> = blocks
            .iter()
            .map(|b| (b.lines.clone(), b.source.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![(2..6, "graph TD\nA-->B"), (15..18, "sequenceDiagram")]
        );
        let whole = extract(&["graph LR"], Path::new("d.mmd"));
        assert_eq!(whole.len(), 1);
        assert_eq!(whole[0].source, "graph LR");
        assert!(extract(&lines, Path::new("d.txt")).is_empty());
    }

    #[test]
    fn art_replaces_blocks_and_ranges_cover_exactly_the_art() {
        let r = render(DOC, Path::new("README.md"), stub_renderer());
        let out: Vec<&str> = r.text.lines().collect();
        assert_eq!(out[0..2], ["# Title", ""]);
        assert_eq!(out[2..4], ["art:graph TD", "art:A-->B"]);
        assert_eq!(
            out[4..],
            [
                "",
                "```rust",
                "let x = 1;",
                "```",
                "",
                "    ```mermaid",
                "    indented, a code block not a fence",
                "    ```",
                "",
                "art:sequenceDiagram",
                "end"
            ]
        );
        assert_eq!(r.plain, vec![2..4, 13..14]);
        for range in &r.plain {
            assert!(out[range.clone()].iter().all(|l| l.starts_with("art:")));
        }
    }

    #[test]
    fn missing_renderer_keeps_the_source_and_adds_one_hint() {
        let r = render(
            DOC,
            Path::new("README.md"),
            "pacer-no-such-renderer-xyz --flag",
        );
        let out: Vec<&str> = r.text.lines().collect();
        let src: Vec<&str> = DOC.lines().collect();
        assert_eq!(out.len(), src.len() + 1);
        assert_eq!(
            out[2],
            "(install `pacer-no-such-renderer-xyz` to render mermaid diagrams here)"
        );
        assert_eq!(out[..2], src[..2]);
        assert_eq!(out[3..], src[2..]);
        assert_eq!(r.plain, vec![2..3]);

        let off = render(DOC, Path::new("README.md"), "");
        assert_eq!(off.text, DOC);
        assert!(off.plain.is_empty());
    }
}
