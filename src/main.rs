use clap::Arg;
use clap::{Command, CommandFactory, FromArgMatches, crate_version};

use gwas_utils::{GuError, handle_broken_pipe};

mod csv;
use crate::csv::*;

mod dn;
use crate::dn::*;

mod licences;
use crate::licences::*;

macro_rules! add_subcommands {
    ($cmd:expr, $($module:ident),*) => {
        $cmd = $cmd
            $(
                .subcommand(
                    Command::new(stringify!($module))
                        .about($module::ABOUT)
                        .override_usage($module::get_usage())
                        .args($module::Args::command().get_arguments()),
                )
            )*
    };
}

macro_rules! match_subcommand {
    ($matches:expr, $($module:ident),*) => {
        match $matches.subcommand() {
            $(
                Some((stringify!($module), sub_m)) => run_subcommand($module::get_usage(), || {
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
    usage: String,
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
        .arg_required_else_help(true)
        .arg(
            Arg::new("licences")
                .short('l')
                .long("licences")
                .alias("licenses")
                .help("Print licence information")
                .action(clap::ArgAction::SetTrue),
        );

    let mut csv_cmg = Command::new("csv")
        .about("Tools for working with CSV files")
        .subcommand_required(true)
        .arg_required_else_help(true);

    let mut dn_cmg = Command::new("dn")
        .about("Tools for working with DNAnexus")
        .subcommand_required(true)
        .arg_required_else_help(true);

    add_subcommands!(
        csv_cmg, addp, addz, concat, delim, filter, merge, regenify, reheader, select, split
    );
    add_subcommands!(dn_cmg, make_dxfuse_manifest);

    cmd = cmd.subcommand(csv_cmg).subcommand(dn_cmg);

    let matches = cmd.get_matches();

    if let Some(true) = matches.get_one::<bool>("licences") {
        println!("gwas-utils uses the following dependencies:\n");
        println!("clap:\n{}\n", LICENCE_CLAP);
        println!("csv:\n{}\n", LICENCE_CSV);
        println!("flate2:\n{}\n", LICENCE_FLATE2);
        println!("regex:\n{}\n", LICENCE_REGEX);
        println!("serde_json:\n{}\n", LICENCE_SERDE);
        println!("thiserror:\n{}\n", LICENCE_THISERROR);
        return Ok(());
    }

    match matches.subcommand() {
        Some(("csv", csv_matches)) => {
            match_subcommand!(
                csv_matches,
                addp,
                addz,
                concat,
                delim,
                filter,
                merge,
                regenify,
                reheader,
                select,
                split
            )
        }
        Some(("dn", dn_matches)) => match_subcommand!(dn_matches, make_dxfuse_manifest),
        _ => unreachable!(),
    }
}

fn run_subcommand(
    usage: String,
    execute: impl FnOnce() -> Result<(), GuError>,
) -> Result<(), SubcommandError> {
    execute().map_err(|source| SubcommandError { source, usage })
}
