use clap::{Command, CommandFactory, FromArgMatches};

mod csv_concat_files;
mod csv_filter_rows;
mod csv_select_columns;
mod csv_split_on_categorical_column;
mod dnanexus_make_dxfuse_manifest;
mod regenie_add_pval_col;

macro_rules! add_subcommands {
    ($cmd:expr, $($module:ident),*) => {
        $cmd = $cmd
            $(
                .subcommand(
                    Command::new(stringify!($module))
                        .about($module::ABOUT)
                        .override_usage($module::USAGE)
                        .args($module::Args::command().get_arguments()),
                )
            )*
    };
}

macro_rules! match_subcommand {
    ($matches:expr, $($module:ident),*) => {
        match $matches.subcommand() {
            $(
                Some((stringify!($module), sub_m)) => run_subcommand($module::USAGE, || {
                    let args = $module::Args::from_arg_matches(sub_m)?;
                    $module::run(args)?;
                    Ok(())
                }),
            )*
            _ => unreachable!(),
        }
    };
}

struct SubcommandError {
    source: Box<dyn std::error::Error>,
    usage: &'static str,
}

impl std::fmt::Debug for SubcommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::fmt::Display for SubcommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for SubcommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.source)
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {}", err.source);
        eprintln!("Usage: {}", err.usage);
        std::process::exit(1);
    }
}

fn run() -> Result<(), SubcommandError> {
    let mut cmd = Command::new("gu")
        .version(env!("CARGO_PKG_VERSION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true);

    add_subcommands!(
        cmd,
        csv_concat_files,
        csv_filter_rows,
        csv_select_columns,
        csv_split_on_categorical_column,
        dnanexus_make_dxfuse_manifest,
        regenie_add_pval_col
    );

    let matches = cmd.get_matches();

    match_subcommand!(
        matches,
        csv_filter_rows,
        csv_concat_files,
        csv_select_columns,
        csv_split_on_categorical_column,
        dnanexus_make_dxfuse_manifest,
        regenie_add_pval_col
    )
}

fn run_subcommand(
    usage: &'static str,
    execute: impl FnOnce() -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), SubcommandError> {
    execute().map_err(|source| SubcommandError { source, usage })
}
