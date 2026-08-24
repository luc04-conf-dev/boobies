use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "boobies",
    version,
    about = "A serious Linux package manager with an unserious name.",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Root directory where packages are installed.
    #[arg(long, global = true, default_value = "/")]
    pub root: PathBuf,

    /// Configuration directory.
    #[arg(long, global = true)]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,

    /// Search the Vale do Boobies.
    #[arg(long = "search-in-the-valle", value_name = "QUERY")]
    pub search: Option<String>,

    /// Ask: "what is this?"
    #[arg(long = "what-is-ts", value_name = "PACKAGE")]
    pub info_flag: Option<String>,

    /// Print extra diagnostics.
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Make a package/system bigger by installing a package.
    Bigger {
        /// Package name or path to a .boob package.
        package: String,

        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Make a package/system smaller by removing a package.
    Smaller {
        /// Installed package name.
        package: String,

        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Grow the local repository database.
    Grow {
        /// Do not overwrite the local cache when the remote is unchanged.
        #[arg(long)]
        force: bool,
    },

    /// Expand: upgrade every installed package for which a newer version exists.
    Expand {
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// List installed packages.
    List,

    /// Show the version of boobies.
    Version,
}
