use clap::{crate_version, Arg, ArgAction, Command};
use fuser::MountOption;

use mini_fs::{fuse_interface::FuseFs, logger::Logger};

fn main() {
    let appname = "mini-fs";
    let matches = Command::new(appname)
        .version(crate_version!())
        .author("Martin")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(true)
                .index(1)
                .help("Act as a client, and mount FUSE at given path"),
        )
        .arg(
            Arg::new("CONTAINER")
                .required(true)
                .index(2)
                .help("The file that will contain everything"),
        )
        .arg(
            Arg::new("allow-notification")
                .long("allow-notification")
                .short('n')
                .action(ArgAction::SetTrue)
                .help("Activate desktop notifications"),
        )
        .get_matches();
    env_logger::init();
    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    let container_name = matches.get_one::<String>("CONTAINER").unwrap();
    let options = vec![MountOption::RW, MountOption::FSName(appname.to_string())];
    let logger = Logger::new(appname.to_string(), matches.get_flag("allow-notification"));
    //TODO Check error message from here
    let fuse_fs = FuseFs::new(container_name.to_string(), logger).unwrap();
    fuser::mount2(fuse_fs, mountpoint, &options).unwrap();
}
