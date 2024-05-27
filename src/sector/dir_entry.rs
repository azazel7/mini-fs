use heapless::String;
use serde::{Deserialize, Serialize};

use crate::sector::FileType;
use crate::sector::FILE_NAME_SIZE;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirEntry {
    pub ino: u64,
    pub name: String<FILE_NAME_SIZE>,
    pub filetype: FileType,
    pub empty: bool,
}
impl DirEntry {
    pub const fn empty() -> Self {
        Self {
            ino: 0,
            name: String::new(),
            filetype: FileType::Regular,
            empty: true,
        }
    }
}
