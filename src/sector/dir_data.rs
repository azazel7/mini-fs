use heapless::Vec;
use serde::{Deserialize, Serialize};

use crate::sector::DirEntry;
use crate::sector::DIR_SECTOR_SIZE;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DirData {
    next_sector: Option<u64>,
    previous_sector: Option<u64>,
    files: Vec<DirEntry, DIR_SECTOR_SIZE>,
}
impl DirData {
    pub fn new() -> Self {
        let mut files = Vec::new();
        let _ = files.resize(DIR_SECTOR_SIZE, DirEntry::empty());
        Self {
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
    pub const fn next_sector(&self) -> Option<u64> {
        self.next_sector
    }
    pub const fn entries(&self) -> &Vec<DirEntry, DIR_SECTOR_SIZE> {
        &self.files
    }
    pub fn entries_mut(&mut self) -> &mut Vec<DirEntry, DIR_SECTOR_SIZE> {
        &mut self.files
    }
}
