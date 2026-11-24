use anyhow::{bail, Context, Result};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageRow {
    pub crate_name: String,
    pub strategy: String,
    pub lines_total: u64,
    pub lines_covered: u64,
    pub regions_total: u64,
    pub regions_covered: u64,
}

impl CoverageRow {
    pub fn line_percent(&self) -> f64 {
        percent(self.lines_covered, self.lines_total)
    }

    pub fn region_percent(&self) -> f64 {
        percent(self.regions_covered, self.regions_total)
    }
}

/// Parse the `totals` object emitted by `llvm-cov export --summary-only`.
///
/// Keeping this parser small avoids adding a serde dependency to `cargo-fuzz`;
/// the benchmark script passes the summary JSON through this function's CLI
/// wrapper in tests.
pub fn parse_llvm_cov_summary(
    crate_name: impl Into<String>,
    strategy: impl Into<String>,
    json: &str,
) -> Result<CoverageRow> {
    let value: serde_json::Value = serde_json::from_str(json).context("invalid llvm-cov JSON")?;
    let totals = value
        .pointer("/data/0/totals")
        .context("llvm-cov JSON does not contain data[0].totals")?;

    let metric = |name: &str| -> Result<(u64, u64)> {
        let metric = totals
            .get(name)
            .with_context(|| format!("coverage summary lacks `{name}`"))?;
        let count = metric
            .get("count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        let covered = metric
            .get("covered")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        Ok((count, covered))
    };

    let (lines_total, lines_covered) = metric("lines")?;
    let (regions_total, regions_covered) = metric("regions")?;
    Ok(CoverageRow {
        crate_name: crate_name.into(),
        strategy: strategy.into(),
        lines_total,
        lines_covered,
        regions_total,
        regions_covered,
    })
}

pub fn parse_llvm_cov_summary_file(
    crate_name: impl Into<String>,
    strategy: impl Into<String>,
    path: &Path,
) -> Result<CoverageRow> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read coverage summary {}", path.display()))?;
    parse_llvm_cov_summary(crate_name, strategy, &json)
}

pub fn markdown_table(rows: &[CoverageRow]) -> String {
    let mut output = String::new();
    output.push_str("| Crate | Strategy | Lines covered | Line coverage | Regions covered | Region coverage |\n");
    output.push_str("|---|---:|---:|---:|---:|---:|\n");
    for row in rows {
        output.push_str(&format!(
            "| {} | {} | {}/{} | {:.1}% | {}/{} | {:.1}% |\n",
            row.crate_name,
            row.strategy,
            row.lines_covered,
            row.lines_total,
            row.line_percent(),
            row.regions_covered,
            row.regions_total,
            row.region_percent(),
        ));
    }
    output
}

pub fn csv(rows: &[CoverageRow]) -> String {
    let mut output =
        String::from("crate,strategy,lines_covered,lines_total,line_percent,regions_covered,regions_total,region_percent\n");
    for row in rows {
        output.push_str(&format!(
            "{},{},{},{},{:.2},{},{},{:.2}\n",
            row.crate_name,
            row.strategy,
            row.lines_covered,
            row.lines_total,
            row.line_percent(),
            row.regions_covered,
            row.regions_total,
            row.region_percent(),
        ));
    }
    output
}

pub fn ensure_three_crates(rows: &[CoverageRow]) -> Result<()> {
    let mut crates: Vec<&str> = rows.iter().map(|row| row.crate_name.as_str()).collect();
    crates.sort_unstable();
    crates.dedup();
    if crates.len() < 3 {
        bail!(
            "coverage comparison requires at least 3 crates, found {}",
            crates.len()
        );
    }
    Ok(())
}

fn percent(covered: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        covered as f64 * 100.0 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMMARY: &str = r#"{
      "data": [{
        "totals": {
          "lines": {"count": 200, "covered": 120},
          "regions": {"count": 80, "covered": 60}
        }
      }]
    }"#;

    #[test]
    fn parses_summary_and_formats_report() {
        let row = parse_llvm_cov_summary("route_lite", "dict", SUMMARY).unwrap();
        assert_eq!(row.lines_total, 200);
        assert_eq!(row.lines_covered, 120);
        assert_eq!(row.line_percent(), 60.0);
        assert_eq!(row.region_percent(), 75.0);

        let table = markdown_table(std::slice::from_ref(&row));
        assert!(table.contains("route_lite"));
        assert!(table.contains("| dict |"));
        assert!(table.contains("60.0%"));
        assert!(csv(&[row]).contains("route_lite,dict,120,200,60.00"));
    }

    #[test]
    fn requires_three_distinct_crates() {
        let row = parse_llvm_cov_summary("a", "baseline", SUMMARY).unwrap();
        assert!(ensure_three_crates(&[row.clone(), row.clone()]).is_err());
        let mut rows = vec![row.clone(), row.clone()];
        rows[1].crate_name = "b".into();
        rows.push(CoverageRow {
            crate_name: "c".into(),
            strategy: "structured".into(),
            ..row
        });
        ensure_three_crates(&rows).unwrap();
    }
}
