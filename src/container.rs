use anyhow::{bail, Ok, Result};
use fuser::FileType;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::mem::size_of;
use std::path::Path;
use std::str::FromStr;
use std::usize;
use std::{fs::File, io::Write};

use crate::sector::{self, DirData, Empty, FileData, FileMetadata, Sector, DATA_CHUNK_SIZE};

use sector::FILE_NAME_SIZE;

pub struct Container {
    _container_name: String,
    file: File,
    metadata: Metadata,
}
#[derive(Debug)]
pub struct Attr {
    pub ino: u64,
    pub filetype: FileType,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    root_dir_sector: u64,
    sector_count: u64,
    first_empty_sector: Option<u64>,
    last_empty_sector: Option<u64>,
    next_ino: u64,
}

impl Container {
    pub fn new(container_name: String) -> Result<Self> {
        //check if file exist
        let (file, metadata) = if Path::new(&container_name).exists() {
            //Load an existing container
            let mut file = OpenOptions::new()
                .write(true)
                .read(true)
                .open(&container_name)?;
            let mut buff = [0; size_of::<Metadata>()];
            let read_count = file.read(&mut buff)?;
            if read_count < size_of::<Metadata>() {
                bail!("The file {container_name} is smaller than the container metadata.");
            }
            let metadata: Metadata = bincode::deserialize(&buff[..])?;
            (file, metadata)
        } else {
            //Initialize the container
            let mut file = File::create_new(&container_name)?;
            let metadata = Metadata {
                root_dir_sector: 0,
                sector_count: 1,
                first_empty_sector: None,
                last_empty_sector: None,
                next_ino: 2,
            };
            let first_sector = Sector::DirMetadata(FileMetadata::new(1, None));

            let mut buff = Vec::with_capacity(size_of::<Metadata>());
            bincode::serialize_into(&mut buff, &metadata)?;
            buff.resize(size_of::<Metadata>(), 0);
            file.write_all(&buff)?;

            let mut buff = Vec::with_capacity(size_of::<Sector>());
            bincode::serialize_into(&mut buff, &first_sector)?;
            buff.resize(size_of::<Sector>(), 0);
            file.write_all(&buff)?;

            (file, metadata)
        };
        Ok(Self {
            _container_name: container_name,
            file,
            metadata,
        })
    }
    fn read_sector(&mut self, sector_id: u64) -> Result<Sector> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<Metadata>() as u64 + sector_id * size_of::<Sector>() as u64;
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
        let mut buff = Vec::with_capacity(size_of::<Metadata>());
        bincode::serialize_into(&mut buff, &self.metadata)?;
        buff.resize(size_of::<Metadata>(), 0);
        self.file.write_all(&buff)?;
        Ok(())
    }
    fn write_sector(&mut self, sector_id: u64, sector: &Sector) -> Result<u64> {
        if sector_id >= self.metadata.sector_count {
            bail!("Seeking out-of-bound sector {sector_id}");
        }
        //Skip the metadata and seek
        let offset = size_of::<Metadata>() as u64 + sector_id * size_of::<Sector>() as u64;
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
        let mut empty_sector = Empty::default();
        //Set previous if any
        if let Some(last_sector) = self.metadata.last_empty_sector {
            empty_sector.set_previous(last_sector);
        }
        //Place the cursor
        let offset =
            size_of::<Metadata>() as u64 + self.metadata.sector_count * size_of::<Sector>() as u64; //TODO maybe add a -1
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
            Ok(empty_sector_id)
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
    fn free_sector(&mut self, sector_id: u64) -> Result<()> {
        let mut empty_sector = Empty::default();
        if let Sector::Empty(_) = self.read_sector(sector_id)? {
            //Sector is already empty
            return Ok(());
        }
        if let Some(first_empty_sector_id) = self.metadata.first_empty_sector {
            let mut base_first_empty_sector = self.read_sector(first_empty_sector_id)?;
            let Sector::Empty(first_empty_sector) = &mut base_first_empty_sector else {
                bail!("First empty sector ({first_empty_sector_id}) is not an empty sector.");
            };
            first_empty_sector.set_previous(sector_id);
            self.write_sector(first_empty_sector_id, &base_first_empty_sector)?;
            empty_sector.set_next(first_empty_sector_id);
        }
        self.write_sector(sector_id, &Sector::Empty(empty_sector))?;
        self.metadata.first_empty_sector = Some(sector_id);
        if self.metadata.last_empty_sector.is_none() {
            self.metadata.last_empty_sector = Some(sector_id);
        }
        self.write_metadata()?;
        Ok(())
    }
    fn delete_file(&mut self, ino: u64) -> Result<()> {
        let (metadata_sector_id, metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &metadata_sector else {
            bail!("Inode {ino} is not a file.");
        };
        let mut current_sector_id = file_metadata.first_sector();
        self.free_sector(metadata_sector_id)?;

        while let Some(sector_id) = current_sector_id {
            let Sector::FileData(file_data) = self.read_sector(sector_id)? else {
                bail!("Sector is not of type FileData.");
            };
            self.free_sector(sector_id)?;
            current_sector_id = file_data.next();
        }

        Ok(())
    }
    pub fn opendir(&mut self, ino: u64) -> Result<u64> {
        let (_sector_id, _sector) = self.find_ino_sector(ino)?;
        Ok(1)
    }
    pub fn readdir(
        &mut self,
        ino: u64,
        _fh: u64,
        offset: i64,
    ) -> Result<Vec<(u64, FileType, String)>> {
        let mut index = 0;

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
    pub fn create(&mut self, parent: u64, name: &OsStr, filetype: sector::FileType) -> Result<u64> {
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
        let Some(entry) = sector.entries_mut().get_mut(idx) else {
            bail!(
                "Error when accessing directory (inode={parent}) entry {idx}, sector={sector_id}"
            );
        };

        //Write the entry
        entry.ino = new_inode;
        entry.empty = false;
        let std::result::Result::Ok(heapless_name) =
            heapless::String::<FILE_NAME_SIZE>::from_str(name)
        else {
            bail!("Error heapless::String::from_str for the filename.");
        };
        entry.name = heapless_name;
        entry.filetype = filetype;

        //Write to container
        self.write_sector(sector_id, &base_sector)?;

        // write metadata of new file
        let empty_sector_id = empty_sector_id_file_metadata;
        let sector = FileMetadata::new(new_inode, Some(parent));
        let sector = match filetype {
            sector::FileType::Regular => Sector::FileMetadata(sector),
            sector::FileType::Directory => Sector::DirMetadata(sector),
        };
        self.write_sector(empty_sector_id, &sector)?;

        Ok(new_inode)
    }
    pub fn getattr(&mut self, ino: u64) -> Result<Option<Attr>> {
        let (_sector_id, sector) = self.find_ino_sector(ino)?;
        if let Sector::DirMetadata(_) = sector {
            let attr = Attr {
                ino,
                filetype: FileType::Directory,
                size: 0,
            };
            return Ok(Some(attr));
        } else if let Sector::FileMetadata(file_metadata) = sector {
            let attr = Attr {
                ino,
                filetype: FileType::RegularFile,
                size: file_metadata.length_byte(),
            };
            return Ok(Some(attr));
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
                    let entry_name = OsString::from(entry.name.to_string());
                    if entry_name == *name {
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
    pub fn unlink(&mut self, parent: u64, name: &OsStr) -> Result<()> {
        let (_metadata_sector_id, mut metadata_sector) = self.find_ino_sector(parent)?;
        let Sector::DirMetadata(dir_metadata) = &mut metadata_sector else {
            bail!("Inode {parent} is not a directory.");
        };
        let mut next_sector = dir_metadata.first_sector();

        let mut ino = None;
        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let mut base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = &mut base_sector else {
                bail!("Directory sector is not DirData (inode {parent}, sector {sector_id})");
            };
            //Look for entry with the right name
            for entry in sector.entries_mut() {
                if !entry.empty {
                    let entry_name = OsString::from(entry.name.to_string());
                    if entry_name == *name {
                        if entry.filetype == sector::FileType::Directory {
                            bail!("{name:?} is a directory.");
                        }
                        //Free entry and set ino
                        ino = Some(entry.ino);
                        entry.empty = true;
                        entry.ino = 0;
                        entry.name = heapless::String::new();
                        break;
                    }
                }
            }
            next_sector = sector.next_sector();
            if ino.is_some() {
                //The file has been found so we write it back
                self.write_sector(sector_id, &base_sector)?;
                break;
            }
        }
        //If the name has not been found, return Ok. No problem encountered
        let Some(ino) = ino else {
            return Ok(());
        };
        self.delete_file(ino)?;
        Ok(())
    }
    pub fn write(&mut self, ino: u64, offset: i64, data: &[u8]) -> Result<u64> {
        //TODO What is offset? The offset base on the beginning of a file or the hyphothetical
        //cursor?
        if offset < 0 {
            bail!("Writing at a negative offset (offset={offset})");
        }
        let offset = offset as u64;
        let (metadata_sector_id, mut metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &mut metadata_sector else {
            bail!("Inode {ino} is not a directory.");
        };
        let offset = if offset > file_metadata.length_byte() {
            file_metadata.length_byte()
        } else {
            offset
        };

        let mut current_sector_id = file_metadata.first_sector();
        let mut file_index = 0;
        let mut data_index = 0;
        let mut sector_index = 0;

        let mut previous_sector_id = None;

        let mut starting_sector = None;
        //find offset
        while let Some(sector_id) = current_sector_id {
            let mut sector = self.read_sector(sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            if offset >= file_index + DATA_CHUNK_SIZE as u64 {
                current_sector_id = sector_data.next();
                file_index += DATA_CHUNK_SIZE as u64;
                continue;
            }

            //Check if offset starts at this sector
            if offset >= file_index && offset < file_index + DATA_CHUNK_SIZE as u64 {
                starting_sector = Some(sector_id);
                sector_index = (offset - file_index) as usize;
                previous_sector_id = Some(sector_id);
                break;
            }
        }

        let mut current_sector_id = if let Some(a) = starting_sector {
            a
        } else {
            //Get an empty sector if needed
            let empty_sector_id = self.get_empty_sector()?;
            file_metadata.increase_length_sector();
            let mut next_sector = FileData::new();
            //Setup the sector
            if let Some(previous_sector_id) = previous_sector_id {
                next_sector.set_previous(previous_sector_id);
            } else {
                file_metadata.set_first_sector(empty_sector_id);
            }
            //Write it
            self.write_sector(empty_sector_id, &Sector::FileData(next_sector))?;
            empty_sector_id
        };
        let mut total_data_diff = 0;

        //Loop through the sectors to write the data
        loop {
            let mut sector = self.read_sector(current_sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {current_sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            //Remaining qty to write from data
            let data_qty = data.len() - data_index;
            //Maximum qty writable in that sector
            let write_qty = data_qty.min(DATA_CHUNK_SIZE - sector_index);

            //Write the data into the FileData struct
            let slice = &data[data_index..data_index + write_qty];
            sector_data.write(slice, sector_index, sector_index + write_qty);
            let prev_data_length = sector_data.data_length() as usize;

            let length_diff = if sector_index + write_qty < prev_data_length {
                0
            } else {
                sector_index + write_qty - prev_data_length
            };
            total_data_diff += length_diff;
            sector_data.set_data_length((prev_data_length + length_diff) as u64);

            //Update index
            data_index += write_qty;
            if data_index == data.len() {
                //We are done writing
                self.write_sector(current_sector_id, &sector)?;
                break;
            }
            sector_index = 0;
            file_index += DATA_CHUNK_SIZE as u64;

            //Append a new sector if needed
            let next_sector_id = if let Some(a) = sector_data.next() {
                a
            } else {
                let empty_sector_id = self.get_empty_sector()?;
                file_metadata.increase_length_sector();
                sector_data.set_next(empty_sector_id);
                let mut next_sector = FileData::new();
                next_sector.set_previous(current_sector_id);
                self.write_sector(empty_sector_id, &Sector::FileData(next_sector))?;
                empty_sector_id
            };
            self.write_sector(current_sector_id, &sector)?;
            current_sector_id = next_sector_id;
        }

        file_metadata.increase_length_byte(total_data_diff as u64);
        self.write_sector(metadata_sector_id, &metadata_sector)?;

        Ok(data.len().try_into()?)
    }
    pub fn read(&mut self, ino: u64, offset: i64, size: u64, data: &mut Vec<u8>) -> Result<u64> {
        //TODO What is offset? The offset base on the beginning of a file or the hyphothetical
        //cursor?
        if offset < 0 {
            bail!("Reading at a negative offset (offset={offset})");
        }
        let offset = offset as u64;
        let (_metadata_sector_id, mut metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &mut metadata_sector else {
            bail!("Inode {ino} is not a directory.");
        };
        if offset > file_metadata.length_byte() {
            return Ok(0);
        }

        let mut current_sector_id = file_metadata.first_sector();
        let mut file_index = 0;
        let mut sector_index = 0;

        let mut starting_sector = None;
        //find offset
        while let Some(sector_id) = current_sector_id {
            let mut sector = self.read_sector(sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            if offset >= file_index + DATA_CHUNK_SIZE as u64 {
                current_sector_id = sector_data.next();
                file_index += DATA_CHUNK_SIZE as u64;
                continue;
            }

            //Check if offset starts at this sector
            if offset >= file_index && offset < file_index + DATA_CHUNK_SIZE as u64 {
                starting_sector = Some(sector_id);
                sector_index = (offset - file_index) as usize;
                break;
            }
        }

        let Some(mut current_sector_id) = starting_sector else {
            bail!("Couldn't find the offset");
        };

        //Loop through the sectors to read the data
        loop {
            let mut sector = self.read_sector(current_sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {current_sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            //Remaining qty to read from data
            let data_qty = size as usize - data.len();
            //Maximum qty readable in that sector
            let read_qty = data_qty.min(sector_data.data_length() as usize - sector_index);

            //Read the data from FileData struct
            let slice = &sector_data.data()[sector_index..sector_index + read_qty];
            data.extend_from_slice(slice);

            //Update index
            if size == data.len() as u64 {
                //We are done reading
                break;
            }
            sector_index = 0;
            file_index += DATA_CHUNK_SIZE as u64;

            //Append a new sector if needed
            if let Some(next_sector_id) = sector_data.next() {
                current_sector_id = next_sector_id;
            } else {
                //EOF
                break;
            }
        }

        Ok(data.len().try_into()?)
    }
    pub fn lookup_name(&mut self, ino: u64) -> Result<OsString> {
        let (_sector_id, sector) = self.find_ino_sector(ino)?;
        let parent_ino = if let Sector::FileMetadata(file_metadata) = sector {
            file_metadata.parent()
        } else if let Sector::DirMetadata(dir_metadata) = sector {
            dir_metadata.parent()
        } else {
            bail!("Sector is not a metadata sector.");
        };
        let Some(parent_ino) = parent_ino else {
            //We are checking the root
            return Ok(OsString::from_str("/")?);
        };
        let (_sector_id, sector) = self.find_ino_sector(parent_ino)?;

        let Sector::DirMetadata(dir_metadata) = sector else {
            bail!("Parent of inode {ino} is not a directory");
        };

        let mut next_sector = dir_metadata.first_sector();
        //Iterate through all sector of directory
        while let Some(sector_id) = next_sector {
            let base_sector = self.read_sector(sector_id)?;
            let Sector::DirData(sector) = base_sector else {
                bail!("Directory sector is not DirData (inode {ino}, sector {sector_id})");
            };
            for entry in sector.entries() {
                if !entry.empty && entry.ino == ino {
                    let ename = OsString::from(entry.name.to_string());
                    return Ok(ename);
                }
            }
            next_sector = sector.next_sector();
        }
        bail!("Inode {ino} not found in parent directory");
    }
    pub fn truncate(&mut self, ino: u64, offset: u64) -> Result<()> {
        let (metadata_sector_id, mut metadata_sector) = self.find_ino_sector(ino)?;
        let Sector::FileMetadata(file_metadata) = &mut metadata_sector else {
            bail!("Inode {ino} is not a directory.");
        };
        match offset.cmp(&file_metadata.length_byte()) {
            Ordering::Greater => bail!(
                "Offset is too large for truncating (offset={offset}, file size={}.",
                file_metadata.length_byte()
            ),
            Ordering::Equal => return Ok(()),
            Ordering::Less => {}
        }
        file_metadata.set_length_byte(offset);
        let mut current_sector_id = file_metadata.first_sector();
        self.write_sector(metadata_sector_id, &metadata_sector)?;
        let mut file_index = 0;

        //find offset
        while let Some(sector_id) = current_sector_id {
            let mut sector = self.read_sector(sector_id)?;
            let Sector::FileData(sector_data) = &mut sector else {
                bail!("Sector {sector_id} (ino {ino} is not a FileData {sector:?}");
            };
            current_sector_id = sector_data.next();
            if offset >= file_index + DATA_CHUNK_SIZE as u64 {
                current_sector_id = sector_data.next();
                file_index += DATA_CHUNK_SIZE as u64;
                continue;
            }
            //Check if offset starts at this sector
            else if offset >= file_index && offset < file_index + DATA_CHUNK_SIZE as u64 {
                sector_data.set_data_length(offset - file_index);
                self.write_sector(sector_id, &sector)?;
            } else if offset < file_index {
                sector_data.set_data_length(0);
                self.write_sector(sector_id, &sector)?;
            }
        }
        Ok(())
    }
}

mod test;
