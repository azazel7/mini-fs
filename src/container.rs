use anyhow::{bail, Ok, Result};
use fuser::{FileType, ReplyDirectory};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;
use std::str::FromStr;
use std::{fs::File, io::Write};

use crate::sector::{self, DirData, EmptySector, FileMetadata, Sector};

use sector::FILE_NAME_SIZE;

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
            let first_sector = Sector::DirMetadata(FileMetadata::new(1, None));

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
    fn get_empty_sector(&mut self) -> Result<u64> {
        if let Some(empty_sector_id) = self.metadata.first_empty_sector {
            let Sector::Empty(empty_sector_data) = self.read_sector(empty_sector_id)? else {
                bail!("Empty sector is not true empty sector");
            };
            if self.metadata.last_empty_sector == self.metadata.first_empty_sector {
                self.metadata.first_empty_sector = None;
                self.metadata.last_empty_sector = None;
            } else {
                self.metadata.first_empty_sector = empty_sector_data.next();
            }
            self.write_metadata()?;
            return Ok(empty_sector_id);
        }
        self.append_empty_sector()?;
        if let Some(empty_sector_id) = self.metadata.first_empty_sector {
            let Sector::Empty(empty_sector_data) = self.read_sector(empty_sector_id)? else {
                bail!("Empty sector is not true empty sector");
            };
            if self.metadata.last_empty_sector == self.metadata.first_empty_sector {
                self.metadata.first_empty_sector = None;
                self.metadata.last_empty_sector = None;
            } else {
                self.metadata.first_empty_sector = empty_sector_data.next();
            }
            self.write_metadata()?;
            return Ok(empty_sector_id);
        } else {
            bail!("No empty sector available");
        }
    }
    fn get_empty_entry(&mut self, dir_metadata: &FileMetadata) -> Result<Option<(u64, usize)>> {
        let mut next_sector = dir_metadata.first_sector();

        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = &base_sector else {
                bail!(
                    "Directory sector is not DirData (inode {}, sector {sector_id})",
                    dir_metadata.ino()
                );
            };
            //Look for an empty entry
            for (i, entry) in sector.entries().iter().enumerate() {
                if entry.empty {
                    //Write the entry
                    return Ok(Some((sector_id, i)));
                }
            }
            next_sector = sector.next_sector();
        }
        Ok(None)
    }
    fn find_ino_sector(&mut self, ino: u64) -> Result<(u64, Sector)> {
        for i in 0..self.metadata.sector_count {
            let sector = self.read_sector(i)?;
            if let Sector::DirMetadata(ref dir_metadata) = sector {
                if dir_metadata.ino() == ino {
                    return Ok((i, sector));
                }
            } else if let Sector::FileMetadata(ref file_metadata) = sector {
                if file_metadata.ino() == ino {
                    return Ok((i, sector));
                }
            }
        }
        bail!("Inode {ino} not found");
    }
    fn new_inode(&mut self) -> Result<u64> {
        if self.metadata.next_ino == u64::max_value() {
            bail!("No more inode");
        }
        self.metadata.next_ino += 1;
        Ok(self.metadata.next_ino - 1)
    }
    pub fn opendir(&mut self, ino: u64) -> Result<u64> {
        let (sector_id, sector) = self.find_ino_sector(ino)?;
        Ok(1)
    }
    pub fn readdir(
        &mut self,
        ino: u64,
        _fh: u64,
        offset: i64,
    ) -> Result<Vec<(u64, FileType, String)>> {
        let mut index = 0;

        // eprintln!("Look for ino {ino}");
        let (_sector_id, sector) = self.find_ino_sector(ino)?;
        let Sector::DirMetadata(dir_metadata) = sector else {
            bail!("Inode {ino} is not a directory.");
        };
        let mut next_sector = dir_metadata.first_sector();
        let mut entry_list = Vec::new();

        entry_list.push((ino, FileType::Directory, ".".to_string()));
        entry_list.push((ino, FileType::Directory, "..".to_string()));

        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = base_sector else {
                eprintln!("{base_sector:?}");
                bail!("Directory sector is not DirData (inode {ino}, sector {sector_id})");
            };
            for entry in sector.entries() {
                if entry.empty {
                    continue;
                }
                if index >= offset {
                    let filetype = match entry.filetype {
                        sector::FileType::Regular => FileType::RegularFile,
                        sector::FileType::Directory => FileType::Directory,
                    };
                    entry_list.push((entry.ino, filetype, entry.name.to_string()));
                }
                index += 1;
            }
            next_sector = sector.next_sector();
        }

        Ok(entry_list)
    }
    pub fn create(&mut self, parent: u64, name: &OsStr) -> Result<u64> {
        let Some(name) = name.to_str() else {
            bail!("Invalid name");
        };
        if name.len() >= FILE_NAME_SIZE {
            bail!("Name is too long (max {FILE_NAME_SIZE})");
        }
        let (metadata_sector_id, mut metadata_sector) = self.find_ino_sector(parent)?;
        let Sector::DirMetadata(dir_metadata) = &mut metadata_sector else {
            bail!("Inode {parent} is not a directory.");
        };
        let new_inode = self.new_inode()?;
        let empty_sector_id_file_metadata = self.get_empty_sector()?;
        //TODO check if name already exist
        let findings = self.get_empty_entry(dir_metadata)?;

        let (sector_id, idx) = if let Some(a) = findings {
            a
        } else {
            //append_empty_sector
            let empty_sector_id = self.get_empty_sector()?;
            eprintln!("Dir Data sector {empty_sector_id}");
            //set the sector data
            let mut sector = DirData::new();
            if let Some(previous_id) = dir_metadata.first_sector() {
                sector.set_previous(previous_id);
            }
            self.write_sector(empty_sector_id, &Sector::DirData(sector))?;
            //modify metadata to emplace it at the front of the sector list
            dir_metadata.set_first_sector(empty_sector_id);
            dir_metadata.increase_length_sector();
            self.write_sector(metadata_sector_id, &metadata_sector)?;
            (empty_sector_id, 0)
        };

        //Read the sector in memory
        let mut base_sector = self.read_sector(sector_id)?;
        let Sector::DirData(sector) = &mut base_sector else {
            bail!("Directory sector is not DirData (inode {parent}, sector {sector_id})");
        };

        //Should always be valid because it should have failed earlier otherwise (no new empty sector)
        let entry = sector.entries_mut().get_mut(idx).unwrap();

        //Write the entry
        entry.ino = new_inode;
        entry.empty = false;
        let std::result::Result::Ok(heapless_name) =
            heapless::String::<FILE_NAME_SIZE>::from_str(name)
        else {
            bail!("Error heapless::String::from_str for the filename.");
        };
        entry.name = heapless_name;
        entry.filetype = sector::FileType::Regular;

        eprintln!("base_sector {sector:?} - {}", sector.entries().len());
        //Write to container
        self.write_sector(sector_id, &base_sector)?;

        // write metadata of new file
        let empty_sector_id = empty_sector_id_file_metadata;
        let sector = FileMetadata::new(new_inode, Some(parent));
        self.write_sector(empty_sector_id, &Sector::FileMetadata(sector))?;
        eprintln!("File metadata sector {empty_sector_id}");

        Ok(new_inode)
    }
    pub fn getattr(&mut self, ino: u64) -> Result<Option<FileType>> {
        let (_sector_id, sector) = self.find_ino_sector(ino)?;
        if let Sector::DirMetadata(_) = sector {
            return Ok(Some(FileType::Directory));
        } else if let Sector::FileMetadata(_) = sector {
            return Ok(Some(FileType::RegularFile));
        }
        Ok(None)
    }
    pub fn lookup(&mut self, parent: u64, name: &OsStr) -> Result<Option<(u64, FileType)>> {
        let (_metadata_sector_id, mut metadata_sector) = self.find_ino_sector(parent)?;
        let Sector::DirMetadata(dir_metadata) = &mut metadata_sector else {
            bail!("Inode {parent} is not a directory.");
        };
        let mut next_sector = dir_metadata.first_sector();

        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = &base_sector else {
                bail!("Directory sector is not DirData (inode {parent}, sector {sector_id})");
            };
            //Look for used entry
            for entry in sector.entries() {
                if !entry.empty {
                    let ename = OsString::from(entry.name.to_string());
                    if ename == *name {
                        let filetype = match entry.filetype {
                            sector::FileType::Directory => FileType::Directory,
                            sector::FileType::Regular => FileType::RegularFile,
                        };
                        return Ok(Some((entry.ino, filetype)));
                    }
                }
            }
            next_sector = sector.next_sector();
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::remove_file;

    #[test]
    fn append_empty_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        let sector_count = container.metadata.sector_count;
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, sector_count + 1);
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
    #[test]
    fn get_empty_sector() {
        let _ = remove_file("/tmp/canard");
        let mut container = Container::new("/tmp/canard".to_string()).unwrap();
        container.append_empty_sector().unwrap();
        assert_eq!(container.metadata.sector_count, 2);
        let empty_sector = container.get_empty_sector().unwrap();
        assert_eq!(empty_sector, 1);
        remove_file("/tmp/canard").unwrap();
    }
}
