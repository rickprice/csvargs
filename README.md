# csvargs

A command-line tool for processing CSV files with Jinja2 templates and executing commands on each row.

## Overview

`csvargs` allows you to read CSV files, apply Jinja2 templates to each row, and execute the resulting commands. This is useful for batch processing tasks where you need to run commands based on data in CSV files.

## Features

- Process CSV files with or without headers
- Use Jinja2 templates to generate commands from CSV data
- Execute shell commands for each row
- Robust error handling and reporting
- Cross-platform support (Windows, Linux, macOS)

## Installation

Make sure you have Rust installed, then build from source:

```bash
cargo build --release
```

The binary will be available at `target/release/csvargs`.

## Usage

```bash
csvargs [OPTIONS] <TEMPLATE> <FILES>...
```

### Arguments

- `TEMPLATE`: Jinja2 template string for generating commands
- `FILES`: One or more CSV files to process

### Options

- `--no-header`: Treat CSV files as having no header row (data starts from first row)

## Examples

### With Headers

Given a CSV file `users.csv`:
```csv
name,email,age
Alice,alice@example.com,25
Bob,bob@example.com,30
```

Execute commands using column names:
```bash
csvargs "echo 'Hello {{row.name}}, your email is {{row.email}}'" users.csv
```

### Without Headers

Given a CSV file `data.csv`:
```csv
Alice,alice@example.com,25
Bob,bob@example.com,30
```

Execute commands using column indices:
```bash
csvargs --no-header "echo 'Hello {{row['0']}}, your email is {{row['1']}}'" data.csv
```

### Complex Commands

Create directories based on CSV data:
```bash
csvargs "mkdir -p /tmp/users/{{row.name}} && echo 'Created directory for {{row.name}}'" users.csv
```

### Multiple Files

Process multiple CSV files at once:
```bash
csvargs "echo 'Processing {{row.name}}'" file1.csv file2.csv file3.csv
```

## Template Syntax

csvargs uses Jinja2 templating. The CSV row data is available as the `row` variable:

- **With headers**: Access columns by name: `{{row.column_name}}`
- **Without headers**: Access columns by index: `{{row['0']}}`, `{{row['1']}}`, etc.

## Error Handling

- Invalid templates will be caught before processing begins
- File access errors are reported with context
- Command execution failures include status codes and error output
- Row-level errors include the row number for easy debugging

## Testing

Run the test suite:

```bash
cargo test
```

## License

BSD 3-Clause License - see [LICENSE](LICENSE) file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request
