use clap::{Command, CommandFactory, FromArgMatches, crate_version};

use gwas_utils::{GuError, handle_broken_pipe};

mod subcommands;
use crate::subcommands::*;

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
    source: GuError,
    usage: &'static str,
}

fn main() {
    if let Err(err) = run() {
        handle_broken_pipe(&err.source);
        eprintln!("Error: {}", err.source);
        eprintln!("Usage: {}", err.usage);
        std::process::exit(1);
    }
}

fn run() -> Result<(), SubcommandError> {
    let mut cmd = Command::new("gu")
        .version(crate_version!())
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
        csv_concat_files,
        csv_filter_rows,
        csv_select_columns,
        csv_split_on_categorical_column,
        dnanexus_make_dxfuse_manifest,
        regenie_add_pval_col
    )
}

fn run_subcommand(
    usage: &'static str,
    execute: impl FnOnce() -> Result<(), GuError>,
) -> Result<(), SubcommandError> {
    execute().map_err(|source| SubcommandError { source, usage })
}
