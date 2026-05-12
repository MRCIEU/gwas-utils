# gwas-utils

`gu` - consists of a set of utilities which were written to do GWAS-related things in cloud environments. 

It compiles to a single (maybe totally self-contained / static) binary, so it comes without the (computational, resource, cognitive) overhead of maintaining a python / R installation + libraries (but also lacks their flexibility). A distroless docker image compiled on Alpine Linux using the Dockerfile in this repo gzips to ~1.3MB in size on disk. 

The utilities (subcommands under `gu`) and their usage are described below.

## subcommands

`csv_concat_files` - Concatenate multiple CSV output files into a single file

`csv_filter_rows` - Filter a CSV file based on logical expressions

`csv_select_columns` - Subset a CSV file to specific columns

`csv_split_on_categorical_column` - Split a CSV file into multiple output files based on the categories in a specified column

`dnanexus_make_dxfuse_manifest` - Convert a list of DNAnexus file identifiers into a dxfuse manifest file

`regenie_add_pval_col` - Add a P column to a Regenie output file based on the LOG10P column

## Installation

You will need to [install Rust](https://www.rust-lang.org/tools/install) first.

Then you should be able to install gwas-utils by running:

```
cargo install --git https://github.com/mrcieu/gwas-utils
```

and the binary will be installed somewhere in your `$PATH`:

```
gu csv_concat_files -h
```

or clone the repository and build it:

```
git clone https://github.com/mrcieu/gwas-utils
cd gwas-utils
cargo build --release
```

and the binary gets built in the repo's directory:

```
❯ ls target/release
gu
```

### Cross-compilation

You can cross-compile the software for different target platforms.

For example, if you want to build a static binary that will run on 64-bit linux with no run-time dependencies, and you're on OSX, you can [install the appropriate toolchain](https://github.com/FiloSottile/homebrew-musl-cross) (and configure `~/.cargo/config.toml`), then run:

```
cargo build --release --target x86_64-unknown-linux-musl
```

## Help / Usage

```
❯ gu -h
Usage: gu <COMMAND>

Commands:
  csv_concat_files                 Concatenate multiple CSV files into a single file
  csv_filter_rows                  Filter rows from a CSV file based on column-specific expressions
  csv_select_columns               Select specific columns from a CSV file
  csv_split_on_categorical_column  Split a CSV file into multiple files based on unique values in a specified categorical column
  dnanexus_make_dxfuse_manifest    Convert a list of dnanexus file identifiers into a dxfuse manifest file. File identifiers are extracted by regex: file-[a-zA-Z0-9]{24}.
  regenie_add_pval_col             Add a P column to a Regenie output file based on the LOG10P column. If the LOG10P value is large enough that the corresponding P value would be zero, then the P value is set to f64::MIN_POSITIVE
  help                             Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
❯ gu csv_concat_files -h
Concatenate multiple CSV output files into a single file. The input files must have the same header line. The output file will contain the header followed by all the records from the input files

Usage: csv_concat_files -i <infile1.csv[.gz] infile2.csv[.gz] ...> -d " " -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>...  CSV files to concatenate (can be gzipped if filenames end with .gz)
  -d, --delim <DELIM>     Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.) [default: \t]
  -o, --output <OUTPUT>   Concatenated CSV file to write (will be gzipped if filename ends with .gz)
  -h, --help              Print help
  -V, --version           Print version
```

```
❯ gu csv_filter_rows -h
Filter rows from a CSV file based on column-specific expressions

Usage: csv_filter_rows -i infile.csv[.gz] -e 'sex == male' -e 'age > 5' ... -d "," -o filtered.csv[.gz]

Options:
  -i, --input <INPUT>               Input CSV file (can be gzipped if filename ends with .gz)
  -e, --expression <EXPRESSION>...  Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<". Rows evaluating to true will be included in the output. Multiple expressions will be combined with AND logic by default (use --any for OR logic)
      --any                         Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
  -d, --delim <DELIM>               Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.) [default: \t]
  -o, --output <OUTPUT>             Filtered CSV file to write (will be gzipped if filename ends with .gz)
  -h, --help                        Print help
  -V, --version                     Print version
```

```
❯ gu csv_select_columns -h
Select specific columns from a CSV file.

Usage: csv_select_columns -i infile.csv[.gz] -d " " -c <column1 column2 ...> -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>         Input CSV file to process (can be gzipped if filename ends with .gz)
  -c, --columns <COLUMNS>...  Column names to select
  -d, --delim <DELIM>         Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.) [default: \t]
  -o, --output <OUTPUT>       Output file to write with selected columns (will be gzipped if filename ends with .gz)
  -h, --help                  Print help
  -V, --version               Print version
```

```
❯ gu csv_split_on_categorical_column -h
Split a CSV file into multiple files based on unique values in a specified categorical column.

Usage: csv_split_on_categorical_column -i infile.csv[.gz] -d " " -c colname

Options:
  -i, --input <INPUT>    Input CSV file (can be gzipped if filename ends with .gz)
  -c, --column <COLUMN>  Categorical column name to split on
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.) [default: \t]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu dnanexus_make_dxfuse_manifest -h
Convert a list of dnanexus file identifiers into a dxfuse manifest file. File identifiers are extracted by regex: file-[a-zA-Z0-9]{24}.

Usage: dnanexus_make_dxfuse_manifest -f <"file-xxxx" "file-yyyy" ...> -p ${DX_PROJECT_CONTEXT_ID} -o manifest.json

Options:
  -f, --fileids <FILEIDS>...   A list of dnanexus file identifiers (file-xxxx, {$dnanexus_link: file-yyyy}, etc.) to include in the manifest
  -p, --projectid <PROJECTID>  The ID of the dnanexus project containing the files
  -o, --output <OUTPUT>        JSON file to write the manifest to
  -h, --help                   Print help
  -V, --version                Print version
```

```
❯ gu regenie_add_pval_col -h
Add a P column to a Regenie output file based on the LOG10P column. If the LOG10P value is large enough that the corresponding P value would be zero, then the P value is set to f64::MIN_POSITIVE

Usage: regenie_add_pval_col -i infile.regenie[.gz] -o outfile.regenie[.gz]

Options:
  -i, --input <INPUT>    Regenie file to process (can be gzipped if filename ends with .gz)
  -o, --output <OUTPUT>  Output file to write with added p-value column (will be gzipped if filename ends with .gz)
  -h, --help             Print help
  -V, --version          Print version
```
