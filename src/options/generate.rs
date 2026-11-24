use crate::{
    dictionary::{default_dictionary_path, serialize_dictionary},
    options::FuzzDirWrapper,
    project::FuzzProject,
    structured::StructuredSchema,
    RunCommand,
};
use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;

/// Generate structured corpus seeds and `Arbitrary` fuzz targets.
#[derive(Clone, Debug, Parser)]
pub struct Generate {
    #[command(flatten)]
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// Target name when generating seeds without an explicit `seeds` subcommand
    pub target: Option<String>,

    /// TOML schema for seed generation; omitted creates an `Arbitrary` target
    #[arg(long, value_name = "PATH")]
    pub schema: Option<PathBuf>,

    /// Generate an `Arbitrary` target; without `--schema` a default shape is used
    #[arg(long, aliases = ["arbitrary"])]
    pub structured: bool,

    /// Number of deterministic seeds to generate
    #[arg(short = 'n', long, default_value_t = 16)]
    pub count: usize,

    /// Output corpus directory for flat seed generation
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,

    /// Overwrite generated files that already exist
    #[arg(long)]
    pub force: bool,

    /// Write literal schema tokens to the target dictionary as well
    #[arg(long)]
    pub write_dictionary: bool,

    #[command(subcommand)]
    pub command: Option<GenerateCommand>,
}

#[derive(Clone, Debug, Parser)]
pub enum GenerateCommand {
    /// Generate deterministic structured corpus files from a TOML schema
    #[command(visible_alias = "structured", aliases = ["input", "gen", "generate"])]
    Seeds(GenerateSeeds),

    /// Generate an `Arbitrary`-based Rust fuzz target from a TOML schema
    #[command(visible_alias = "arbitrary")]
    Target(GenerateTarget),
}

#[derive(Clone, Debug, Parser)]
pub struct GenerateSeeds {
    /// Name of the fuzz target and default corpus directory
    pub target: String,

    /// TOML schema describing fields and an optional output template
    #[arg(long, value_name = "PATH")]
    pub schema: PathBuf,

    #[command(flatten)]
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// Number of deterministic seeds to generate
    #[arg(short = 'n', long, default_value_t = 16)]
    pub count: usize,

    /// Output corpus directory
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,

    /// Overwrite generated `structured-*` files that already exist
    #[arg(long)]
    pub force: bool,

    /// Also write literal schema tokens to the target's default dictionary
    #[arg(long)]
    pub write_dictionary: bool,
}

#[derive(Clone, Debug, Parser)]
pub struct GenerateTarget {
    /// Name of the fuzz target to create
    pub target: String,

    #[command(flatten)]
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// TOML schema; when omitted a small representative schema is used
    #[arg(long, value_name = "PATH")]
    pub schema: Option<PathBuf>,

    /// Overwrite an existing target file
    #[arg(long)]
    pub force: bool,
}

impl RunCommand for Generate {
    fn run_command(&mut self) -> Result<()> {
        let Some(command) = &mut self.command else {
            let Some(target) = self.target.clone() else {
                bail!("usage: cargo fuzz generate <TARGET> [--schema <SCHEMA>]");
            };
            if self.structured || self.schema.is_none() {
                let mut target_command = GenerateTarget {
                    target,
                    fuzz_dir_wrapper: self.fuzz_dir_wrapper.clone(),
                    schema: self.schema.clone(),
                    force: self.force,
                };
                return target_command.run_command();
            };
            let schema = self.schema.clone().expect("schema checked above");
            let mut seeds = GenerateSeeds {
                target,
                schema,
                fuzz_dir_wrapper: self.fuzz_dir_wrapper.clone(),
                count: self.count,
                out: self.out.clone(),
                force: self.force,
                write_dictionary: self.write_dictionary,
            };
            return seeds.run_command();
        };

        match command {
            GenerateCommand::Seeds(seeds) => {
                if seeds.fuzz_dir_wrapper.fuzz_dir.is_none() {
                    seeds.fuzz_dir_wrapper.fuzz_dir = self.fuzz_dir_wrapper.fuzz_dir.clone();
                }
                seeds.run_command()
            }
            GenerateCommand::Target(target) => {
                if target.fuzz_dir_wrapper.fuzz_dir.is_none() {
                    target.fuzz_dir_wrapper.fuzz_dir = self.fuzz_dir_wrapper.fuzz_dir.clone();
                }
                target.run_command()
            }
        }
    }
}

impl RunCommand for GenerateSeeds {
    fn run_command(&mut self) -> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.clone())?;
        let schema = StructuredSchema::from_path(&self.schema)?;
        let output_dir = self
            .out
            .clone()
            .unwrap_or_else(|| project.fuzz_dir().join("corpus").join(&self.target));

        let paths = schema.generate_seed_files(&output_dir, self.count, self.force)?;
        eprintln!(
            "Generated {} structured seeds in {}",
            paths.len(),
            output_dir.display()
        );

        if self.write_dictionary {
            let path = default_dictionary_path(project.fuzz_dir(), &self.target);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let entries = schema.dictionary_entries();
            std::fs::write(&path, serialize_dictionary(&entries))
                .with_context(|| format!("failed to write {}", path.display()))?;
            eprintln!(
                "Wrote {} schema tokens to dictionary {}",
                entries.len(),
                path.display()
            );
        }

        Ok(())
    }
}

impl RunCommand for GenerateTarget {
    fn run_command(&mut self) -> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.clone())?;
        let schema = match &self.schema {
            Some(path) => StructuredSchema::from_path(path)?,
            None => crate::structured::default_schema(),
        };
        let code = schema.arbitrary_target();
        let path = project.write_generated_target(&self.target, &code, self.force)?;
        eprintln!("Generated structured target {}", path.display());
        Ok(())
    }
}
