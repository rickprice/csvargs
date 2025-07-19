use anyhow::{Context, Result};
use clap::Parser;
use csv::ReaderBuilder;
use minijinja::{context, Environment};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

#[derive(Parser)]
#[command(name = "csvargs")]
#[command(about = "Process CSV files with Jinja2 templates and execute commands")]
pub struct Args {
    #[arg(value_name = "TEMPLATE", help = "Jinja2 template string")]
    pub template: String,

    #[arg(long = "no-header", help = "CSV files do NOT have header row")]
    pub no_header: bool,

    #[arg(value_name = "FILES", help = "CSV files to process")]
    pub files: Vec<String>,
}

#[derive(Debug)]
pub struct CsvProcessor {
    env: Environment<'static>,
    template_str: String,
    has_header: bool,
}

impl CsvProcessor {
    pub fn new(template_str: &str, has_header: bool) -> Result<Self> {
        let env = Environment::new();
        env.template_from_str(template_str)
            .with_context(|| "Failed to parse template")?;
        
        Ok(Self {
            env,
            template_str: template_str.to_string(),
            has_header,
        })
    }

    pub fn process_file<P: AsRef<Path>>(&self, file_path: P) -> Result<()> {
        let file_path = file_path.as_ref();
        let file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        self.process_reader(file)
    }

