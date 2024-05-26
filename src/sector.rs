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
pub const DATA_CHUNK_SIZE: usize = 200;
pub const FILE_NAME_SIZE: usize = 30;
pub const DIR_SECTOR_SIZE: usize = 5;

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
    pub fn ino(&self) -> u64 {
        self.ino
    }
    pub fn parent(&self) -> Option<u64> {
        self.parent
    }
    pub fn first_sector(&self) -> Option<u64> {
        self.first_sector
    }
    pub fn set_first_sector(&mut self, sector_id: u64) {
        self.first_sector = Some(sector_id);
    }
    pub fn increase_length_sector(&mut self) {
        self.length_sector += 1;
    }
    pub fn length_byte(&self) -> u64 {
        self.length_byte
    }
    pub fn increase_length_byte(&mut self, qty: u64) {
        self.length_byte += qty;
    }
}
#[serde_as]
#[derive(Serialize, Deserialize, Debug)]
pub struct FileData {
    data_length: u64,
    next_sector: Option<u64>,
    previous_sector: Option<u64>,
    #[serde_as(as = "Bytes")]
    data: [u8; DATA_CHUNK_SIZE],
}
impl FileData {
    pub fn new() -> Self {
        FileData {
            data_length: 0,
            next_sector: None,
            previous_sector: None,
            data: [0; DATA_CHUNK_SIZE],
        }
    }
    pub fn next(&self) -> Option<u64> {
        self.next_sector
    }
    pub fn set_next(&mut self, next: u64) {
        self.next_sector = Some(next);
    }
    pub fn set_previous(&mut self, prev: u64) {
        self.previous_sector = Some(prev);
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn data_length(&self) -> u64 {
        self.data_length
    }
    pub fn set_data_length(&mut self, data_length: u64) {
        self.data_length = data_length.min(DATA_CHUNK_SIZE as u64);
    }
    pub fn write(&mut self, data: &[u8], start: usize, end: usize) {
        let slice = &mut self.data[start..end];
        slice.clone_from_slice(data);
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum FileType {
    Regular,
    Directory,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirEntry {
    pub ino: u64,
    pub name: String<FILE_NAME_SIZE>,
    pub filetype: FileType,
    pub empty: bool,
}
impl DirEntry {
    pub fn empty() -> Self {
        Self {
            ino: 0,
            name: String::new(),
            filetype: FileType::Regular,
            empty: true,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DirData {
    next_sector: Option<u64>,
    previous_sector: Option<u64>,
    files: Vec<DirEntry, DIR_SECTOR_SIZE>,
}
impl DirData {
    pub fn new() -> Self {
        let mut files = Vec::new();
        let _ = files.resize(DIR_SECTOR_SIZE, DirEntry::empty());
        DirData {
            next_sector: None,
            previous_sector: None,
            files,
        }
    }
    pub fn set_previous(&mut self, previous: u64) {
        self.previous_sector = Some(previous);
    }
    pub fn set_next(&mut self, next: u64) {
        self.next_sector = Some(next);
    }
    pub fn next_sector(&self) -> Option<u64> {
        self.next_sector
    }
    pub fn entries(&self) -> &Vec<DirEntry, DIR_SECTOR_SIZE> {
        &self.files
    }
    pub fn entries_mut(&mut self) -> &mut Vec<DirEntry, DIR_SECTOR_SIZE> {
        &mut self.files
    }
}
