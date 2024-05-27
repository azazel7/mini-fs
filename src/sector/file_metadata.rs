use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct FileMetadata {
    ino: u64,
    parent: Option<u64>,
    length_byte: u64,
    length_sector: u64,
    first_sector: Option<u64>,
}
impl FileMetadata {
    pub const fn new(ino: u64, parent: Option<u64>) -> Self {
        Self {
            ino,
            parent,
            length_byte: 0,
            length_sector: 0,
            first_sector: None,
        }
    }
    pub const fn ino(&self) -> u64 {
        self.ino
    }
    pub const fn parent(&self) -> Option<u64> {
        self.parent
    }
    pub const fn first_sector(&self) -> Option<u64> {
        self.first_sector
    }
    pub fn set_first_sector(&mut self, sector_id: u64) {
        self.first_sector = Some(sector_id);
    }
    pub fn increase_length_sector(&mut self) {
        self.length_sector += 1;
    }
    pub const fn length_byte(&self) -> u64 {
        self.length_byte
    }
    pub fn increase_length_byte(&mut self, qty: u64) {
        self.length_byte += qty;
    }
    pub fn set_length_byte(&mut self, length_byte: u64) {
        self.length_byte = length_byte;
    }
}
