# gwas-utils

Command-line utilities to do GWAS-related things

## Constituent programs

`csv_concat_files` - Concatenate multiple CSV output files into a single file

`csv_select_columns` - Subset a CSV file to specific columns

`dnanexus_make_dxfuse_manifest` - Convert a list of DNAnexus file identifiers into a dxfuse manifest file

`regenie_add_pval_col` - Add a P column to a Regenie output file based on the LOG10P column

## Installation

You will need to [install Rust](https://www.rust-lang.org/tools/install) first.

Then you should be able to install gwas-utils by running:

```
cargo install --git https://github.com/mrcieu/gwas-utils
```

and the binaries will be installed somewhere in your `$PATH`:

```
csv_concat_files -h
```

or clone the repository and build it:

```
git clone https://github.com/mrcieu/gwas-utils
cd gwas-utils
cargo build --release
```

and the binaries get built in the repo's directory:

```
❯ ls target/release
...
... csv_select_columns ... csv_concat_files ... dnanexus_make_dxfuse_manifest ... regenie_add_pval_col ...
```

### Cross-compilation

You can cross-compile the software for different target platforms.

For example, if you want to build a static binary that will run on 64-bit linux with no run-time dependencies, and you're on OSX, you can [install the appropriate toolchain](https://github.com/FiloSottile/homebrew-musl-cross) (and configure `~/.cargo/config.toml`), then run:

```
cargo build --release --target x86_64-unknown-linux-musl
```

## Help / Usage

```
❯ csv_concat_files -h
Concatenate multiple CSV output files into a single file...

Usage: csv_concat_files -i <infile1.regenie[.gz] infile2.regenie[.gz] ...> -o outfile.regenie[.gz]

Options:
  -i, --input <INPUT> <INPUT>...     CSV output files to concatenate (can be gzipped if filename ends with .gz)
  -d, --delim <DELIM>                Delimiter for CSV file reading and writing (default is tab, use " " for space, etc.) [default: \t]
  -o, --output <OUTPUT>              Concatenated CSV file to write (will be gzipped if filename ends with .gz)
  -h, --help                         Print help
  -V, --version                      Print version
```

```
❯ csv_select_columns -h
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
❯ dnanexus_make_dxfuse_manifest -h
Convert a list of dnanexus file identifiers into a dxfuse manifest file. File identifiers are extracted by regex: file-[a-zA-Z0-9]{24}.

Usage: dnanexus_make_dxfuse_manifest -f <"file-xxxx" "file-yyyy" ...> -p ${DX_PROJECT_CONTEXT_ID} -o manifest.json

Options:
  -f, --fileids <FILEIDS>...   A list of dnanexus file identifiers (file-xxxx, {$dnanexus_link: file-yyyy}, etc.) to include in the manifest
  -p, --projectid <PROJECTID>  The ID of the dnanexus project containing the files
  -o, --output <OUTPUT>        The name of the output file to write the manifest to
  -h, --help                   Print help
  -V, --version                Print version
```

```
❯ regenie_add_pval_col -h
Add a P column to a Regenie output file based on the LOG10P column...

Usage: regenie_add_pval_col -i infile.regenie[.gz] -o outfile.regenie[.gz]

Options:
  -i, --input <INPUT>    Regenie output file to process (can be gzipped if filename ends with .gz)
  -o, --output <OUTPUT>  Output file to write with added p-value column (will be gzipped if filename ends with .gz)
  -h, --help             Print help (see more with '--help')
  -V, --version          Print version
```
