use serde::{Deserialize, Serialize};

pub use self::dir_data::DirData;
pub use self::dir_entry::DirEntry;
pub use self::empty::Empty;
pub use self::file_data::FileData;
pub use self::file_metadata::FileMetadata;

mod dir_data;
mod dir_entry;
mod empty;
mod file_data;
mod file_metadata;

#[derive(Serialize, Deserialize, Debug)]
pub enum Sector {
    Empty(Empty),
    FileMetadata(FileMetadata),
    FileData(FileData),
    DirMetadata(FileMetadata),
    DirData(DirData),
}
pub const DATA_CHUNK_SIZE: usize = 200;
pub const FILE_NAME_SIZE: usize = 30;
pub const DIR_SECTOR_SIZE: usize = 5;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum FileType {
    Regular,
    Directory,
}
