mod loader;
mod stkm;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use stk_codegen::{build_executable, jit_run};
use stk_types::typecheck;

#[derive(Parser)]
#[command(name = "steampunk", version, about = "Steampunk language compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile in memory (JIT) and run
    Run {
        /// Source file (default: main.stk or manager.stkm entry)
        file: Option<PathBuf>,
    },
    /// Compile to a native binary
    Build {
        /// Source file (default: main.stk)
        file: Option<PathBuf>,
        /// Output path
        #[arg(long, short = 'o')]
        out: Option<PathBuf>,
    },
    /// Resolve dependencies from manager.stkm into the global cache
    Deps {
        /// Project directory (default: .)
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Run a script declared in manager.stkm (`steampunk script start`)
    Script {
        /// Script name from scripts.declare
        name: String,
        /// Project directory
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Run `*_test.stk` / `fn test_*` style tests under a directory
    Test {
        /// Directory to search (default: .)
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Format `.stk` sources (MVP: normalize trailing whitespace / ensure final newline)
    Fmt {
        /// Files or directories
        paths: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    if let Err(e) = real_main() {
        eprintln!("error: {e:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { file } => {
            let path = resolve_entry(file)?;
            let checked = compile_file(&path)?;
            jit_run(&checked)?;
        }
        Commands::Build { file, out } => {
            let path = resolve_entry(file)?;
            let checked = compile_file(&path)?;
            let out = out.unwrap_or_else(|| default_out_path(&path));
            build_executable(&checked, &out)?;
            eprintln!("wrote {}", out.display());
        }
        Commands::Deps { dir } => cmd_deps(&dir)?,
        Commands::Script { name, dir } => cmd_script(&dir, &name)?,
        Commands::Test { dir } => cmd_test(&dir)?,
        Commands::Fmt { paths } => cmd_fmt(&paths)?,
    }
    Ok(())
}

fn resolve_entry(file: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(f) = file {
        return Ok(f);
    }
    if Path::new("main.stk").is_file() {
        return Ok(PathBuf::from("main.stk"));
    }
    if let Some(m) = stkm::find_manifest(Path::new(".")) {
        let man = stkm::load_manifest(&m)?;
        if let Some(entry) = man.entry {
            let p = m.parent().unwrap_or(Path::new(".")).join(entry);
            return Ok(p);
        }
    }
    Ok(PathBuf::from("main.stk"))
}

fn default_out_path(src: &Path) -> PathBuf {
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app");
    PathBuf::from("build").join(stem)
}

fn compile_file(path: &Path) -> Result<stk_types::CheckedProgram> {
    let (program, entry_module) = loader::load_project(path)?;
    let file = path.display().to_string();
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let checked = match typecheck(&program, &entry_module) {
        Ok(c) => c,
        Err(d) => bail!("{}", d.format_with_source(&source, &file)),
    };
    Ok(checked)
}

fn cmd_deps(dir: &Path) -> Result<()> {
    let man_path = stkm::find_manifest(dir)
        .ok_or_else(|| anyhow::anyhow!("no manager.stkm found under {}", dir.display()))?;
    let man = stkm::load_manifest(&man_path)?;
    let cache = stkm::deps_cache_root();
    std::fs::create_dir_all(&cache)?;
    eprintln!("deps cache: {}", cache.display());
    if man.dependencies.is_empty() {
        eprintln!("no dependencies declared");
        return Ok(());
    }
    let registry = std::env::var("STEAMPUNK_REGISTRY").ok().map(PathBuf::from);
    for dep in &man.dependencies {
        let dest = stkm::resolve_dep_dir(&dep.name, &dep.version);
        if dest.join(format!("{}.stkb", dep.name)).is_file()
            && dest.join(format!("{}.stkmap", dep.name)).is_file()
        {
            eprintln!("ok {}@{} (cached)", dep.name, dep.version);
            continue;
        }
        if let Some(reg) = &registry {
            let ver = dep.version.trim_start_matches('^').trim_start_matches('~');
            let src = reg.join(&dep.name).join(ver);
            if src.is_dir() {
                std::fs::create_dir_all(&dest)?;
                for ent in std::fs::read_dir(&src)? {
                    let ent = ent?;
                    let to = dest.join(ent.file_name());
                    std::fs::copy(ent.path(), &to)?;
                }
                eprintln!("fetched {}@{} from registry", dep.name, dep.version);
                continue;
            }
        }
        // Local stub: create placeholder .stkmap so tooling can proceed
        std::fs::create_dir_all(&dest)?;
        let map = dest.join(format!("{}.stkmap", dep.name));
        let stkb = dest.join(format!("{}.stkb", dep.name));
        if !map.exists() {
            std::fs::write(
                &map,
                format!(
                    "{{\n  \"name\": \"{}\",\n  \"version\": \"{}\",\n  \"exports\": []\n}}\n",
                    dep.name, dep.version
                ),
            )?;
        }
        if !stkb.exists() {
            std::fs::write(&stkb, b"")?;
        }
        eprintln!(
            "warn: {}@{} not in registry; wrote stub into {}",
            dep.name,
            dep.version,
            dest.display()
        );
    }
    Ok(())
}

fn cmd_script(dir: &Path, name: &str) -> Result<()> {
    let man_path = stkm::find_manifest(dir)
        .ok_or_else(|| anyhow::anyhow!("no manager.stkm found"))?;
    let man = stkm::load_manifest(&man_path)?;
    let cmd = man
        .scripts
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("unknown script '{name}'"))?;
    eprintln!("+ {cmd}");
    let status = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(man_path.parent().unwrap_or(dir))
        .status()?;
    if !status.success() {
        bail!("script '{name}' failed with {status}");
    }
    Ok(())
}

fn cmd_test(dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    collect_test_files(dir, &mut files)?;
    if files.is_empty() {
        eprintln!("no *_test.stk files under {}", dir.display());
        return Ok(());
    }
    let mut failed = 0;
    for f in &files {
        eprint!("test {} ... ", f.display());
        match compile_file(f).and_then(|c| jit_run(&c).map_err(|e| e.into())) {
            Ok(()) => eprintln!("ok"),
            Err(e) => {
                eprintln!("FAILED\n{e:#}");
                failed += 1;
            }
        }
    }
    if failed > 0 {
        bail!("{failed} test file(s) failed");
    }
    Ok(())
}

fn collect_test_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if dir.is_file() {
        if dir
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with("_test.stk"))
        {
            out.push(dir.to_path_buf());
        }
        return Ok(());
    }
    for ent in std::fs::read_dir(dir)? {
        let ent = ent?;
        let p = ent.path();
        if p.is_dir() {
            if p.file_name().and_then(|s| s.to_str()) == Some("target") {
                continue;
            }
            collect_test_files(&p, out)?;
        } else if p
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.ends_with("_test.stk"))
        {
            out.push(p);
        }
    }
    Ok(())
}

fn cmd_fmt(paths: &[PathBuf]) -> Result<()> {
    let paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.to_vec()
    };
    let mut files = Vec::new();
    for p in &paths {
        collect_stk_files(p, &mut files)?;
    }
    for f in files {
        let src = std::fs::read_to_string(&f)?;
        let formatted = format_stk(&src);
        if formatted != src {
            std::fs::write(&f, formatted)?;
            eprintln!("formatted {}", f.display());
        }
    }
    Ok(())
}

fn collect_stk_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if dir.is_file() {
        if dir.extension().and_then(|s| s.to_str()) == Some("stk") {
            out.push(dir.to_path_buf());
        }
        return Ok(());
    }
    for ent in std::fs::read_dir(dir)? {
        let ent = ent?;
        let p = ent.path();
        if p.is_dir() {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }
            collect_stk_files(&p, out)?;
        } else if p.extension().and_then(|s| s.to_str()) == Some("stk") {
            out.push(p);
        }
    }
    Ok(())
}

/// MVP formatter: trim trailing whitespace per line, ensure final newline, collapse 3+ blank lines.
fn format_stk(src: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in src.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank += 1;
            if blank <= 2 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    if out.is_empty() {
        out.push('\n');
    }
    out
}
