use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Empty {
    previous: Option<u64>,
    next: Option<u64>,
}
impl Empty {
    pub fn set_previous(&mut self, sector_id: u64) {
        self.previous = Some(sector_id);
    }
    pub fn set_next(&mut self, sector_id: u64) {
        self.next = Some(sector_id);
    }
    pub const fn previous(&self) -> Option<u64> {
        self.previous
    }
    pub const fn next(&self) -> Option<u64> {
        self.next
    }
}
