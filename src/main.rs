use clap::Parser;
use fuser::MountOption;
use anyhow::{Context, Result};

use mini_fs::{fuse_interface::FuseFs, logger::Logger};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    mountpoint : String,
    container : String,
    #[arg(short = 'n', long)]
    allow_notification : bool,
}

fn main() -> Result<()>{
    let appname = "mini-fs";
    let cli = Cli::parse();
    let options = vec![MountOption::RW, MountOption::FSName(appname.to_string())];
    let logger = Logger::new(appname.to_string(), cli.allow_notification);
    let fuse_fs = FuseFs::new(cli.container, logger)?;
    fuser::mount2(fuse_fs, cli.mountpoint, &options).context("fuser::mount2 ")?;
    Ok(())
}
