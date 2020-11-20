//! A syntax checker for Age of Empires 2 random map scripts.
//!
//! ```bash
//! rms-check "/path/to/aoc/Random/Everything_Random_v4.3.rms"
//! ```

mod check;
mod cli_reporter;
mod highlight;
mod language_server;
mod zip_rms;

use crate::check::{cli_check, cli_fix, CheckArgs};
use crate::language_server::cli_server;
use crate::zip_rms::{cli_pack, cli_unpack};
use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rms_check::{Compatibility, FormatOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use structopt::StructOpt;

// CLI flags for selecting a compatibility level.
#[derive(Debug, StructOpt)]
struct CliCompat {
    /// Set the default compatibility to Age of Conquerors. Scripts can override this using
    /// `/* Compatibility: */` comments.
    #[structopt(long = "aoc")]
    aoc: bool,
    /// Set the default compatibility to UserPatch 1.4. Scripts can override this using
    /// `/* Compatibility: */` comments.
    #[structopt(long = "up14")]
    userpatch14: bool,
    /// Set the default compatibility to UserPatch 1.5. Scripts can override this using
    /// `/* Compatibility: */` comments.
    #[structopt(long = "up15")]
    userpatch15: bool,
    /// Set the default compatibility to HD Edition. Scripts can override this using
    /// `/* Compatibility: */` comments.
    #[structopt(long = "hd")]
    hd_edition: bool,
    /// Set the default compatibility to WololoKingdoms. Scripts can override this using
    /// `/* Compatibility: */` comments.
    #[structopt(long = "wk")]
    wololo_kingdoms: bool,
    /// Set the default compatibility to Definitive Edition. Scripts can override this using
    /// `/* Compatibility: */` comments.
    #[structopt(long = "de")]
    definitive_edition: bool,
}

impl CliCompat {
    fn to_compatibility(&self) -> Compatibility {
        if self.definitive_edition {
            Compatibility::DefinitiveEdition
        } else if self.wololo_kingdoms {
            Compatibility::WololoKingdoms
        } else if self.hd_edition {
            Compatibility::HDEdition
        } else if self.userpatch14 {
            Compatibility::UserPatch14
        } else if self.userpatch15 {
            Compatibility::UserPatch15
        } else if self.aoc {
            Compatibility::Conquerors
        } else {
            Compatibility::All
        }
    }
}

#[derive(Debug, StructOpt)]
struct CliCheck {
    /// The file to check.
    file: PathBuf,
    #[structopt(flatten)]
    compat_flags: CliCompat,
}

#[derive(Debug, StructOpt)]
enum CliCommand {
    /// Pack a folder into an Zip-RMS map.
    #[structopt(name = "pack")]
    Pack {
        output: PathBuf,
        #[structopt(long, short = "d")]
        indir: PathBuf,
        #[structopt(long, short = "w")]
        watch: bool,
    },
    /// Unpack a Zip-RMS map into a folder.
    #[structopt(name = "unpack")]
    Unpack {
        #[structopt(long, short = "o")]
        outdir: PathBuf,
        input: PathBuf,
    },
    /// Auto-fix problems with a random map script.
    #[structopt(name = "fix")]
    Fix {
        /// Don't write the results.
        #[structopt(long = "dry-run")]
        dry_run: bool,
        /// The file to check.
        file: PathBuf,
        #[structopt(flatten)]
        compat_flags: CliCompat,
    },
    /// Format the given file.
    #[structopt(name = "format")]
    Format {
        /// The file to format. Use "-" to read from standard input.
        file: PathBuf,
        /// Set the size in spaces of a single tab indentation.
        #[structopt(long = "tab-size", default_value = "2")]
        tab_size: u32,
        /// Whether to use spaces instead of tabs for indentation.
        #[structopt(long = "no-use-spaces")]
        no_use_spaces: bool,
        /// Whether to align arguments in a list of commands.
        #[structopt(long = "no-align-arguments")]
        no_align_arguments: bool,
    },
    /// Syntax check and lint a random map script.
    #[structopt(name = "check")]
    Check(CliCheck),
    /// Start the language server.
    #[structopt(name = "server")]
    Server,
}

/// Syntax checking and linting tool suite for Age of Empires 2 random map scripts.
#[derive(Debug, StructOpt)]
#[structopt(name = "rms-check")]
pub struct Cli {
    #[structopt(subcommand)]
    command: Option<CliCommand>,
    // Compatibility flags for implicit `check`, when not using any subcommand.
    #[structopt(flatten)]
    compat_flags: CliCompat,
    /// The file to check, when not using any subcommand.
    file: Option<String>,
}

fn read_input(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let stdin = PathBuf::from("-");
    if path.as_ref() == stdin {
        let mut bytes = vec![];
        io::stdin().read_to_end(&mut bytes)?;
        Ok(bytes)
    } else {
        std::fs::read(path)
    }
}

/// Watch a directory for changes, and call the `callback` when something changes.
fn cli_watch(indir: impl AsRef<Path>, callback: impl Fn() -> Result<()>) -> Result<()> {
    callback()?;
    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(500))?;
    watcher.watch(indir.as_ref(), RecursiveMode::Recursive)?;

    while let Ok(_event) = rx.recv() {
        match callback() {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::from_args();

    match args.command {
        Some(CliCommand::Unpack { outdir, input }) => cli_unpack(input, outdir),
        Some(CliCommand::Pack {
            indir,
            output,
            watch,
        }) => {
            if watch {
                cli_watch(&indir, || {
                    cli_pack(&indir, &output)?;
                    println!("wrote {:?}", output);
                    Ok(())
                })
            } else {
                cli_pack(indir, output)
            }
        }
        Some(CliCommand::Fix {
            dry_run,
            file,
            compat_flags,
        }) => cli_fix(CheckArgs {
            compatibility: compat_flags.to_compatibility(),
            file,
            dry_run,
        }),
        Some(CliCommand::Format {
            file,
            tab_size,
            no_use_spaces,
            no_align_arguments,
        }) => {
            let options = FormatOptions::default()
                .tab_size(tab_size)
                .use_spaces(!no_use_spaces)
                .align_arguments(!no_align_arguments);

            let bytes = read_input(file)?;
            let string = std::str::from_utf8(&bytes)?;
            let formatted = rms_check::format(string, options);
            highlight::highlight_to(&formatted, std::io::stdout())?;
            Ok(())
        }
        Some(CliCommand::Server) => {
            cli_server();
            unreachable!();
        }
        Some(CliCommand::Check(args)) => cli_check(CheckArgs {
            compatibility: args.compat_flags.to_compatibility(),
            file: args.file,
            ..Default::default()
        }),
        None => {
            let args = CliCheck::from_args();
            cli_check(CheckArgs {
                compatibility: args.compat_flags.to_compatibility(),
                file: args.file,
                ..Default::default()
            })
        }
    }
}
