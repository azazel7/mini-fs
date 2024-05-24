use crate::container::Container;
use anyhow::Result;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    Request,
};
use libc::{ENOENT, ENOSYS};
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1); // 1 second

const HELLO_DIR_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o777,
    nlink: 2,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

const HELLO_TXT_CONTENT: &str = "Hello World!\n";

const HELLO_TXT_ATTR: FileAttr = FileAttr {
    ino: 2,
    size: 13,
    blocks: 1,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::RegularFile,
    perm: 0o777,
    nlink: 1,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

pub struct FuseFs {
    container: Container,
}

impl FuseFs {
    pub fn new(container_name: String) -> Result<Self> {
        Ok(Self {
            container: Container::new(container_name)?,
        })
    }
}

impl Filesystem for FuseFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        eprintln!("lookup {parent} {:?}", name);
        let Ok(ret) = self.container.lookup(parent, name) else {
            reply.error(ENOENT);
            return;
        };
        if let Some((ino, filetype)) = ret {
            let attr: FileAttr = FileAttr {
                ino,
                size: 0,
                blocks: 1,
                atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: filetype,
                perm: 0o777,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };
            reply.entry(&TTL, &attr, 0);
        } else {
            reply.error(ENOENT);
        }
    }
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        eprintln!("getattr {ino}");
        let Ok(ret) = self.container.getattr(ino) else {
            reply.error(ENOENT);
            return;
        };
        if let Some(filetype) = ret {
            let attr: FileAttr = FileAttr {
                ino,
                size: 0,
                blocks: 1,
                atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: filetype,
                perm: 0o777,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };
            reply.attr(&TTL, &attr);
        } else {
            reply.error(ENOENT);
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        if ino == 2 {
            reply.data(&HELLO_TXT_CONTENT.as_bytes()[offset as usize..]);
        } else {
            eprintln!("read ino {}", ino);
            reply.error(ENOENT);
        }
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        let fd = self.container.opendir(ino);
        if let Ok(fd) = fd {
            reply.opened(fd, flags as u32)
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        eprintln!("Call to readdir inode:{ino} - {offset}");
        let ret = self.container.readdir(ino, _fh, offset);
        match ret {
            Err(err) => {
                eprintln!("{err}");
                reply.error(ENOENT);
            }
            Ok(entries) => {
                for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
                    // i + 1 means the index of the next entry
                    if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                        break;
                    }
                }
                reply.ok();
            }
        }
    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        umask: u32,
        flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        eprintln!("Call to create");
        let ret = self.container.create(parent, name);
        if let Ok(ino) = ret {
            let attr: FileAttr = FileAttr {
                ino,
                size: 0,
                blocks: 1,
                atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: FileType::RegularFile,
                perm: 0o777,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };

            reply.created(&TTL, &attr, 1, 1, flags as u32);
        } else {
            eprintln!("{:?}", ret);
            reply.error(ENOSYS);
        }
    }
}
