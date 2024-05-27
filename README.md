# Mini-FS : A Container-Based Filesystem

Mini-FS is a containerized filesystem built on FUSE (Filesystem in Userspace).
It operates by mounting a container file, allowing all files and directories
created within the mount point to be stored within this container file.

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [Limitations and Optimization Opportunities](#limitations-and-optimization-opportunities)
- [License](#license)

## Installation
This project has been tested on Archlinux with Rust 1.77.

### Dependencies
This project relies on the following crates:
- [FUSE](https://github.com/libfuse/libfuse) and the Rust crate [fuser](https://github.com/cberner/fuser) for handling filesystem-related calls.
- [notify-rust](https://github.com/hoodie/notify-rust) for desktop notifications.
- [serde](https://github.com/serde-rs/serde) and [bincode](https://github.com/bincode-org/bincode) for binary serialization.
- [heapless](https://github.com/rust-embedded/heapless) for easily serializable structures.
- [clap](https://github.com/clap-rs/clap) for parsing command-line parameters.
- [anyhow](https://github.com/dtolnay/anyhow) for handling errors throughout the entire program.


[FUSE for Linux] is available in most Linux distributions and usually called `fuse` or `fuse3`. See installation [guide](https://github.com/cberner/fuser?tab=readme-ov-file#dependencies) for most distributions.

### Compilation

```sh
cargo build
```

### Tests
Mini-FS includes unit tests that can be executed as follows:
```sh
cargo test
```

Additionally, there's a basic filesystem test script provided. After mounting a
container filesystem on `mountpoint/` (refer to [usage](#usage)), run the
following command:
```sh
./test.sh mountpoint
```

This script conducts file creation, reading, writing, and deletion operations based on basic shell operations.

## Usage
To mount the container file `container_file` to `./mountpoint/`, follow these steps:
```sh
mkdir mountpoint
./target/debug/mini-fs mountpoint container_file
```

Once mounted, you can use `mountpoint/` as a directory, and all files and directories will be stored in `container_file`.
For example:
```sh
mkdir mountpoint/ocean
echo "Whale swim like otters" > mountpoint/ocean/whale.txt
cat mountpoint/ocean/whale.txt
```
This creates a directory `ocean` inside `mountpoint/`, writes a text file `whale.txt` with the specified content, and then displays the contents of `whale.txt`.

### Notification
Mini-FS features a basic notification system that can be enabled using the option `-n` or `--allow-notification`.

To enable notifications, use one of the following commands:
```sh
./target/debug/mini-fs mountpoint container_file -n
./target/debug/mini-fs mountpoint container_file --allow-notification
```
However, be cautious as enabling this feature may result in frequent notifications, which could potentially become very annoying.

## Limitations and Optimization Opportunities
Mini-FS is not a fully-fledged filesystem and lacks several operations, including:

- Removing a directory.
- Changing file permissions (currently fixed at 777).
- Renaming files or directories.
- Names limited to 30 bytes.
- (Probably) not thread-safe.

Additionally, the container structure exhibits some inefficiencies that could be addressed for improved performance:

- Excessive read and write operations.
- Failure to release sectors when directories or files are truncated.
- Absence of a cache system.
- Fragmentation issues.

## License
Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
