use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredSchema {
    pub name: String,
    pub template: Option<String>,
    pub fields: Vec<StructuredField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredField {
    pub name: String,
    pub kind: FieldKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Bool,
    Integer { min: i64, max: i64 },
    Float,
    String,
    Choice(Vec<String>),
    Constant(String),
}

impl StructuredSchema {
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read schema {}", path.display()))?;
        Self::from_toml(&text).with_context(|| format!("invalid schema {}", path.display()))
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        let value: toml::Value = toml::from_str(text).context("schema is not valid TOML")?;
        let table = value
            .as_table()
            .context("schema must be a TOML table at the top level")?;

        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("StructuredInput")
            .to_owned();
        let template = table
            .get("template")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);

        let raw_fields = table
            .get("fields")
            .and_then(toml::Value::as_array)
            .context("schema must contain a [[fields]] array")?;

        let mut fields = Vec::new();
        for raw_field in raw_fields {
            let field_table = raw_field
                .as_table()
                .context("every [[fields]] entry must be a table")?;
            let name = field_table
                .get("name")
                .and_then(toml::Value::as_str)
                .context("every field needs a string `name`")?
                .to_owned();

            let string_array = |key: &str| -> Option<Vec<String>> {
                field_table
                    .get(key)
                    .and_then(toml::Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(toml::Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
            };

            let kind = if let Some(constant) =
                field_table.get("constant").and_then(toml::Value::as_str)
            {
                FieldKind::Constant(constant.to_owned())
            } else if let Some(choices) = string_array("choices").or_else(|| string_array("values"))
            {
                if choices.is_empty() {
                    bail!("field `{name}` has an empty choices list");
                }
                FieldKind::Choice(choices)
            } else if let Some(tokens) = string_array("tokens") {
                if tokens.is_empty() {
                    bail!("field `{name}` has an empty tokens list");
                }
                FieldKind::Choice(tokens)
            } else {
                match field_table.get("type").and_then(toml::Value::as_str) {
                    Some("bool" | "boolean") => FieldKind::Bool,
                    Some("integer" | "int" | "i64" | "u64" | "number") => {
                        let min = field_table
                            .get("min")
                            .and_then(toml::Value::as_integer)
                            .unwrap_or(0);
                        let max = field_table
                            .get("max")
                            .and_then(toml::Value::as_integer)
                            .unwrap_or(99);
                        if min > max {
                            bail!("field `{name}` has min greater than max");
                        }
                        FieldKind::Integer { min, max }
                    }
                    Some("float" | "f64") => FieldKind::Float,
                    Some("string" | "text") | None => FieldKind::String,
                    Some(other) => bail!("unsupported field type `{other}` for `{name}`"),
                }
            };

            fields.push(StructuredField { name, kind });
        }

        if fields.is_empty() {
            bail!("schema must contain at least one field");
        }

        Ok(Self {
            name,
            template,
            fields,
        })
    }

    /// Render one deterministic structured seed.
    pub fn render_seed(&self, seed: usize) -> Vec<u8> {
        let values: Vec<(String, String)> = self
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| (field.name.clone(), render_field(&field.kind, seed, index)))
            .collect();

        let output = match &self.template {
            Some(template) => render_template(template, &values),
            None => render_json(&self.fields, seed),
        };
        output.into_bytes()
    }

    pub fn generate_seed_files(
        &self,
        output_dir: &Path,
        count: usize,
        overwrite: bool,
    ) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(output_dir)
            .with_context(|| format!("failed to create seed directory {}", output_dir.display()))?;

        let mut paths = Vec::with_capacity(count);
        for seed in 0..count {
            let path = output_dir.join(format!("structured-{seed:04}"));
            if path.exists() && !overwrite {
                bail!(
                    "seed file {} already exists; pass --force to overwrite",
                    path.display()
                );
            }
            fs::write(&path, self.render_seed(seed))
                .with_context(|| format!("failed to write seed {}", path.display()))?;
            paths.push(path);
        }
        Ok(paths)
    }

    /// Collect literal tokens useful as a libFuzzer dictionary.
    pub fn dictionary_entries(&self) -> Vec<crate::dictionary::DictionaryEntry> {
        let mut entries = Vec::new();
        for field in &self.fields {
            match &field.kind {
                FieldKind::Choice(choices) => {
                    for value in choices {
                        entries.push(crate::dictionary::DictionaryEntry {
                            name: Some(format!("{}_{}", field.name, entries.len())),
                            value: value.as_bytes().to_vec(),
                        });
                    }
                }
                FieldKind::Constant(value) => {
                    entries.push(crate::dictionary::DictionaryEntry {
                        name: Some(field.name.clone()),
                        value: value.as_bytes().to_vec(),
                    });
                }
                _ => {}
            }
        }
        crate::dictionary::merge_entries(Vec::new(), entries)
    }

    /// Generate a Rust target using `libfuzzer_sys::arbitrary`.
    ///
    /// The implementation is hand-written rather than using `#[derive]` so
    /// that projects do not need to enable Arbitrary's derive feature.
    pub fn arbitrary_target(&self) -> String {
        let mut output = String::from(
            "#![no_main]\n\
             use libfuzzer_sys::arbitrary::{Arbitrary, Result as ArbitraryResult, Unstructured};\n\
             use libfuzzer_sys::fuzz_target;\n\n\
             #[derive(Debug)]\n\
             struct ",
        );
        output.push_str(&rust_ident(&self.name));
        output.push_str(" {\n");
        for field in &self.fields {
            output.push_str("    pub ");
            output.push_str(&rust_field_name(&field.name));
            output.push_str(": ");
            output.push_str(rust_type(&field.kind));
            output.push_str(",\n");
        }
        output.push_str(
            "}\n\n\
             impl<'a> Arbitrary<'a> for ",
        );
        output.push_str(&rust_ident(&self.name));
        output.push_str(
            " {\n\
                 fn arbitrary(u: &mut Unstructured<'a>) -> ArbitraryResult<Self> {\n\
                     Ok(Self {\n",
        );
        for field in &self.fields {
            output.push_str("                        ");
            output.push_str(&rust_field_name(&field.name));
            output.push_str(": u.arbitrary()?,\n");
        }
        output.push_str(
            "                    })\n\
                 }\n\
             }\n\n\
             fuzz_target!(|input: ",
        );
        output.push_str(&rust_ident(&self.name));
        output.push_str(
            "| {\n\
                 let _ = input;\n\
             });\n",
        );
        output
    }
}

