use anyhow::{bail, Ok, Result};
use bincode::Options;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;
use std::{fs::File, io::Write};

use crate::sector::{self, EmptySector, FileData, FileMetadata, Sector};

pub struct Container {
    container_name: String,
    file: File,
    metadata: ContainerMetadata,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContainerMetadata {
    root_dir_sector: u64,
    sector_count: u64,
    first_empty_sector: Option<u64>,
    last_empty_sector: Option<u64>,
    next_ino: u64,
}

impl Container {
    pub fn new(container_name: String) -> Result<Self> {
        //check if file exist
        let (file, metadata) = if !Path::new(&container_name).exists() {
            //Initialize the container
            let mut file = File::create_new(&container_name)?;
            let metadata = ContainerMetadata {
                root_dir_sector: 0,
                sector_count: 1,
                first_empty_sector: None,
                last_empty_sector: None,
                next_ino: 2,
            };
            let first_sector = Sector::FileMetadata(FileMetadata::new(1, None));

            let mut buff = Vec::with_capacity(size_of::<ContainerMetadata>());
            bincode::serialize_into(&mut buff, &metadata)?;
            buff.resize(size_of::<ContainerMetadata>(), 0);
            file.write_all(&buff)?;

            let mut buff = Vec::with_capacity(size_of::<Sector>());
            bincode::serialize_into(&mut buff, &first_sector)?;
            buff.resize(size_of::<Sector>(), 0);
            file.write_all(&buff)?;

            (file, metadata)
        } else {
            //Load an existing container
            let mut file = OpenOptions::new()
                .write(true)
                .read(true)
                .open(&container_name)?;
            let mut buff = [0; size_of::<ContainerMetadata>()];
            let read_count = file.read(&mut buff)?;
            if read_count < size_of::<ContainerMetadata>() {
                bail!("The file {container_name} is smaller than the container metadata.");
            }
            let metadata: ContainerMetadata = bincode::deserialize(&buff[..])?;
            eprintln!("Meta data {:?}", metadata);
            (file, metadata)
        };
        Ok(Self {
            container_name,
            file,
            metadata,
        })
    }
    fn read_sector(&mut self, sector_id: u64) -> Result<Sector> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<ContainerMetadata>() as u64 + sector_id * size_of::<Sector>() as u64;
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;

        //Read the sector
        let mut buff = [0; size_of::<Sector>()];
        let read_count = self.file.read(&mut buff)?;
        if read_count < size_of::<Sector>() {
            bail!("Reading not enough byte for sector {sector_id}.");
        }

        //Deserialize
        let sector: Sector = bincode::deserialize(&buff[..])?;
        Ok(sector)
    }
    fn write_metadata(&mut self) -> Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut buff = Vec::with_capacity(size_of::<ContainerMetadata>());
        bincode::serialize_into(&mut buff, &self.metadata)?;
        buff.resize(size_of::<ContainerMetadata>(), 0);
        self.file.write_all(&buff)?;
        Ok(())
    }
    fn write_sector(&mut self, sector_id: u64, sector: &Sector) -> Result<u64> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<ContainerMetadata>() as u64 + sector_id * size_of::<Sector>() as u64;
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;

        //Write the sector
        let mut buff = Vec::with_capacity(size_of::<Sector>());
        bincode::serialize_into(&mut buff, sector)?;
        buff.resize(size_of::<Sector>(), 0);
        self.file.write_all(&buff)?;
        self.file.flush()?;
        Ok(size_of::<Sector>() as u64)
    }
    fn append_empty_sector(&mut self) -> Result<u64> {
        let mut empty_sector = EmptySector::default();
        //Set previous if any
        if let Some(last_sector) = self.metadata.last_empty_sector {
            empty_sector.set_previous(last_sector);
        }
        //Place the cursor
        let offset = size_of::<ContainerMetadata>() as u64
            + self.metadata.sector_count * size_of::<Sector>() as u64; //TODO maybe add a -1
        let offset = SeekFrom::Start(offset);
        self.file.seek(offset)?;
        //Write the empty sector
        let mut buff = Vec::with_capacity(size_of::<Sector>());
        bincode::serialize_into(&mut buff, &Sector::Empty(empty_sector))?;
        buff.resize(size_of::<Sector>(), 0);
        self.file.write_all(&buff)?;
        self.file.flush()?;

        //Modify the previous last_empty_sector if any
        if let Some(last_empty_sector_id) = self.metadata.last_empty_sector {
            //Read and check for emptyness
            let Sector::Empty(mut last_empty_sector) = self.read_sector(last_empty_sector_id)?
            else {
                bail!("Last empty sector {last_empty_sector_id} is not empty.");
            };
            last_empty_sector.set_next(self.metadata.sector_count);
            self.write_sector(last_empty_sector_id, &Sector::Empty(last_empty_sector))?;
        }

        //If this one is the first empty sector update the list
        if self.metadata.first_empty_sector.is_none() {
            self.metadata.first_empty_sector = Some(self.metadata.sector_count);
        }
        self.metadata.last_empty_sector = Some(self.metadata.sector_count);
        self.metadata.sector_count += 1;
        self.write_metadata()?;
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::remove_file;
    use super::*;

    #[test]
    fn append_empty_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        let sector_count = container.metadata.sector_count;
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, sector_count+1);
        assert_eq!(container.metadata.last_empty_sector, Some(sector_count));
        remove_file("/tmp/canard").unwrap();
    }
    #[test]
    fn read_write_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, 4); //3 empty+the root = 4
        let sector = container.read_sector(1).unwrap();
        if let Sector::Empty(sector) = sector {
            assert_eq!(sector.previous(), None);
            assert_eq!(sector.next(), Some(2));
        }
        let sector = container.read_sector(2).unwrap();
        if let Sector::Empty(sector) = sector {
            assert_eq!(sector.previous(), Some(1));
            assert_eq!(sector.next(), Some(3));
        }
        let sector = container.read_sector(3).unwrap();
        if let Sector::Empty(sector) = sector {
            assert_eq!(sector.previous(), Some(2));
            assert_eq!(sector.next(), None);
        }
        remove_file("/tmp/canard").unwrap();
    }
}
