# gwas-utils

`gu` consists of a set of subcommands which were written to do specific, GWAS-related tasks in cloud environments without the overhead of maintaining a python / R installation + libraries. 

A distroless docker image compiled on Alpine Linux using the Dockerfile in this repo gzips to ~1.3MB in size on disk.

`gu` is a work in progress - subcommands + API are both subject to change at the moment.


## Subcommands

`csv addp` - Add a P column to a CSV file based on a LOG10P column

`csv concat` - Concatenate multiple CSV output files into a single file

`csv delim`  - Change the delimeter of a CSV file

`csv filter` - Filter the rows of a CSV file based on logical expressions

`csv select` - Subset a CSV file to specific columns

`csv split` - Split a CSV file into multiple output files based on the categories in a specified column

`dn make_dxfuse_manifest` - Convert a list of DNAnexus file identifiers into a dxfuse manifest file


## Installation

### Locally

You will need to [install Rust](https://www.rust-lang.org/tools/install) first.

Then you should be able to install gwas-utils by running:

```
cargo install --git https://github.com/mrcieu/gwas-utils
```

and the binary will be installed somewhere in your `$PATH`:

```
gu -h
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

### Docker

There is a Dockerfile in this repository which you can use to make a distroless image containing `gu` (and nothing else)

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
  csv   Tools for working with CSV files
  dn    Tools for working with DNAnexus
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
❯ gu csv -h
Tools for working with CSV files

Usage: gu csv <COMMAND>

Commands:
  addp    Add a P column to a CSV file based on a LOG10P column
  concat  Concatenate multiple CSV files into a single file
  delim   Change the delimeter of a CSV file
  filter  Filter rows from a CSV file based on column-specific expressions
  select  Select specific columns from a CSV file
  split   Split a CSV file into multiple files based on unique values in a specified categorical column
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
❯ gu csv addp -h
Add a P column to a CSV file based on a LOG10P column

Usage: gu csv addp infile.regenie[.gz] [-o outfile.regenie[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --log10p <LOG10P>  Name of column containing the LOG10P values [default: LOG10P]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csv concat -h
Concatenate multiple CSV files into a single file

Usage: gu csv concat infile1.csv[.gz] infile2.csv[.gz] ... [-o outfile.csv[.gz]]

Arguments:
  <INPUT>...  CSV files to concatenate (can be gzipped if filenames end with .gz)

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csv delim -h
Change the delimeter of a CSV file

Usage: gu csv delim infile.csv[.gz] -d"\t" [-o outfile.tsv[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
      --din <DIN>        Delimiter for INPUT CSV file [default: auto]
  -d, --delim <DELIM>    Delimiter for OUTPUT CSV file
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csv filter -h
Filter rows from a CSV file based on column-specific expressions

Usage: gu csv filter infile.csv[.gz] -e 'sex == male' 'age > 5' ... [-o outfile.csv[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -e, --expression <EXPRESSION>...  Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<"
      --any                         Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
  -d, --delim <DELIM>               Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>             CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                        Print help
  -V, --version                     Print version
```

```
❯ gu csv select -h
Select specific columns from a CSV file

Usage: gu csv select infile.csv[.gz] -c <column1 column2 ...> [-o outfile.csv[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -c, --columns <COLUMNS>...  Column names to select
  -d, --delim <DELIM>         Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>       CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                  Print help
  -V, --version               Print version
```

```
❯ gu csv split -h
Split a CSV file into multiple files based on unique values in a specified categorical column

Usage: gu csv split infile.csv[.gz] -c colname

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -c, --column <COLUMN>  Categorical column name to split on
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
  -s, --suffix <SUFFIX>  Suffix to add to output filenames (by default output files will be named "COLNAME.VAL.csv") [default: csv]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu dn -h
Tools for working with DNAnexus

Usage: gu dn <COMMAND>

Commands:
  make_dxfuse_manifest  Convert a list of dnanexus file identifiers into a dxfuse manifest file
  help                  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
❯ gu dn make_dxfuse_manifest -h
Convert a list of dnanexus file identifiers into a dxfuse manifest file

Usage: gu dn make_dxfuse_manifest -f "file-xxxx" "file-yyyy" ... -p ${DX_PROJECT_CONTEXT_ID} [-o manifest.json]

Options:
  -f, --fileids <FILEIDS>...   A list of DNAnexus file identifiers (file-xxxx, {$dnanexus_link: file-yyyy}, etc.) to include in the manifest
  -p, --projectid <PROJECTID>  The ID of the DNAnexus project containing the files
  -o, --output <OUTPUT>        JSON file to write the manifest to [default: stdout]
  -h, --help                   Print help
  -V, --version                Print version
```