pub fn default_schema() -> StructuredSchema {
    StructuredSchema::from_toml(
        r#"
        name = "StructuredInput"
        [[fields]]
        name = "id"
        type = "integer"
        min = 0
        max = 1000
        [[fields]]
        name = "name"
        type = "string"
        [[fields]]
        name = "active"
        type = "bool"
        "#,
    )
    .expect("default schema must be valid")
}

fn render_field(kind: &FieldKind, seed: usize, field_index: usize) -> String {
    match kind {
        FieldKind::Bool => ((seed + field_index) % 2 == 0).to_string(),
        FieldKind::Integer { min, max } => {
            let span = (*max - *min + 1).max(1) as usize;
            let value = min + ((seed * 31 + field_index * 17) % span) as i64;
            value.to_string()
        }
        FieldKind::Float => format!("{}.{}", seed % 10, (seed * 7 + field_index) % 100),
        FieldKind::String => format!("value-{seed}-{field_index}"),
        FieldKind::Choice(choices) => choices[seed % choices.len()].clone(),
        FieldKind::Constant(value) => value.clone(),
    }
}

fn render_template(template: &str, values: &[(String, String)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('}') else {
            output.push_str(&rest[start..]);
            return output;
        };

        let name = after_start[..end].trim();
        let replacement = values
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or("");
        output.push_str(replacement);
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    output
}

fn render_json(fields: &[StructuredField], seed: usize) -> String {
    let mut output = String::from("{\n");
    for (index, field) in fields.iter().enumerate() {
        let value = render_field(&field.kind, seed, index);
        output.push_str("  \"");
        output.push_str(&json_escape(&field.name));
        output.push_str("\": ");
        if matches!(
            field.kind,
            FieldKind::Bool | FieldKind::Integer { .. } | FieldKind::Float
        ) {
            output.push_str(&value);
        } else {
            output.push('"');
            output.push_str(&json_escape(&value));
            output.push('"');
        }
        if index + 1 != fields.len() {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str("}\n");
    output
}

fn json_escape(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if (ch as u32) < 0x20 => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output
}

fn rust_type(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Bool => "bool",
        FieldKind::Integer { .. } => "i64",
        FieldKind::Float => "f64",
        FieldKind::String | FieldKind::Choice(_) | FieldKind::Constant(_) => "String",
    }
}

fn rust_ident(name: &str) -> String {
    let mut output = String::new();
    let mut capitalize_next = true;
    for ch in name.chars() {
        if !ch.is_ascii_alphanumeric() {
            capitalize_next = true;
            continue;
        }
        if capitalize_next {
            output.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            output.push(ch);
        }
    }
    if output.is_empty() {
        output.push_str("StructuredInput");
    }
    output
}

fn rust_field_name(name: &str) -> String {
    let mut output = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        output.insert(0, '_');
    }
    if output.is_empty() {
        output.push_str("field");
    }
    if RUST_KEYWORDS.contains(&output.as_str()) {
        output.insert_str(0, "r#");
    }
    output
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> StructuredSchema {
        StructuredSchema::from_toml(
            r#"
            name = "http-request"
            template = "{method} {path} HTTP/1.1\n"
            [[fields]]
            name = "method"
            choices = ["GET", "POST"]
            [[fields]]
            name = "path"
            tokens = ["/health", "/users"]
            [[fields]]
            name = "retries"
            type = "integer"
            min = 0
            max = 3
            "#,
        )
        .unwrap()
    }

    #[test]
    fn parses_and_renders_deterministic_template() {
        let schema = schema();
        assert_eq!(
            String::from_utf8(schema.render_seed(0)).unwrap(),
            "GET /health HTTP/1.1\n"
        );
        assert_eq!(
            String::from_utf8(schema.render_seed(1)).unwrap(),
            "POST /users HTTP/1.1\n"
        );
    }

    #[test]
    fn renders_json_without_template() {
        let schema = StructuredSchema::from_toml(
            r#"
            [[fields]]
            name = "active"
            type = "bool"
            "#,
        )
        .unwrap();
        let seed = String::from_utf8(schema.render_seed(0)).unwrap();
        assert!(seed.contains("\"active\": true"));
    }

    #[test]
    fn generates_arbitrary_target() {
        let code = schema().arbitrary_target();
        assert!(code.contains("struct HttpRequest"));
        assert!(code.contains("pub method: String"));
        assert!(code.contains("pub retries: i64"));
        assert!(code.contains("fuzz_target!(|input: HttpRequest|"));
    }
}
