# gwas-utils

`gu` consists of a set of subcommands which were written to do specific, GWAS-related tasks in cloud environments without the overhead of maintaining a python / R installation + libraries. 

A distroless docker image with `gu` compiled on Alpine Linux using the Dockerfile in this repo gzips to ~1.6MB in size on disk.

`gu` is a work in progress - subcommands + API are both subject to change at the moment.


## Installation

### Locally

You will need to [install Rust](https://www.rust-lang.org/tools/install) first (version 1.89 or newer; the minimum supported version is `rust-version` in `Cargo.toml`, and CI checks it).

Then you should be able to install gwas-utils by running:

```
cargo install --git https://github.com/mrcieu/gwas-utils
```

and the binary will be installed locally (in `$HOME/.cargo/bin` unless you have specified otherwise. Run `cargo help install` if you need to work out where):

```
~/.cargo/bin/gu -h
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


## Usage

### C[?]SV file handling

* By default, `gu csv <SUBCOMMAND>` will try to automatically detect the delimiter in the file it's reading. If that fails for some reason, you can specify it manually (usually using the `-d` flag). Automatically detected delimiters must be one of: `[',', '\t', ';', '|', ' ']`. A delimiter that appears the same number of times on every line (outside quotes) is preferred, so e.g. commas inside a free-text column of a TSV file won't confuse it.

* `gu csv <SUBCOMMAND>`s default to reading from `stdin` and writing to `stdout`, so you can chain multiple commands together to achieve what you want:

  ```
  gu csv filter myfile.csv -e 'CHROM == 1' 'P < 5E-8' |\
      gu csv select -c CHROM POS P |\
      gu csv delim -d "\t" -o chr1.signif.tsv
  ```

* `gu csv <SUBCOMMAND>` will automatically read gzipped input (detected from the file's contents, so it also works on `stdin` and with multi-member files such as bgzip output) and will write gzipped output if the output filename ends with `.gz`:

  ```
  gu csv delim input.csv.gz -d "\t" -o output.tsv.gz
  ```

* Missing values: in numeric columns, empty fields and `NA`, `N/A` and `NaN` (any case) are treated as missing. `gu csv filter` treats a missing value as not satisfying `<`, `<=`, `>` or `>=`; `gu csv addz` and `gu csv addp` write `NA` when an input value is missing; `gu csv stats lambda` skips them; and `gu csv merge` never matches rows whose key is missing (they are kept in left, right and outer joins). Any other non-numeric value is an error that names the column and line.

* A subcommand that is asked to do nothing (for example `gu csv select` with no columns, or `gu csv filter` with no expression) exits with an error rather than copying its input through.

## Help 

```
❯ gu -h
Usage: gu [OPTIONS] [COMMAND]

Commands:
  csv   Tools for working with CSV files
  dn    Tools for working with DNAnexus
  help  Print this message or the help of the given subcommand(s)

Options:
  -v, --version   Print version information
  -l              Print a summary of licence information
      --licences  Print full licence information, including all third-party notices
  -h, --help      Print help
```

```
❯ gu csv -h
Tools for working with CSV files

Usage: gu csv <COMMAND>

Commands:
  addp      Add a P column to a CSV file based on a (minus) LOG10P column
  addz      Add a Z-score column to a CSV file based on a beta column and a standard error column
  cat       Concatenate multiple CSV files into a single file
  delim     Change the delimiter of a CSV file
  filter    Filter rows from a CSV file based on column-specific expressions
  merge     Merge two CSV files based on a shared column
  reheader  Reheader a CSV file
  select    Select specific columns from a CSV file
  split     Split a CSV file into multiple files based on unique values in a specified categorical column
  stats     Calculate summary statistics from a CSV file
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

```
❯ gu csv addp -h
Add a P column to a CSV file based on a (minus) LOG10P column

Usage: gu csv addp infile.regenie[.gz] [-o outfile.regenie[.gz]]

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --log10p <LOG10P>  Name of column containing [optionally negative] LOG10P values [default: LOG10P]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
```

```
❯ gu csv addz -h
Add a Z-score column to a CSV file based on a beta column and a standard error column

Usage: gu csv addz infile.regenie[.gz] [-o outfile.regenie[.gz]]

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --beta <BETA>      Name of column containing [optionally negative] BETA values [default: BETA]
      --se <SE>          Name of column containing standard error values [default: SE]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
```

```
❯ gu csv cat -h
Concatenate multiple CSV files into a single file

Usage: gu csv cat infile1.csv[.gz] infile2.csv[.gz] ... [-o outfile.csv[.gz]]

Arguments:
  <INPUT>...  CSV files to concatenate (gzipped input is detected automatically)

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
```

```
❯ gu csv delim -h
Change the delimiter of a CSV file

Usage: gu csv delim infile.csv[.gz] -d"\t" [-o outfile.tsv[.gz]]

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
      --din <DIN>        Delimiter for INPUT CSV file [default: auto]
  -d, --delim <DELIM>    Delimiter for OUTPUT CSV file
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
```

```
❯ gu csv filter -h
Filter rows from a CSV file based on column-specific expressions

Usage: 
    gu csv filter infile.csv[.gz] -e 'sex == male' 'age > 5' ... [-o outfile.csv[.gz]]
    gu csv filter infile.csv[.gz] -c ALLELE1 -r "^[ACGT]$" [-o outfile.csv[.gz]]

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -e, --expression <EXPRESSION>...  Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<"
      --any                         Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
  -c, --column <COLUMN>             A single column name whose values will be matched against a single regular expression
  -r, --regex <REGEX>               Regular expression to apply (to the values in the column specified by -c)
  -v, --invert                      Invert the logic of the filter, i.e. keep rows that do NOT match the expression(s) or regex
  -d, --delim <DELIM>               Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>             CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                        Print help
```

```
❯ gu csv merge -h
Merge two CSV files based on a shared column

Usage: gu csv merge infile1.csv[.gz] infile2.csv[.gz] -c KEY_COLUMN [-o outfile.csv[.gz]]

Arguments:
  <INPUT> <INPUT>...  CSV files to merge (gzipped input is detected automatically). The first file is held in memory, so its keys should be unique (see --on-duplicate). The second file is streamed from disk for --join=inner and --join=right (see --strict); keys that repeat in it each produce an output row. Rows with a missing key (empty, NA) never match

Options:
  -c, --column <COLUMN>              Column name to merge on
  -j, --join <JOIN>                  Join type [default: inner] [possible values: inner, outer, left, right]
      --c2 <C2>                      Column name to merge on for second file, if different
      --on-duplicate <ON_DUPLICATE>  What to do if a key is repeated in the first file: error, or keep only the first or last row for that key (the kept row stays in the position of the first) [default: error] [possible values: error, first, last]
      --strict                       Load both files into memory and apply --on-duplicate to the second file as well as the first
      --d1 <D1>                      Delimiter for CSV file reading and writing [default: auto]
      --d2 <D2>                      Delimiter for the second CSV file [default: auto]
  -f, --fill <FILL>                  Fill string for missing values in unmatched rows (left, right, outer joins) [default: ""]
  -o, --output <OUTPUT>              CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                         Print help
```

```
❯ gu csv reheader -h
Reheader a CSV file

Usage: 
    gu csv reheader infile.csv[.gz] -l COL1 COL2 ... [-o outfile.csv[.gz]]
    gu csv reheader infile.csv[.gz] -c OLDCOL1=NEWCOL1 OLDCOL2=NEWCOL2 ... [-o outfile.csv[.gz]]

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -l, --list <LIST>...        New header to write (format: list of strings corresponding to new column names)
  -c, --columns <COLUMNS>...  Specific columns to rename (format: oldcolumn=newcolumn for arbitrary number of columns)
  -d, --delim <DELIM>         Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>       CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                  Print help
```

```
❯ gu csv select -h
Select specific columns from a CSV file

Usage: 
    gu csv select infile.csv[.gz] -c <column1 column2 ...> [-o outfile.csv[.gz]]
    gu csv select infile.csv[.gz] -i <index1 index2 ...> [-o outfile.csv[.gz]]
    gu csv select infile.csv[.gz] -r <REGEX1 REGEX2 ...> [-o outfile.csv[.gz]]
    gu csv select infile.csv[.gz] -v -c|-i|-r <...> [-o outfile.csv[.gz]]  (all columns EXCEPT those)

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -c, --columns <COLUMNS>...  Column names to select or...
  -i, --indices <INDICES>...  Column indices (1-based) to select or...
  -r, --regexes <REGEXES>...  Regexes to select column names against
  -d, --delim <DELIM>         Delimiter for CSV file reading and writing [default: auto]
      --no-reorder            Don't reorder selected columns
  -v, --invert                Invert the selection, i.e. keep every column EXCEPT the ones selected by -c, -i or -r (in their original order)
  -o, --output <OUTPUT>       CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                  Print help
```

```
❯ gu csv split -h
Split a CSV file into multiple files based on unique values in a specified categorical column

Usage: gu csv split infile.csv[.gz] -c colname

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -c, --column <COLUMN>  Categorical column name to split on. One file is written per distinct value, named COLNAME.VALUE.SUFFIX. Characters that can't be used in a file name ("/", control characters) and "%" are percent-encoded (e.g. "a/b" becomes "a%2Fb"). Values that differ only by case are an error, because their files can't be told apart on some file systems
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
  -s, --suffix <SUFFIX>  Suffix to add to output filenames (by default output files will be named "COLNAME.VAL.csv") [default: csv]
  -h, --help             Print help
```

```
❯ gu csv stats -h
Calculate summary statistics from a CSV file

Usage: gu csv stats <COMMAND>

Commands:
  lambda  Calculate the genomic inflation factor (lambda) from a P or (minus) LOG10P column
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

```
❯ gu csv stats lambda -h
Calculate the genomic inflation factor (lambda) from a P or (minus) LOG10P column

Usage: gu csv stats lambda infile.csv[.gz] [--p P_COLUMN | --log10p LOG10P_COLUMN] [-o outfile.txt[.gz]]

Arguments:
  [INPUT]  CSV file to process (gzipped input is detected automatically) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading [default: auto]
  -p, --p <P>            Name of column containing P values [default: P]
      --log10p <LOG10P>  Name of column containing [optionally negative] LOG10P values (use instead of --p)
  -o, --output <OUTPUT>  File to write lambda to (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
```

```
❯ gu dn -h
Tools for working with DNAnexus

Usage: gu dn <COMMAND>

Commands:
  make_dxfuse_manifest  Convert a list of dnanexus file identifiers into a dxfuse manifest file
  help                  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
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
```
