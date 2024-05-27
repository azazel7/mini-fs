use crate::container::Container;
use crate::logger::{EventType, Logger};
use crate::sector;
use anyhow::Result;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyLseek, Request, TimeOrNow,
};
use libc::{EIO, ENOENT, ENOSYS, O_APPEND, O_CREAT, O_EXCL, O_TRUNC};
use std::ffi::OsStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
                ino: file_attr.ino,
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
        let ret = self
            .container
            .create(parent, name, sector::FileType::Regular);
        if let Ok(ino) = ret {
            self.logger
                .log(EventType::Open, &format!("{name:?} (inode={ino:?})"));
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
            self.logger.log(EventType::Open, &format!("{name:?}"));
            reply.error(ENOSYS);
        }
    }
    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        eprintln!("Open ino {ino} flags={flags}");
        match flags & libc::O_ACCMODE {
            libc::O_RDONLY => {
                eprintln!("O_RDONLY");
                // Behavior is undefined, but most filesystems return EACCES
                if flags & libc::O_TRUNC != 0 {
                    eprintln!("O_TRUNC");
                    reply.error(libc::EACCES);
                    return;
                }
            }
            libc::O_WRONLY => eprintln!("O_WRONLY"),
            libc::O_RDWR => eprintln!("O_WRONLY"),
            // Exactly one access mode flag must be specified
            _ => {
                eprintln!("Other");
                reply.error(libc::EINVAL);
                return;
            }
        };

        if let Ok(name) = self.container.lookup_name(ino) {
            self.logger
                .log(EventType::Open, &format!("{name:?} (inode={ino:?})"));
        } else {
            self.logger.log(EventType::Open, &format!("{ino:?}"));
        }
        reply.opened(0, 0);
    }
    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        flags: u32,
        reply: ReplyEmpty,
    ) {
        eprintln!(
            "[Not Implemented] rename(parent: {:#x?}, name: {:?}, newparent: {:#x?}, \
            newname: {:?}, flags: {})",
            parent, name, newparent, newname, flags,
        );
        reply.error(ENOSYS);
    }
    fn mknod(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        umask: u32,
        rdev: u32,
        reply: ReplyEntry,
    ) {
        eprintln!(
            "[Not Implemented] mknod(parent: {:#x?}, name: {:?}, mode: {}, \
            umask: {:#x?}, rdev: {})",
            parent, name, mode, umask, rdev
        );
        reply.error(ENOSYS);
    }
    fn lseek(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        whence: i32,
        reply: ReplyLseek,
    ) {
        eprintln!(
            "[Not Implemented] lseek(ino: {:#x?}, fh: {}, offset: {}, whence: {})",
            ino, fh, offset, whence
        );
        reply.error(ENOSYS);
    }
    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        if let Some(size) = size {
            let ret = self.container.truncate(ino, size);
            if ret.is_err() {
                eprintln!("{:?}", ret);
                reply.error(EIO);
                return;
            }
        }
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
    fn release(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        eprintln!("Release ino {ino}");
        if let Ok(name) = self.container.lookup_name(ino) {
            self.logger
                .log(EventType::Close, &format!("{name:?} (inode={ino:?})"));
        } else {
            self.logger.log(EventType::Close, &format!("{ino:?}"));
        }
        reply.ok();
    }
    fn flush(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        _lock_owner: u64,
        reply: ReplyEmpty,
    ) {
        if let Ok(name) = self.container.lookup_name(ino) {
            self.logger
                .log(EventType::Close, &format!("{name:?} (inode={ino:?})"));
        } else {
            self.logger.log(EventType::Close, &format!("{ino:?}"));
        }
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
