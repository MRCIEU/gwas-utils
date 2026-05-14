# gwas-utils

`gu` consists of a set of utilities which were written to do GWAS-related things in cloud environments. 

It compiles to a single (maybe totally self-contained / static) binary, so it comes without the (computational, resource, cognitive) overhead of maintaining a python / R installation + libraries (but also lacks their flexibility). A distroless docker image compiled on Alpine Linux using the Dockerfile in this repo gzips to ~1.3MB in size on disk. 

The utilities (subcommands under `gu`) and their usage are described below.

## Subcommands

`csvaddp` - Add a P column to a CSV file based on a LOG10P column

`csvconcat` - Concatenate multiple CSV output files into a single file

`csvdelim`  - Change the delimeter of a CSV file

`csvfilter` - Filter the rows of a CSV file based on logical expressions

`csvselect` - Subset a CSV file to specific columns

`csvsplit` - Split a CSV file into multiple output files based on the categories in a specified column

`make_dxfuse_manifest` - Convert a list of DNAnexus file identifiers into a dxfuse manifest file

## Installation

You will need to [install Rust](https://www.rust-lang.org/tools/install) first.

Then you should be able to install gwas-utils by running:

```
cargo install --git https://github.com/mrcieu/gwas-utils
```

and the binary will be installed somewhere in your `$PATH`:

```
gu csvconcat -h
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
  csvaddp               Add a P column to a CSV file based on a LOG10P column
  csvconcat             Concatenate multiple CSV files into a single file
  csvdelim              Change the delimeter of a CSV file
  csvfilter             Filter rows from a CSV file based on column-specific expressions
  csvselect             Select specific columns from a CSV file
  csvsplit              Split a CSV file into multiple files based on unique values in a specified categorical column
  make_dxfuse_manifest  Convert a list of dnanexus file identifiers into a dxfuse manifest file
  help                  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
❯ gu csvaddp -h
Add a P column to a CSV file based on a LOG10P column

Usage: gu csvaddp -i infile.regenie[.gz] -o outfile.regenie[.gz]

Options:
  -i, --input <INPUT>    Regenie file to process (can be gzipped if filename ends with .gz) [default: stdin]
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --log10p <LOG10P>  Column name for the LOG10P values [default: LOG10P]
  -o, --output <OUTPUT>  Output file to write with added p-value column (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csvconcat -h
Concatenate multiple CSV files into a single file

Usage: gu csvconcat -i infile1.csv[.gz] infile2.csv[.gz] ... -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>...  CSV files to concatenate (can be gzipped if filenames end with .gz)
  -d, --delim <DELIM>     Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>   Concatenated CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help              Print help
  -V, --version           Print version
```

```
❯ gu csvdelim -h
Change the delimeter of a CSV file

Usage: gu csvdelim -i infile.csv[.gz] -d"\t" -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>    CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]
  -d, --delim <DELIM>    Delimiter for OUTPUT CSV file
      --din <DIN>        Delimiter for INPUT CSV file [default: auto]
  -o, --output <OUTPUT>  Re-delimetered CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csvfilter -h
Filter rows from a CSV file based on column-specific expressions

Usage: gu csvfilter -i infile.csv[.gz] -e 'sex == male' 'age > 5' ... -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>               Input CSV file (can be gzipped if filename ends with .gz) [default: stdin]
  -e, --expression <EXPRESSION>...  Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<"
      --any                         Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
  -d, --delim <DELIM>               Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>             Filtered CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                        Print help
  -V, --version                     Print version
```

```
❯ gu csvselect -h
Select specific columns from a CSV file

Usage: gu csvselect -i infile.csv[.gz] -c <column1 column2 ...> -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>         Input CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]
  -c, --columns <COLUMNS>...  Column names to select
  -d, --delim <DELIM>         Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>       Output file to write with selected columns (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                  Print help
  -V, --version               Print version
```

```
❯ gu csvsplit -h
Split a CSV file into multiple files based on unique values in a specified categorical column

Usage: gu csvsplit -i infile.csv[.gz] -c colname -o outfile.csv[.gz]

Options:
  -i, --input <INPUT>    Input CSV file (can be gzipped if filename ends with .gz) [default: stdin]
  -c, --column <COLUMN>  Categorical column name to split on
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
  -s, --suffix <SUFFIX>  output suffix to add to output files (default is csv, so output files will be named colname.value.csv) [default: csv]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu make_dxfuse_manifest -h
Convert a list of dnanexus file identifiers into a dxfuse manifest file

Usage: gu make_dxfuse_manifest -f "file-xxxx" "file-yyyy" ... -p ${DX_PROJECT_CONTEXT_ID} -o manifest.json

Options:
  -f, --fileids <FILEIDS>...   A list of dnanexus file identifiers (file-xxxx, {$dnanexus_link: file-yyyy}, etc.) to include in the manifest
  -p, --projectid <PROJECTID>  The ID of the dnanexus project containing the files
  -o, --output <OUTPUT>        JSON file to write the manifest to
  -h, --help                   Print help
  -V, --version                Print version
```
