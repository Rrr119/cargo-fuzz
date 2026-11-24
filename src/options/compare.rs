use crate::coverage_compare::{
    csv, ensure_three_crates, markdown_table, parse_llvm_cov_summary_file, CoverageRow,
};
use crate::RunCommand;
use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, Parser)]
pub struct CompareCoverage {
    /// Coverage report in the form CRATE:STRATEGY:llvm-cov-summary.json
    #[arg(
        short = 'r',
        long = "report",
        value_name = "CRATE:STRATEGY:PATH",
        required = true
    )]
    pub reports: Vec<ReportSpec>,

    /// Output format
    #[arg(long, value_enum, default_value = "markdown")]
    pub format: ReportFormat,

    /// Require reports from at least three distinct crates
    #[arg(long)]
    pub expect_three_crates: bool,
}

#[derive(Clone, Debug)]
pub struct ReportSpec {
    crate_name: String,
    strategy: String,
    path: PathBuf,
}

impl FromStr for ReportSpec {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let mut parts = value.splitn(3, ':');
        let crate_name = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("report must start with CRATE:STRATEGY:PATH"))?;
        let strategy = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("report must include STRATEGY:PATH"))?;
        let path = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("report must include a PATH"))?;
        Ok(Self {
            crate_name: crate_name.to_owned(),
            strategy: strategy.to_owned(),
            path: PathBuf::from(path),
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReportFormat {
    Markdown,
    Csv,
}

impl RunCommand for CompareCoverage {
    fn run_command(&mut self) -> Result<()> {
        let mut rows: Vec<CoverageRow> = Vec::new();
        for report in &self.reports {
            rows.push(parse_llvm_cov_summary_file(
                report.crate_name.clone(),
                report.strategy.clone(),
                &report.path,
            )?);
        }

        if self.expect_three_crates {
            ensure_three_crates(&rows)?;
        }

        match self.format {
            ReportFormat::Markdown => print!("{}", markdown_table(&rows)),
            ReportFormat::Csv => print!("{}", csv(&rows)),
        }
        Ok(())
    }
}
