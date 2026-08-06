//! Steampunk Manifest (`.stkm`) parser — SPEC §17.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Default)]
pub struct Manifest {
    pub name: Option<String>,
    pub version: Option<String>,
    pub private: bool,
    pub description: Option<String>,
    pub entry: Option<String>,
    pub scripts: HashMap<String, String>,
    pub dependencies: Vec<DepSpec>,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DepSpec {
    pub name: String,
    pub version: String,
}

/// Parse a `.stkm` file from disk.
pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let src = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let mut m = parse_manifest(&src)?;
    m.path = path.to_path_buf();
    Ok(m)
}

/// Find `manager.stkm` walking up from `start`.
pub fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let cand = dir.join("manager.stkm");
        if cand.is_file() {
            return Some(cand);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub fn parse_manifest(src: &str) -> Result<Manifest> {
    let mut m = Manifest::default();
    let mut mode: Option<&str> = None;

    for (lineno, raw) in src.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if line == "scripts" {
            mode = Some("scripts");
            continue;
        }
        if line == "dependencies" {
            mode = Some("dependencies");
            continue;
        }

        if let Some(rest) = line.strip_prefix('.') {
            match mode {
                Some("scripts") => {
                    // .declare("name", "cmd")
                    let (name, cmd) = parse_declare(rest)
                        .with_context(|| format!("line {}: bad scripts.declare", lineno + 1))?;
                    m.scripts.insert(name, cmd);
                }
                Some("dependencies") => {
                    // .use("dep", version = "^1.0.0")
                    let dep = parse_use(rest)
                        .with_context(|| format!("line {}: bad dependencies.use", lineno + 1))?;
                    m.dependencies.push(dep);
                }
                None => bail!("line {}: '.' call outside scripts/dependencies block", lineno + 1),
                Some(other) => bail!("line {}: unknown block '{other}'", lineno + 1),
            }
            continue;
        }

        // key = value
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = parse_value(v.trim())
                .with_context(|| format!("line {}: bad value", lineno + 1))?;
            mode = None;
            match key {
                "name" => m.name = Some(val),
                "version" => m.version = Some(val),
                "description" => m.description = Some(val),
                "entry" => m.entry = Some(val),
                "private" => {
                    m.private = matches!(val.as_str(), "true" | "True" | "1");
                }
                _ => {
                    // ignore unknown metadata for forward compat
                }
            }
            continue;
        }

        bail!("line {}: unexpected content: {line}", lineno + 1);
    }

    Ok(m)
}

fn strip_comment(line: &str) -> &str {
    if let Some(i) = line.find("//") {
        &line[..i]
    } else {
        line
    }
}

fn parse_value(v: &str) -> Result<String> {
    if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
        return Ok(v[1..v.len() - 1].to_string());
    }
    if v == "true" || v == "false" {
        return Ok(v.to_string());
    }
    // bare token
    Ok(v.to_string())
}

fn parse_declare(rest: &str) -> Result<(String, String)> {
    let rest = rest.trim();
    let rest = rest
        .strip_prefix("declare")
        .ok_or_else(|| anyhow::anyhow!("expected declare"))?
        .trim();
    let rest = rest
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| anyhow::anyhow!("expected (...)"))?;
    let parts = split_args(rest)?;
    if parts.len() != 2 {
        bail!("declare expects 2 args");
    }
    Ok((unquote(&parts[0])?, unquote(&parts[1])?))
}

fn parse_use(rest: &str) -> Result<DepSpec> {
    let rest = rest.trim();
    let rest = rest
        .strip_prefix("use")
        .ok_or_else(|| anyhow::anyhow!("expected use"))?
        .trim();
    let rest = rest
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| anyhow::anyhow!("expected (...)"))?;
    let parts = split_args(rest)?;
    if parts.is_empty() {
        bail!("use expects package name");
    }
    let name = unquote(&parts[0])?;
    let mut version = "*".to_string();
    for p in &parts[1..] {
        let p = p.trim();
        if let Some(v) = p.strip_prefix("version") {
            let v = v.trim().strip_prefix('=').unwrap_or(v).trim();
            version = unquote(v)?;
        }
    }
    Ok(DepSpec { name, version })
}

fn split_args(s: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut quote = '"';
    for ch in s.chars() {
        if in_str {
            cur.push(ch);
            if ch == quote {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_str = true;
                quote = ch;
                cur.push(ch);
            }
            ',' => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    Ok(out)
}

fn unquote(s: &str) -> Result<String> {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        Ok(s[1..s.len() - 1].to_string())
    } else {
        Ok(s.to_string())
    }
}

/// Global deps cache root: `$STEAMPUNK_HOME/deps` or `~/.steampunk/deps`.
pub fn deps_cache_root() -> PathBuf {
    if let Ok(home) = std::env::var("STEAMPUNK_HOME") {
        return PathBuf::from(home).join("deps");
    }
    dirs_home()
        .map(|h| h.join(".steampunk").join("deps"))
        .unwrap_or_else(|| PathBuf::from(".steampunk/deps"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Resolve a concrete version directory for a dependency (exact match for MVP).
pub fn resolve_dep_dir(name: &str, version_req: &str) -> PathBuf {
    let ver = version_req.trim_start_matches('^').trim_start_matches('~');
    deps_cache_root().join(name).join(ver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample() {
        let src = r#"
name = "My App"
version = "1.0.0"
private = true
description = "demo"

scripts
    .declare("start", "steampunk run main.stk")
    .declare("build", "steampunk build main.stk --out build/app")

dependencies
    .use("dep1", version = "^1.0.1")
"#;
        let m = parse_manifest(src).unwrap();
        assert_eq!(m.name.as_deref(), Some("My App"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));
        assert!(m.private);
        assert_eq!(m.scripts.get("start").unwrap(), "steampunk run main.stk");
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "dep1");
        assert_eq!(m.dependencies[0].version, "^1.0.1");
    }
}
