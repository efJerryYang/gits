use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Parser, ValueEnum};
use pathdiff::diff_paths;
use terminal_size::{terminal_size, Width};

#[derive(Parser, Debug)]
#[command(
    name = "gits",
    version,
    about = "Bulk git wrapper for multi-repo workspaces",
    disable_help_subcommand = true
)]
struct Cli {
    /// Search root (defaults to current directory)
    #[arg(long)]
    root: Option<PathBuf>,

    /// Print absolute headings instead of relative paths
    #[arg(long)]
    absolute_path: bool,

    /// Include ancestor repositories from cwd up to filesystem root
    #[arg(long)]
    parent: bool,

    /// Limit child search depth (0 = only root). Omit for unlimited.
    #[arg(long)]
    max_depth: Option<usize>,

    /// List discovered repositories without executing git
    #[arg(long)]
    list: bool,

    /// Heading style for repository separators
    #[arg(long, value_enum, default_value_t = HeadingStyle::Rule)]
    heading_style: HeadingStyle,

    /// Color mode for headings
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,

    /// Suppress headings entirely (even for multiple repos)
    #[arg(long)]
    no_heading: bool,

    /// Git command and args (first token is the git subcommand)
    #[arg(value_name = "GIT_ARGS", trailing_var_arg = true)]
    git_args: Vec<OsString>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum HeadingStyle {
    Plain,
    Rule,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

fn is_git_repo_dir(dir: &Path) -> bool {
    let git_path = dir.join(".git");
    match fs::symlink_metadata(&git_path) {
        Ok(meta) => meta.is_dir() || meta.is_file(),
        Err(_) => false,
    }
}

fn ancestors_with_git(mut dir: PathBuf) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    loop {
        if is_git_repo_dir(&dir) {
            repos.push(dir.clone());
        }
        if !dir.pop() {
            break;
        }
    }
    repos
}

fn discover_children(root: &Path, max_depth: Option<usize>) -> io::Result<Vec<PathBuf>> {
    let mut repos = Vec::new();
    fn walk(
        dir: &Path,
        depth: usize,
        max_depth: Option<usize>,
        out: &mut Vec<PathBuf>,
    ) -> io::Result<()> {
        if is_git_repo_dir(dir) {
            out.push(dir.to_path_buf());
            return Ok(()); // first occurrence rule: do not descend further
        }

        if let Some(max) = max_depth {
            if depth >= max {
                return Ok(());
            }
        }

        let entries = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                // Skip unreadable directories silently.
                if e.kind() == io::ErrorKind::PermissionDenied {
                    return Ok(());
                }
                return Err(e);
            }
        };
        // Collect and sort for deterministic order
        let mut dirs = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                dirs.push(entry.path());
            }
        }
        dirs.sort();
        for sub in dirs {
            // Skip hidden dirs like .git automatically by first check above; still avoid descending into .git/*
            if sub.file_name().map(|n| n == ".git").unwrap_or(false) {
                continue;
            }
            walk(&sub, depth + 1, max_depth, out)?;
        }
        Ok(())
    }

    walk(root, 0, max_depth, &mut repos)?;
    // Sort lexicographically for stable output
    repos.sort();
    Ok(repos)
}

fn heading_for(path: &Path, root_for_rel: &Path, absolute: bool) -> String {
    if absolute {
        match path.canonicalize() {
            Ok(p) => format!("{}/", p.display()),
            Err(_) => format!("{}/", path.display()),
        }
    } else {
        match diff_paths(path, root_for_rel) {
            Some(rel) => {
                if rel.as_os_str().is_empty() {
                    "./".to_string()
                } else {
                    format!("{}/", rel.display())
                }
            }
            None => format!("{}/", path.display()),
        }
    }
}

fn colorize(s: &str, color: bool) -> String {
    if color {
        format!("\x1b[1;36m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

fn print_heading(_index: usize, _total: usize, text: &str, style: HeadingStyle, color: bool) {
    let label = colorize(text, color);
    match style {
        HeadingStyle::Plain => {
            println!("{}", label);
        }
        HeadingStyle::Rule => {
            println!("{}", label);
        }
    }
}

fn print_fence(style: HeadingStyle, color: bool) {
    if !matches!(style, HeadingStyle::Rule) {
        return;
    }
    let width = terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80)
        .clamp(20, 200);
    let fence = "-".repeat(width);
    println!("{}", colorize(&fence, color));
}

fn run_git_in(repo: &Path, git_args: &[OsString]) -> io::Result<i32> {
    let status = Command::new("git")
        .args(git_args)
        .current_dir(repo)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(status.code().unwrap_or(1))
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let cwd = env::current_dir()?;
    let root = cli.root.clone().unwrap_or_else(|| cwd.clone());
    let root_is_repo = is_git_repo_dir(&root);

    // Determine target repositories
    let mut repos: Vec<PathBuf> = if root_is_repo {
        if cli.parent {
            // include root and its ancestors with git
            let mut v = Vec::new();
            if is_git_repo_dir(&root) {
                v.push(root.clone());
            }
            v.extend(ancestors_with_git(cwd.clone()));
            // Remove duplicates while preserving order
            v.dedup();
            v
        } else {
            vec![root.clone()]
        }
    } else {
        discover_children(&root, cli.max_depth)?
    };

    // Stable order: lexicographic
    repos.sort();
    // If parent was requested and duplicated roots appeared, ensure unique
    repos.dedup();

    // Determine git args
    let git_args: Vec<OsString> = if cli.git_args.is_empty() {
        vec![OsString::from("status")]
    } else {
        cli.git_args
    };

    if cli.list {
        for r in &repos {
            let head = heading_for(r, &root, cli.absolute_path);
            println!("{}", head);
        }
        return Ok(());
    }

    // Decide whether to print headings for single-repo case
    let mut print_headings =
        repos.len() > 1 || cli.parent || cli.absolute_path || cli.root.is_some();
    if cli.no_heading {
        print_headings = false;
    }

    // Determine color usage
    let use_color = match cli.color {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            let no_color = env::var_os("NO_COLOR").is_some();
            #[allow(deprecated)]
            let is_tty = std::io::stdout().is_terminal();
            is_tty && !no_color
        }
    };

    let mut last_code = 0i32;
    for (idx, repo) in repos.iter().enumerate() {
        if print_headings {
            let head = heading_for(repo, &root, cli.absolute_path);
            print_heading(idx, repos.len(), &head, cli.heading_style, use_color);
        }
        let code = run_git_in(repo, &git_args)?;
        last_code = code;
        if print_headings {
            print_fence(cli.heading_style, use_color);
        }
    }

    // Propagate a failing exit code if any
    if last_code != 0 {
        std::process::exit(last_code);
    }
    Ok(())
}
