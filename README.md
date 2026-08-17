# gwas-utils

`gu` consists of a set of subcommands which were written to do specific, GWAS-related tasks in cloud environments without the overhead of maintaining a python / R installation + libraries. 

A distroless docker image with `gu` compiled on Alpine Linux using the Dockerfile in this repo gzips to ~1.3MB in size on disk.

`gu` is a work in progress - subcommands + API are both subject to change at the moment.


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


## Usage

### C[?]SV file handling

* By default, `gu csv <SUBCOMMAND>` will try to automatically detect the delimeter in the file it's reading. If that fails for some reason, you can specify it manually (usually using the `-d` flag). Automatically detected delimeters must be one of: `[',', '\t', ';', '|', ' ']`.

* `gu csv <SUBCOMMAND>`s default to reading from `stdin` and writing to `stdout`, so you can chain multiple commands together to achieve what you want:

  ```
  gu csv filter myfile.csv -e 'CHROM == 1' 'P < 5E-8' |\
      gu csv select -c CHROM POS P |\
      gu csv delim -d "\t" -o chr1.signif.tsv
  ```

* `gu csv <SUBCOMMAND>` will automatically read and write gzipped files if the filenames end with `.gz`:

  ```
  gu csv regenify input.csv.gz -o output.tsv.gz
  ```

## Help 

```
❯ gu -h
Usage: gu [OPTIONS] [COMMAND]

Commands:
  csv   Tools for working with CSV files
  dn    Tools for working with DNAnexus
  help  Print this message or the help of the given subcommand(s)

Options:
  -l, --licences  Print licence information
  -h, --help      Print help
  -V, --version   Print version
```

```
❯ gu csv -h
Tools for working with CSV files

Usage: gu csv <COMMAND>

Commands:
  addp      Add a P column to a CSV file based on a (minus) LOG10P column
  addz      Add a Z-score column to a CSV file based on a beta column and a standard error column
  concat    Concatenate multiple CSV files into a single file
  delim     Change the delimeter of a CSV file
  filter    Filter rows from a CSV file based on column-specific expressions
  merge     Merge two CSV files based on a shared column. Duplicate keys are not handled sensibly
  regenify  Write a tab separated file with missing data replaced by "NA"s
  reheader  Reheader a CSV file
  select    Select specific columns from a CSV file
  split     Split a CSV file into multiple files based on unique values in a specified categorical column
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```
❯ gu csv addp -h
Add a P column to a CSV file based on a (minus) LOG10P column

Usage: gu csv addp infile.regenie[.gz] [-o outfile.regenie[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --log10p <LOG10P>  Name of column containing [optionally negative] LOG10P values [default: LOG10P]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csv addz -h
Add a Z-score column to a CSV file based on a beta column and a standard error column

Usage: gu csv addz infile.regenie[.gz] [-o outfile.regenie[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --beta <BETA>      Name of column containing [optionally negative] BETA values [default: BETA]
      --se <SE>          Name of column containing standard error values [default: SE]
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

Usage: 
    gu csv filter infile.csv[.gz] -e 'sex == male' 'age > 5' ... [-o outfile.csv[.gz]]
    gu csv filter infile.csv[.gz] -c ALLELE1 -r "^[ACGT]$" [-o outfile.csv[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -e, --expression <EXPRESSION>...  Expression(s) to filter rows, in the format "COLUMN-NAME OPERATOR VALUE". Possible operators are: "==", "!=", ">=", "<=", ">", "<"
      --any                         Rows will be included in the output if any expression is true (default is to include rows only if all expressions are true)
  -c, --column <COLUMN>             A single column name whose values will be matched against a single regular expression
  -r, --regex <REGEX>               Regular expression to apply (to the values in the column specified by -c)
  -v, --invert                      Invert the logic of the filter, i.e. keep rows that do NOT match the expression(s) or regex
  -d, --delim <DELIM>               Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>             CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                        Print help
  -V, --version                     Print version
```

```
❯ gu csv merge -h
Merge two CSV files based on a shared column. Duplicate keys are not handled sensibly

Usage: gu csv merge infile1.csv[.gz] infile2.csv[.gz] -c KEY_COLUMN [-o outfile.csv[.gz]]

Arguments:
  <INPUT> <INPUT>...  CSV files to merge (can be gzipped if filenames end with .gz). Right-hand file is streamed from disk for --join=inner and --join=right

Options:
  -c, --column <COLUMN>  Column name to merge on
  -j, --join <JOIN>      Join type [default: inner] [possible values: inner, outer, left, right]
      --c2 <C2>          Column name to merge on for second file, if different
      --d1 <D1>          Delimiter for CSV file reading and writing [default: auto]
      --d2 <D2>          Delimiter for the second CSV file [default: auto]
  -f, --fill <FILL>      Fill string for missing values in unmatched rows (left, right, outer joins) [default: ""]
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csv regenify -h
Write a tab separated file with missing data replaced by "NA"s

Usage: gu csv regenify infile.csv[.gz] [-o outfile.tsv[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -d, --delim <DELIM>    Delimiter for CSV file reading and writing [default: auto]
      --no-rep-neg-one   Don't replace -1 with NA
  -o, --output <OUTPUT>  CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help             Print help
  -V, --version          Print version
```

```
❯ gu csv reheader -h
Reheader a CSV file

Usage: 
    gu csv reheader infile.csv[.gz] -l COL1 COL2 ... [-o outfile.csv[.gz]]
    gu csv reheader infile.csv[.gz] -c OLDCOL1=NEWCOL1 OLDCOL2=NEWCOL2 ... [-o outfile.csv[.gz]]

Arguments:
  [INPUT]  CSV file to process (can be gzipped if filename ends with .gz) [default: stdin]

Options:
  -l, --list <LIST>...        New header to write (format: list of strings corresponding to new columm names)
  -c, --columns <COLUMNS>...  Specific columns to rename (format: oldcolumn=newcolumn for arbitrary number of columns)
  -d, --delim <DELIM>         Delimiter for CSV file reading and writing [default: auto]
  -o, --output <OUTPUT>       CSV file to write (will be gzipped if filename ends with .gz) [default: stdout]
  -h, --help                  Print help
  -V, --version               Print version
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
      --no-reorder            Don't reorder selected columns
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
