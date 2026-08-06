use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use stk_ast::Program;
use stk_parser::parse;
use stk_span::Diagnostic;

/// Roots used to resolve `@import ":path"`.
struct ImportRoots {
    /// Directory containing the entry `.stk` (tried first).
    entry_dir: PathBuf,
    /// Ancestor with `manager.stkm`, or the same as `entry_dir`.
    package_root: PathBuf,
}

/// Load entry file and all `@import ":path"` modules into one Program.
pub fn load_project(entry: &Path) -> Result<(Program, String)> {
    let entry = entry
        .canonicalize()
        .with_context(|| format!("canonicalize {}", entry.display()))
        .unwrap_or_else(|_| entry.to_path_buf());
    let roots = import_roots(&entry);
    let entry_key = module_key(&entry);

    let mut loaded: HashMap<String, Program> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut sources: HashMap<String, String> = HashMap::new();

    load_module(
        &entry,
        &entry_key,
        &roots,
        &mut loaded,
        &mut order,
        &mut stack,
        &mut sources,
    )?;

    let mut merged = Program {
        imports: Vec::new(),
        constants: Vec::new(),
        functions: Vec::new(),
        classes: Vec::new(),
        iclasses: Vec::new(),
    };
    let mut has_std = false;

    for key in &order {
        let prog = loaded.get(key).unwrap();
        for imp in &prog.imports {
            if imp.path == "std" {
                has_std = true;
            } else if !imp.path.starts_with(':') {
                bail!(
                    "{}: unknown import '{}' (MVP: \"std\" or \":path\")",
                    key,
                    imp.path
                );
            }
        }
        for mut c in prog.constants.clone() {
            c.module = key.clone();
            merged.constants.push(c);
        }
        for mut f in prog.functions.clone() {
            f.module = key.clone();
            merged.functions.push(f);
        }
        for mut c in prog.classes.clone() {
            c.module = key.clone();
            merged.classes.push(c);
        }
        for mut i in prog.iclasses.clone() {
            i.module = key.clone();
            merged.iclasses.push(i);
        }
    }

    if has_std {
        merged.imports.push(stk_ast::Import {
            path: "std".into(),
            span: stk_span::Span::dummy(),
        });
    }

    Ok((merged, entry_key))
}

fn import_roots(entry: &Path) -> ImportRoots {
    let entry_dir = entry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut dir = entry_dir.clone();
    let package_root = loop {
        if dir.join("manager.stkm").is_file() {
            break dir;
        }
        if !dir.pop() {
            break entry_dir.clone();
        }
    };
    ImportRoots {
        entry_dir,
        package_root,
    }
}

fn module_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn with_stk_ext(mut p: PathBuf) -> PathBuf {
    if p.extension().is_none() {
        p.set_extension("stk");
    }
    p
}

/// Resolve `:path` — prefer entry directory, then package root (`manager.stkm`).
fn resolve_import(roots: &ImportRoots, import_path: &str) -> PathBuf {
    let rel = import_path.trim_start_matches(':');
    let from_entry = with_stk_ext(roots.entry_dir.join(rel));
    if from_entry.is_file() {
        return from_entry;
    }
    with_stk_ext(roots.package_root.join(rel))
}

fn load_module(
    path: &Path,
    key: &str,
    roots: &ImportRoots,
    loaded: &mut HashMap<String, Program>,
    order: &mut Vec<String>,
    stack: &mut Vec<String>,
    sources: &mut HashMap<String, String>,
) -> Result<()> {
    if loaded.contains_key(key) {
        return Ok(());
    }
    if stack.iter().any(|s| s == key) {
        bail!("import cycle involving '{key}'");
    }
    stack.push(key.to_string());

    let source = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    sources.insert(key.to_string(), source.clone());
    let program = match parse(&source) {
        Ok(p) => p,
        Err(d) => bail!("{}", format_diag(&d, &source, key)),
    };

    let mut local_imports = Vec::new();
    for imp in &program.imports {
        if imp.path.starts_with(':') {
            local_imports.push(imp.path.clone());
        } else if imp.path != "std" {
            bail!(
                "{}: unknown import '{}' (MVP: \"std\" or \":path\")",
                key,
                imp.path
            );
        }
    }

    for imp_path in local_imports {
        let child = resolve_import(roots, &imp_path);
        let child_key = module_key(
            &child
                .canonicalize()
                .unwrap_or_else(|_| child.clone()),
        );
        if !child.exists() {
            bail!(
                "{}: module file not found for import '{}'",
                key,
                imp_path
            );
        }
        load_module(
            &child,
            &child_key,
            roots,
            loaded,
            order,
            stack,
            sources,
        )?;
    }

    loaded.insert(key.to_string(), program);
    order.push(key.to_string());
    stack.pop();
    Ok(())
}

fn format_diag(d: &Diagnostic, source: &str, file: &str) -> String {
    d.format_with_source(source, file)
}
