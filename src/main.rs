use clap::Arg;
use clap::{Command, CommandFactory, FromArgMatches, crate_version};

use gwas_utils::{GuError, handle_broken_pipe};

mod csv;
use crate::csv::stats::lambda;
use crate::csv::*;

mod dn;
use crate::dn::*;

/// The NOTICE file: this program's licence, and the notices of its third-party dependencies
const NOTICE: &str = include_str!("../NOTICE");

/// The opening statement of NOTICE, i.e. everything before the first third-party notice
fn get_notice_summary() -> &'static str {
    NOTICE
        .split_once("\n---")
        .map_or(NOTICE, |(summary, _)| summary)
}

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
        .disable_version_flag(true)
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .help("Print version information")
                .action(clap::ArgAction::Version),
        )
        .arg_required_else_help(true)
        .arg(
            Arg::new("licences_summary")
                .short('l')
                .help("Print a summary of licence information")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("licences")
                .long("licences")
                .alias("licenses")
                .help("Print full licence information, including all third-party notices")
                .action(clap::ArgAction::SetTrue),
        );

    let mut csv_cmg = Command::new("csv")
        .about("Tools for working with CSV files")
        .subcommand_required(true)
        .arg_required_else_help(true);

    let mut stats_cmg = Command::new("stats")
        .about(stats::ABOUT)
        .subcommand_required(true)
        .arg_required_else_help(true);

    let mut dn_cmg = Command::new("dn")
        .about("Tools for working with DNAnexus")
        .subcommand_required(true)
        .arg_required_else_help(true);

    add_subcommands!(
        csv_cmg, addp, addz, cat, delim, filter, merge, regenify, reheader, select, split
    );
    add_subcommands!(stats_cmg, lambda);
    add_subcommands!(dn_cmg, make_dxfuse_manifest);

    csv_cmg = csv_cmg.subcommand(stats_cmg);
    cmd = cmd.subcommand(csv_cmg).subcommand(dn_cmg);

    let matches = cmd.get_matches();

    if let Some(true) = matches.get_one::<bool>("licences") {
        print!("{NOTICE}");
        return Ok(());
    }

    if let Some(true) = matches.get_one::<bool>("licences_summary") {
        println!("{}", get_notice_summary());
        println!(
            "Use \"gu --licences\" to print the full text, including all third-party notices."
        );
        return Ok(());
    }

    match matches.subcommand() {
        Some(("csv", csv_matches)) => match csv_matches.subcommand() {
            Some(("stats", stats_matches)) => match_subcommand!(stats_matches, lambda),
            _ => match_subcommand!(
                csv_matches,
                addp,
                addz,
                cat,
                delim,
                filter,
                merge,
                regenify,
                reheader,
                select,
                split
            ),
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notice_summary_stops_before_first_third_party_notice() {
        let summary = get_notice_summary();
        assert!(summary.starts_with("gwas-utils is licensed under the MIT License"));
        assert!(!summary.contains("---"));
        assert!(summary.len() < NOTICE.len());
    }
}
