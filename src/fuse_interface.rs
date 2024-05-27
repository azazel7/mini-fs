use crate::container::Container;
use crate::logger::{EventType, Logger};
use crate::sector;
use anyhow::Result;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    Request,
};
use libc::{ENOENT, ENOSYS};
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1); // 1 second

pub struct FuseFs {
    container: Container,
    logger: Logger,
}

impl FuseFs {
    pub fn new(container_name: String, logger: Logger) -> Result<Self> {
        Ok(Self {
            container: Container::new(container_name)?,
            logger,
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
        let Some((ino, _filetype)) = ret else {
            reply.error(ENOENT);
            return;
        };
        let Ok(ret) = self.container.getattr(ino) else {
            reply.error(ENOENT);
            return;
        };
        if let Some(file_attr) = ret {
            let attr: FileAttr = FileAttr {
                ino : file_attr.ino,
                size: file_attr.size,
                blocks: 1,
                atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: file_attr.filetype,
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
        eprintln!("Getattr ino {ino}");
        let Ok(ret) = self.container.getattr(ino) else {
            reply.error(ENOENT);
            return;
        };
        if let Some(file_attr) = ret {
            let attr: FileAttr = FileAttr {
                ino,
                size: file_attr.size,
                blocks: 1,
                atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: file_attr.filetype,
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
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        eprintln!("Read ino {ino} (offset={offset}, size={size})");
        let mut data = Vec::new();
        let ret = self.container.read(ino, offset, size as u64, &mut data);
        if let Ok(_read) = ret {
            reply.data(&data);
        } else {
            reply.error(ENOENT);
        }
    }
    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: fuser::ReplyWrite,
    ) {
        eprintln!("Write ino {ino} (offset={offset}, data={data:?})");
        let result = self.container.write(ino, offset, data);
        if let Ok(written) = result {
            reply.written(written as u32);
        } else {
            reply.error(ENOENT);
        }
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        let fd = self.container.opendir(ino);
        if let Ok(fd) = fd {
            reply.opened(fd, flags as u32)
        } else {
            reply.error(ENOENT);
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
        _mode: u32,
        _umask: u32,
        flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        eprintln!("Create parent {parent} name={name:?}");
        self.logger.log(EventType::Open, &format!("{name:?}"));
        let ret = self
            .container
            .create(parent, name, sector::FileType::Regular);
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

            reply.created(&TTL, &attr, 1, 0, 0);
        } else {
            eprintln!("{:?}", ret);
            reply.error(ENOSYS);
        }
    }
    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        eprintln!("Open ino {ino}");
        self.logger.log(EventType::Open, &format!("{ino:?}"));
        reply.opened(0, 0);
    }
    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        eprintln!("Release ino {_ino}");
        self.logger.log(EventType::Close, &format!("{_ino:?}"));
        reply.ok();
    }
    fn flush(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, _lock_owner: u64, reply: ReplyEmpty) {
        self.logger.log(EventType::Close, &format!("{ino:?}"));
        reply.ok();
    }
    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let ret = self
            .container
            .create(parent, name, sector::FileType::Directory);
        if let Ok(ino) = ret {
            let attr: FileAttr = FileAttr {
                ino,
                size: 0,
                blocks: 1,
                atime: UNIX_EPOCH, // 1970-01-01 00:00:00
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: FileType::Directory,
                perm: 0o777,
                nlink: 1,
                uid: 501,
                gid: 20,
                rdev: 0,
                flags: 0,
                blksize: 512,
            };

            reply.entry(&TTL, &attr, 1);
        } else {
            eprintln!("{:?}", ret);
            reply.error(ENOSYS);
        }
    }
    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let ret = self.container.unlink(parent, name);
        if ret.is_ok() {
            reply.ok();
        } else {
            reply.error(ENOSYS);
        }
    }
}
