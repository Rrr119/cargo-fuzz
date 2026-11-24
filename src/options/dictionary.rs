use crate::{
    dictionary::{
        default_dictionary_path, dictionary_dir, merge_entries, parse_source, serialize_dictionary,
        DictionaryFormat,
    },
    options::FuzzDirWrapper,
    project::FuzzProject,
    RunCommand,
};
use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CliDictionaryFormat {
    Auto,
    Dict,
    WordList,
}

impl From<CliDictionaryFormat> for DictionaryFormat {
    fn from(value: CliDictionaryFormat) -> Self {
        match value {
            CliDictionaryFormat::Auto => DictionaryFormat::Auto,
            CliDictionaryFormat::Dict => DictionaryFormat::LibFuzzer,
            CliDictionaryFormat::WordList => DictionaryFormat::WordList,
        }
    }
}

/// Import and inspect libFuzzer dictionaries.
#[derive(Clone, Debug, Parser)]
pub struct Dictionary {
    #[command(flatten)]
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    #[command(subcommand)]
    pub command: DictionaryCommand,
}

#[derive(Clone, Debug, Parser)]
pub enum DictionaryCommand {
    /// Import a dictionary file, word list, or directory
    Import(ImportDict),

    /// List dictionaries in `fuzz/dictionaries`
    List(ListDictionaries),
}

#[derive(Clone, Debug, Parser)]
pub struct ImportDict {
    /// Dictionary file, word list, or directory to import
    pub source: PathBuf,

    /// Target whose default dictionary should receive the import
    #[arg(value_name = "TARGET")]
    pub target: Option<String>,

    #[command(flatten)]
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// Write the normalized dictionary to this path instead of the target default
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Input format
    #[arg(long, value_enum, default_value = "auto")]
    pub format: CliDictionaryFormat,

    /// Replace an existing output dictionary instead of merging
    #[arg(long)]
    pub force: bool,

    /// Print the normalized dictionary to stdout
    #[arg(long)]
    pub print: bool,
}

#[derive(Clone, Debug, Parser)]
pub struct ListDictionaries {
    /// Optional target whose dictionary should be shown
    pub target: Option<String>,

    #[command(flatten)]
    pub fuzz_dir_wrapper: FuzzDirWrapper,
}

impl RunCommand for Dictionary {
    fn run_command(&mut self) -> Result<()> {
        match &mut self.command {
            DictionaryCommand::Import(import) => {
                if import.fuzz_dir_wrapper.fuzz_dir.is_none() {
                    import.fuzz_dir_wrapper.fuzz_dir = self.fuzz_dir_wrapper.fuzz_dir.clone();
                }
                import.run_command()
            }
            DictionaryCommand::List(list) => {
                if list.fuzz_dir_wrapper.fuzz_dir.is_none() {
                    list.fuzz_dir_wrapper.fuzz_dir = self.fuzz_dir_wrapper.fuzz_dir.clone();
                }
                list.run_command()
            }
        }
    }
}

impl RunCommand for ImportDict {
    fn run_command(&mut self) -> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.clone())?;
        let target = self.target.clone().unwrap_or_else(|| "imported".to_owned());
        let output = self
            .output
            .clone()
            .unwrap_or_else(|| default_dictionary_path(project.fuzz_dir(), &target));

        let incoming = parse_source(&self.source, self.format.into())?;
        if incoming.is_empty() {
            bail!("no dictionary entries found in {}", self.source.display());
        }

        let entries = if output.exists() && !self.force {
            let existing_text = std::fs::read_to_string(&output).with_context(|| {
                format!("failed to read existing dictionary {}", output.display())
            })?;
            let existing = crate::dictionary::parse_dictionary(&existing_text)?;
            merge_entries(existing, incoming)
        } else {
            incoming
        };

        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let serialized = serialize_dictionary(&entries);
        std::fs::write(&output, &serialized)
            .with_context(|| format!("failed to write dictionary {}", output.display()))?;

        eprintln!(
            "Imported {} dictionary entries into {}",
            entries.len(),
            output.display()
        );
        if self.print {
            print!("{serialized}");
        }
        Ok(())
    }
}

impl RunCommand for ListDictionaries {
    fn run_command(&mut self) -> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.clone())?;
        let dir = dictionary_dir(project.fuzz_dir());

        if let Some(target) = &self.target {
            let path = default_dictionary_path(project.fuzz_dir(), target);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            print!("{text}");
            return Ok(());
        }

        if !dir.exists() {
            return Ok(());
        }

        let mut paths = Vec::new();
        for entry in
            std::fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            if entry.path().extension().and_then(|ext| ext.to_str()) == Some("dict") {
                paths.push(entry.path());
            }
        }
        paths.sort();
        for path in paths {
            if let Some(name) = path.file_name() {
                println!("{}", name.to_string_lossy());
            }
        }
        Ok(())
    }
}
