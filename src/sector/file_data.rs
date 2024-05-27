use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

use crate::sector::DATA_CHUNK_SIZE;

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
    pub const fn new() -> Self {
        Self {
            data_length: 0,
            next_sector: None,
            previous_sector: None,
            data: [0; DATA_CHUNK_SIZE],
        }
    }
    pub const fn next(&self) -> Option<u64> {
        self.next_sector
    }
    pub fn set_next(&mut self, next: u64) {
        self.next_sector = Some(next);
    }
    pub fn set_previous(&mut self, prev: u64) {
        self.previous_sector = Some(prev);
    }
    pub const fn data(&self) -> &[u8] {
        &self.data
    }
    pub const fn data_length(&self) -> u64 {
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
impl Default for FileData {
    fn default() -> Self {
        Self::new()
    }
}