    pub fn process_reader<R: Read>(&self, reader: R) -> Result<()> {
        let mut csv_reader = ReaderBuilder::new()
            .has_headers(self.has_header)
            .from_reader(reader);

        let headers = if self.has_header {
            Some(csv_reader.headers()?.clone())
        } else {
            None
        };

        for (row_index, result) in csv_reader.records().enumerate() {
            let record = result
                .with_context(|| format!("Failed to read row {}", row_index))?;
            
            let row_data = match &headers {
                Some(h) => create_named_context(h, &record),
                None => create_indexed_context(&record),
            };

            let template = self.env.template_from_str(&self.template_str)
                .with_context(|| "Failed to parse template")?;
            let rendered = template.render(context! { row => row_data })
                .with_context(|| format!("Failed to render template for row {}", row_index))?;

            execute_command(&rendered, row_index)
                .with_context(|| format!("Failed to execute command for row {}", row_index))?;
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.files.is_empty() {
        anyhow::bail!("At least one CSV file must be provided");
    }

    let processor = CsvProcessor::new(&args.template, !args.no_header)?;
    
    for file_path in &args.files {
        processor.process_file(file_path)
            .with_context(|| format!("Failed to process file: {}", file_path))?;
    }

    Ok(())
}

fn create_named_context(headers: &csv::StringRecord, record: &csv::StringRecord) -> HashMap<String, Value> {
    headers.iter()
        .enumerate()
        .map(|(i, header)| {
            let value = record.get(i).unwrap_or("").to_string();
            (header.to_string(), Value::String(value))
        })
        .collect()
}

fn create_indexed_context(record: &csv::StringRecord) -> HashMap<String, Value> {
    record.iter()
        .enumerate()
        .map(|(i, field)| (i.to_string(), Value::String(field.to_string())))
        .collect()
}

fn execute_command(command: &str, row_index: usize) -> Result<()> {
    println!("Executing for row {}: {}", row_index, command);
    
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .output()
    } else {
        Command::new("sh")
            .args(["-c", command])
            .output()
    }
    .with_context(|| "Failed to execute command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command failed with status {}: {}", output.status, stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        println!("{}", stdout);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_create_named_context() {
        let headers = csv::StringRecord::from(vec!["name", "age", "city"]);
        let record = csv::StringRecord::from(vec!["Alice", "25", "NYC"]);
        
        let context = create_named_context(&headers, &record);
        
        assert_eq!(context.get("name"), Some(&Value::String("Alice".to_string())));
        assert_eq!(context.get("age"), Some(&Value::String("25".to_string())));
        assert_eq!(context.get("city"), Some(&Value::String("NYC".to_string())));
    }

    #[test]
    fn test_create_indexed_context() {
        let record = csv::StringRecord::from(vec!["Alice", "25", "NYC"]);
        
        let context = create_indexed_context(&record);
        
        assert_eq!(context.get("0"), Some(&Value::String("Alice".to_string())));
        assert_eq!(context.get("1"), Some(&Value::String("25".to_string())));
        assert_eq!(context.get("2"), Some(&Value::String("NYC".to_string())));
    }

    #[test]
    fn test_csv_processor_new_valid_template() {
        let processor = CsvProcessor::new("echo {{row.name}}", true);
        assert!(processor.is_ok());
    }

    #[test]
    fn test_csv_processor_new_invalid_template() {
        let processor = CsvProcessor::new("echo {{row.name", true);
        assert!(processor.is_err());
    }

    #[test]
    fn test_process_csv_with_headers() {
        let csv_data = "name,age\nAlice,25\nBob,30";
        let cursor = Cursor::new(csv_data);
        
        let processor = CsvProcessor::new("echo Hello {{row.name}}", true).unwrap();
        let result = processor.process_reader(cursor);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_csv_without_headers() {
        let csv_data = "Alice,25\nBob,30";
        let cursor = Cursor::new(csv_data);
        
        let processor = CsvProcessor::new("echo Hello {{row['0']}}", false).unwrap();
        let result = processor.process_reader(cursor);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_empty_csv() {
        let csv_data = "";
        let cursor = Cursor::new(csv_data);
        
        let processor = CsvProcessor::new("echo {{row['0']}}", false).unwrap();
        let result = processor.process_reader(cursor);
        
        assert!(result.is_ok());
    }

    #[test] 
    fn test_process_csv_with_missing_fields() {
        let csv_data = "name,age\nAlice,25\nBob,";
        let cursor = Cursor::new(csv_data);
        
        let processor = CsvProcessor::new("echo Hello {{row.name}} age {{row.age}}", true).unwrap();
        let result = processor.process_reader(cursor);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "name,age")?;
        writeln!(temp_file, "Alice,25")?;
        writeln!(temp_file, "Bob,30")?;
        
        let processor = CsvProcessor::new("echo Hello {{row.name}}", true)?;
        let result = processor.process_file(temp_file.path());
        
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_process_nonexistent_file() {
        let processor = CsvProcessor::new("echo {{row['0']}}", false).unwrap();
        let result = processor.process_file("/nonexistent/file.csv");
        
        assert!(result.is_err());
    }

    #[test]
    fn test_template_rendering_with_special_characters() {
        let csv_data = "message\nHello World\nquoted text";
        let cursor = Cursor::new(csv_data);
        
        let processor = CsvProcessor::new("echo '{{row.message}}'", true).unwrap();
        let result = processor.process_reader(cursor);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_columns_template() {
        let csv_data = "first,last,age\nJohn,Doe,30\nJane,Smith,25";
        let cursor = Cursor::new(csv_data);
        
        let processor = CsvProcessor::new("echo {{row.first}} {{row.last}} is {{row.age}} years old", true).unwrap();
        let result = processor.process_reader(cursor);
        
        assert!(result.is_ok());
    }

    mod integration_tests {
        use super::*;
        use assert_cmd::Command;
        use predicates::prelude::*;
        use std::io::Write;

        #[test]
        fn test_cli_with_headers() -> Result<()> {
            let mut temp_file = NamedTempFile::new()?;
            writeln!(temp_file, "name,age")?;
            writeln!(temp_file, "Alice,25")?;
            
            let mut cmd = Command::cargo_bin("csvargs")?;
            cmd.arg("echo test-{{row.name}}")
                .arg(temp_file.path());
            
            cmd.assert()
                .success()
                .stdout(predicate::str::contains("test-Alice"));
            
            Ok(())
        }

        #[test]
        fn test_cli_without_headers() -> Result<()> {
            let mut temp_file = NamedTempFile::new()?;
            writeln!(temp_file, "Alice,25")?;
            
            let mut cmd = Command::cargo_bin("csvargs")?;
            cmd.arg("--no-header")
                .arg("echo test-{{row['0']}}")
                .arg(temp_file.path());
            
            cmd.assert()
                .success()
                .stdout(predicate::str::contains("test-Alice"));
            
            Ok(())
        }

        #[test]
        fn test_cli_no_files() -> Result<()> {
            let mut cmd = Command::cargo_bin("csvargs")?;
            cmd.arg("echo {{row['0']}}");
            
            cmd.assert()
                .failure()
                .stderr(predicate::str::contains("At least one CSV file must be provided"));
            
            Ok(())
        }

        #[test]
        fn test_cli_invalid_template() -> Result<()> {
            let mut temp_file = NamedTempFile::new()?;
            writeln!(temp_file, "Alice,25")?;
            
            let mut cmd = Command::cargo_bin("csvargs")?;
            cmd.arg("echo {{row['0'")
                .arg(temp_file.path());
            
            cmd.assert()
                .failure()
                .stderr(predicate::str::contains("Failed to parse template"));
            
            Ok(())
        }

        #[test]
        fn test_cli_nonexistent_file() -> Result<()> {
            let mut cmd = Command::cargo_bin("csvargs")?;
            cmd.arg("echo {{row['0']}}")
                .arg("/nonexistent/file.csv");
            
            cmd.assert()
                .failure()
                .stderr(predicate::str::contains("Failed to open file"));
            
            Ok(())
        }
    }
}