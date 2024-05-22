use clap::{crate_version, Arg, ArgAction, Command};
use fuser::MountOption;

use mini_fs::fuse_interface::FuseFs;


fn main() {
    let matches = Command::new("mini-fs")
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
            Arg::new("auto_unmount")
                .long("auto_unmount")
                .action(ArgAction::SetTrue)
                .help("Automatically unmount on process exit"),
        )
        .arg(
            Arg::new("allow-root")
                .long("allow-root")
                .action(ArgAction::SetTrue)
                .help("Allow root user to access filesystem"),
        )
        .get_matches();
    env_logger::init();
    let mountpoint = matches.get_one::<String>("MOUNT_POINT").unwrap();
    let container_name = matches.get_one::<String>("CONTAINER").unwrap();
    let mut options = vec![MountOption::RO, MountOption::FSName("mini-fs".to_string())];
    if matches.get_flag("auto_unmount") {
        options.push(MountOption::AutoUnmount);
    }
    if matches.get_flag("allow-root") {
        options.push(MountOption::AllowRoot);
    }
    //TODO Check error message from here
    let fuse_fs = FuseFs::new(container_name.to_string()).unwrap();
    fuser::mount2(fuse_fs, mountpoint, &options).unwrap();
}
