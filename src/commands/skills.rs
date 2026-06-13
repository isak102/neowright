use crate::cli::{SkillsArgs, SkillsCommand};

pub fn run(args: SkillsArgs) -> Result<String, String> {
    match args.command {
        SkillsCommand::Install(_args) => Ok(
            "`skills install` parsed successfully. Skill installation is not implemented yet."
                .to_string(),
        ),
    }
}
