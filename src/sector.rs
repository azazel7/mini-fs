use heapless::{String, Vec};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

#[derive(Serialize, Deserialize, Debug)]
pub enum Sector {
    Empty(EmptySector),
    FileMetadata(FileMetadata),
    FileData(FileData),
    DirMetadata(FileMetadata),
    DirData(DirData),
}
const DATA_CHUNK_SIZE: usize = 200;
const FILE_NAME_SIZE: usize = 30;
const DIR_SECTOR_SIZE: usize = 5;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct EmptySector {
    previous: Option<u64>,
    next: Option<u64>,
}
impl EmptySector {
    pub fn set_previous(&mut self, sector_id: u64) {
        self.previous = Some(sector_id);
    }
    pub fn set_next(&mut self, sector_id: u64) {
        self.next = Some(sector_id);
    }
    pub fn previous(&self) -> Option<u64> {
        self.previous
    }
    pub fn next(&self) -> Option<u64> {
        self.next
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct FileMetadata {
    ino: u64,
    parent: Option<u64>,
    length_byte: u64,
    length_sector: u64,
    first_sector: Option<u64>,
}
impl FileMetadata {
    pub fn new(ino: u64, parent: Option<u64>) -> Self {
        Self {
            ino,
            parent,
            length_byte: 0,
            length_sector: 0,
            first_sector: None,
        }
    }
}
#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct FileData {
    length_data: u64,
    next_sector: u64,
    previous_sector: u64,
    #[serde_as(as = "Bytes")]
    data: [u8; DATA_CHUNK_SIZE],
}
impl FileData {
    pub fn new() -> Self {
        FileData {
            length_data: 0,
            next_sector: 0,
            previous_sector: 0,
            data: [0; DATA_CHUNK_SIZE],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum FileType {
    Regular,
    Directory,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DirEntry {
    ino: u64,
    name: String<FILE_NAME_SIZE>,
    filetype: FileType,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DirData {
    next_sector: u64,
    previous_sector: u64,
    files: Vec<DirEntry, DIR_SECTOR_SIZE>,
}
