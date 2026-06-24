use crate::profile::ProfileKind;
use crate::scaffold::{self, Artifact, Ownership};
use crate::{claudegen, config, context, detect, output};
use clap::Args;
use std::path::Path;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Language profile (auto-detected from repo markers when omitted).
    #[arg(long)]
    pub profile: Option<String>,
    /// One-line project description (seeds wording for new projects).
    #[arg(long)]
    pub description: Option<String>,
    /// Show what would be written without touching the filesystem.
    #[arg(long)]
    pub dry_run: bool,
    /// Overwrite an existing .meta/meta.toml.
    #[arg(long)]
    pub force: bool,
    /// Skip the optional `claude` CLI wording enrichment.
    #[arg(long)]
    pub no_ai: bool,
}

pub fn run(args: InitArgs) -> anyhow::Result<i32> {
    let root = context::scaffold_root()?;
    let detection = detect::detect(&root);

    // Resolve the profile: explicit flag wins, else detection, else generic.
    let kind = match &args.profile {
        Some(p) => ProfileKind::parse(p)?,
        None => detection.kind.unwrap_or(ProfileKind::Generic),
    };
    let title = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    output::head(format!("meta init — {} ({})", title, kind.name()));
    if args.profile.is_none() {
        output::info(format!(
            "  detected profile: {} ({})",
            kind.name(),
            if detection.markers.is_empty() {
                "no markers".to_string()
            } else {
                detection.markers.join(", ")
            }
        ));
    }
    for note in &detection.notes {
        output::note(note);
    }

    // Description: explicit flag, else optional `claude` enrichment, else none.
    let description = args.description.clone().or_else(|| {
        if claudegen::available(args.no_ai) {
            output::info("  asking `claude` to tailor the project description…");
            claudegen::describe_project(&root, &title, kind.name(), &detection.domains)
        } else {
            None
        }
    });

    let meta_toml =
        scaffold::render_meta_toml(kind, &title, description.as_deref(), Some(&detection));
    // Resolve config from the generated meta.toml to drive the rest.
    let cfg = config::load_from_str(&root, &meta_toml)?;
    let artifacts = scaffold::generated_artifacts(&cfg);

    let meta_path = config::config_path(&root);
    let meta_exists = meta_path.exists();

    if args.dry_run {
        output::head("\nWould write");
        let label = if meta_exists && !args.force {
            output::dim("skip (exists)")
        } else {
            output::green("new")
        };
        println!("  {label}  .meta/meta.toml");
        for a in &artifacts {
            println!("  {}  {}", plan_label(&root, a), a.path);
        }
        output::head("\n.meta/meta.toml");
        println!("{meta_toml}");
        output::note("dry run — nothing written.");
        return Ok(0);
    }

    // Write meta.toml (user-owned; never clobber without --force).
    if meta_exists && !args.force {
        output::note(".meta/meta.toml exists — keeping it (use --force to overwrite)");
    } else {
        write_file(&meta_path, &meta_toml, false)?;
        output::ok("wrote .meta/meta.toml");
    }

    // Write the generated/managed artifacts.
    for a in &artifacts {
        let path = root.join(&a.path);
        let existing = std::fs::read_to_string(&path).ok();
        let content = scaffold::resolve_content(a, existing.as_deref());
        write_file(&path, &content, a.executable)?;
    }
    output::ok(format!("wrote {} managed files", artifacts.len()));

    println!();
    output::head("Next steps");
    output::info("  • Review .meta/meta.toml and CLAUDE.md, then commit.");
    output::info("  • `./meta check` to run standards · `./meta setup` to bootstrap GitHub.");
    Ok(0)
}

fn plan_label(root: &Path, a: &Artifact) -> String {
    let path = root.join(&a.path);
    if !path.exists() {
        return output::green("new");
    }
    match a.ownership {
        Ownership::Generated => output::yellow("overwrite"),
        Ownership::Managed => output::yellow("update block"),
        Ownership::AppendMerge => output::yellow("merge"),
    }
}

fn write_file(path: &Path, content: &str, executable: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    if executable {
        set_executable(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
