use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::cli::{SkillsArgs, SkillsCommand, SkillsInstallArgs};

const SKILL_NAME: &str = "neowright";
const BUNDLED_SKILL_FILES: &[(&str, &str)] =
    &[("SKILL.md", include_str!("../../skills/neowright/SKILL.md"))];

pub fn run(args: SkillsArgs) -> Result<String, String> {
    match args.command {
        SkillsCommand::Install(args) => install(args),
    }
}

fn install(args: SkillsInstallArgs) -> Result<String, String> {
    let (scope, target) = install_target(args.local)?;
    let overwrote = target.exists();

    if overwrote {
        fs::remove_dir_all(&target).map_err(|error| {
            format!(
                "failed to remove existing Neowright Agent Skill `{}`: {error}",
                target.display()
            )
        })?;
    }

    fs::create_dir_all(&target).map_err(|error| {
        format!(
            "failed to create Neowright Agent Skill directory `{}`: {error}",
            target.display()
        )
    })?;

    for (relative_path, contents) in BUNDLED_SKILL_FILES {
        validate_relative_skill_path(relative_path)?;
        let path = target.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Neowright Agent Skill directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }
        fs::write(&path, contents).map_err(|error| {
            format!(
                "failed to write Neowright Agent Skill file `{}`: {error}",
                path.display()
            )
        })?;
    }

    let overwrite_message = if overwrote {
        "\n- Overwrote existing skill files"
    } else {
        ""
    };

    Ok(format!(
        "Installed Neowright Agent Skill.\n\n- Scope: `{scope}`\n- Path: `{}`",
        target.display(),
    ) + overwrite_message)
}

fn install_target(local: bool) -> Result<(&'static str, PathBuf), String> {
    let base = if local {
        let cwd = env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?;
        ("local", cwd)
    } else {
        let home = env::var_os("HOME").ok_or_else(|| {
            "HOME must be set to install the Neowright Agent Skill globally".to_string()
        })?;
        ("global", PathBuf::from(home))
    };

    Ok((
        base.0,
        base.1.join(".agents").join("skills").join(SKILL_NAME),
    ))
}

fn validate_relative_skill_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "bundled Neowright Agent Skill contains unsafe path `{}`",
            path.display()
        ));
    }

    Ok(())
}
