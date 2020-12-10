//! File system with directory support
//!
//! Create a filesystem that has a notion of blocks, inodes and directory inodes, by implementing the [`FileSysSupport`], the [`BlockSupport`], the [`InodeSupport`] and the [`DirectorySupport`] traits together (again, all earlier traits are supertraits of the later ones).
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
//! [`InodeSupport`]: ../../cplfs_api/fs/trait.InodeSupport.html
//! [`DirectorySupport`]: ../../cplfs_api/fs/trait.DirectorySupport.html
//! Make sure this file does not contain any unaddressed `TODO`s anymore when you hand it in.
//!
//! # Status
//!
//! indicate the status of this assignment. If you want to tell something
//! about this assignment to the grader, e.g., you have a bug you can't fix,
//! or you want to explain your approach, write it down after the comments
//! section. If you had no major issues and everything works, there is no need to write any comments.
//!
//! COMPLETED: YES
//!
//! COMMENTS:
//!

use anyhow::Result;
use cplfs_api::controller::Device;
use thiserror::Error;

use super::b_inode_support::InodeFileSystem;
use crate::b_inode_support::InodeLevelError;
use cplfs_api::error_given::APIError;
use cplfs_api::fs::{BlockSupport, DirectorySupport, FileSysSupport, InodeSupport};
use cplfs_api::types::{
    Block, DInode, DirEntry, FType, Inode, InodeLike, SuperBlock, DIRECT_POINTERS, DIRENTRY_SIZE,
    DIRNAME_SIZE,
};
use std::path::Path;

/// This error can occurs during manipulating with Directory File System
#[derive(Error, Debug)]
pub enum DirectoryLevelError {
    /// Error caused when performing controller operations.
    #[error("Inode error: {0}")]
    InodeError(#[from] InodeLevelError),
    /// Error caused when Directory operation is not valid.
    #[error("Invalid directory operation: {0}")]
    InvalidDirectoryOperation(&'static str),
    /// Error caused when performing controller operations.
    #[error("Controller error: {0}")]
    ControllerError(#[from] APIError),
    ///This error has mostly been added for illustrative purposes, and can be useful for quickly drafting some code without thinking about the concrete error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// `DirectoryFileSystem` wraps InodeFileSystem and add Directory support
pub struct DirectoryFileSystem {
    /// wrapped Inodes support layer
    inode_fs: InodeFileSystem,
}

/// `InodeFileSystem` struct implements `FileSysSupport` and the `BlockSupport`. Structure wraps `Device` to offer block-level abstraction to operate with File System.
/// Implementation of FileSysSupport in BlockFileSystem
impl FileSysSupport for DirectoryFileSystem {
    type Error = DirectoryLevelError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        return InodeFileSystem::sb_valid(sb);
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        let mut fs = Self {
            inode_fs: InodeFileSystem::mkfs(path, sb)?,
        };
        fs.i_put(&Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 1,
                size: 0,
                direct_blocks: [0; DIRECT_POINTERS as usize],
            },
        ))?;
        return Ok(fs);
    }

    fn mountfs(dev: Device) -> Result<Self, Self::Error> {
        let mut fs = Self {
            inode_fs: InodeFileSystem::mountfs(dev)?,
        };
        fs.i_put(&Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 1,
                size: 0,
                direct_blocks: [0; DIRECT_POINTERS as usize],
            },
        ))?;
        return Ok(fs);
    }

    fn unmountfs(self) -> Device {
        return self.inode_fs.unmountfs();
    }
}

/// Implementation of BlockSupport in BlockFileSystem
impl BlockSupport for DirectoryFileSystem {
    fn b_get(&self, i: u64) -> Result<Block, Self::Error> {
        return Ok(self.inode_fs.b_get(i)?);
    }

    fn b_put(&mut self, b: &Block) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.b_put(b)?);
    }

    fn b_free(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.b_free(i)?);
    }

    fn b_zero(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.b_zero(i)?);
    }

    fn b_alloc(&mut self) -> Result<u64, Self::Error> {
        return Ok(self.inode_fs.b_alloc()?);
    }

    fn sup_get(&self) -> Result<SuperBlock, Self::Error> {
        return Ok(self.inode_fs.sup_get()?);
    }

    fn sup_put(&mut self, sup: &SuperBlock) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.sup_put(sup)?);
    }
}

impl InodeSupport for DirectoryFileSystem {
    type Inode = Inode;

    fn i_get(&self, i: u64) -> Result<Self::Inode, Self::Error> {
        return Ok(self.inode_fs.i_get(i)?);
    }

    fn i_put(&mut self, ino: &Self::Inode) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.i_put(ino)?);
    }

    fn i_free(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.i_free(i)?);
    }

    fn i_alloc(&mut self, ft: FType) -> Result<u64, Self::Error> {
        return Ok(self.inode_fs.i_alloc(ft)?);
    }

    fn i_trunc(&mut self, inode: &mut Self::Inode) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.i_trunc(inode)?);
    }
}

impl DirectorySupport for DirectoryFileSystem {
    fn new_de(inum: u64, name: &str) -> Option<DirEntry> {
        let mut de = DirEntry::default();
        de.inum = inum;
        return match Self::set_name_str(&mut de, name) {
            Some(_) => Some(de), // name is correct
            None => None,
        };
    }

    fn get_name_str(de: &DirEntry) -> String {
        // find position of \0 and return collected string
        return match de.name.iter().position(|&x| x == '\0') {
            Some(p) => de.name[0..p].iter().collect(),
            None => de.name.iter().collect(),
        };
    }

    fn set_name_str(de: &mut DirEntry, name: &str) -> Option<()> {
        // check if is too long or empty string
        if name.len() > DIRNAME_SIZE || name.len() == 0 {
            return None;
        }
        // if name is . or .. then it is OK, then save and return
        if name == "." || name == ".." {
            for (i, ch) in name.chars().enumerate() {
                de.name[i] = ch;
            }
            de.name[name.len()] = '\0';
            return Some(());
        }
        // iterate over all chars and save to array
        for (i, ch) in name.chars().enumerate() {
            if !ch.is_alphanumeric() {
                return None;
            }
            de.name[i] = ch;
        }
        if name.len() < DIRNAME_SIZE {
            // if name is shorter then 14 put \0 on the end
            de.name[name.len() as usize] = '\0';
        }
        return Some(());
    }

    fn dirlookup(
        &self,
        inode: &Self::Inode,
        name: &str,
    ) -> Result<(Self::Inode, u64), Self::Error> {
        let sb = self.sup_get()?;
        if inode.get_ft() != FType::TDir {
            // error if inode is not Directory
            return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "To look up DirEntry inode must be Directory type.",
            ));
        }
        if inode.get_size() == 0 {
            // error if dir has no entries
            return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "Directory has no entries.",
            ));
        }

        // number of allocated data blocks
        let num_blocks = (inode.get_size() as f64 / sb.block_size as f64).ceil() as u64;

        // loop allocated blocks
        for b_i in 0..num_blocks {
            // wrap block to DirEntryBlock and look for DirEntry with name
            let de_block = DirEntryBlock::new(self.b_get(inode.get_block(b_i))?);
            match de_block.lookup_name(name) {
                Ok((de, offset)) => {
                    // DirEntry exists in current block
                    return Ok((self.i_get(de.inum)?, b_i * sb.block_size + offset));
                }
                _ => {}
            }
        }
        return Err(DirectoryLevelError::InvalidDirectoryOperation(
            "Directory has no entry with this name.",
        ));
    }

    fn dirlink(
        &mut self,
        inode: &mut Self::Inode,
        name: &str,
        inum: u64,
    ) -> Result<u64, Self::Error> {
        if inode.get_ft() != FType::TDir {
            // error if inode is not directory type
            return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "Inode must be Directory type to save DirEntry.",
            ));
        };

        if self.i_get(inum).unwrap().get_ft() == FType::TFree {
            // error if one inode to be linked is Free
            return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "Inode to by linked is not in use.",
            ));
        };

        match self.dirlookup(inode, name) {
            // error if DirEntry with name already exist
            Ok(_) => {
                return Err(DirectoryLevelError::InvalidDirectoryOperation(
                    "DirEntry with same name is already in directory",
                ))
            }
            _ => {}
        };
        let sb = self.sup_get()?;
        // number of allocated data blocks
        let num_blocks = (inode.get_size() as f64 / sb.block_size as f64).ceil() as u64;
        // if any allocated blocks then find free space for DirEntry
        if num_blocks > 0 {
            // loop allocated blocks
            for b_i in 0..num_blocks {
                let mut de_block = DirEntryBlock::new(self.b_get(inode.get_block(b_i))?);
                match de_block.find_free() {
                    Ok(o) => {
                        // free space for DirEntry was found in current block
                        // create DirEntry and save to block
                        match Self::new_de(inum, name) {
                            Some(de) => de_block.de_put(&de, o),
                            None => {
                                return Err(DirectoryLevelError::InvalidDirectoryOperation(
                                    "Unable to create DirEntry with given name and inum.",
                                ))
                            }
                        }?;
                        // find corresponding linked inode and update nlink
                        if inum != inode.get_inum() {
                            let mut inode_df = self.i_get(inum)?;
                            inode_df.disk_node.nlink += 1;
                            self.i_put(&inode_df).unwrap();
                        }
                        self.b_put(&de_block.return_block()).unwrap();
                        // update size of inode
                        if b_i * sb.block_size + o >= inode.get_size() {
                            inode.disk_node.size = inode.get_size() + *DIRENTRY_SIZE;
                        }
                        self.i_put(inode)?;
                        return Ok(b_i * sb.block_size + o);
                    }
                    _ => {}
                }
            }
        }

        // when no free space was found in allocated blocks, we have to add new one
        inode.disk_node.direct_blocks[num_blocks as usize] = sb.datastart + self.b_alloc()?;
        let mut de_block = DirEntryBlock::new(self.b_get(inode.get_block(num_blocks))?);
        match Self::new_de(inum, name) {
            // create DirEntry and save to block
            Some(de) => de_block.de_put(&de, 0),
            None => {
                return Err(DirectoryLevelError::InvalidDirectoryOperation(
                    "Unable to create DirEntry.",
                ))
            }
        }?;
        // find corresponding linked inode and update nlink
        if inum != inode.get_inum() {
            let mut inode_df = self.i_get(inum)?;
            inode_df.disk_node.nlink += 1;
            self.i_put(&inode_df)?;
        }
        self.b_put(&de_block.return_block()).unwrap();
        // update size of inode
        inode.disk_node.size += num_blocks * sb.block_size + *DIRENTRY_SIZE;
        self.i_put(inode)?;
        return Ok(num_blocks * sb.block_size);
    }
}

/// Wrapper for `Block` to execute `DirEntry` operations.
pub struct DirEntryBlock {
    /// block as boxed Block
    pub block: Box<Block>,
}

impl DirEntryBlock {
    /// Create new DirEntryBlock, having given `Block`.
    pub fn new(block: Block) -> DirEntryBlock {
        return DirEntryBlock {
            block: Box::from(block),
        };
    }
    /// Method to find free space inside of block buffer to save DirEntry
    /// It returns offset where DirEntry can be placed
    /// Error if there is no free space
    pub fn find_free(&self) -> Result<u64, DirectoryLevelError> {
        // remove left space from end of block buffer
        let space_for_entries = self.block.len() - (self.block.len() % *DIRENTRY_SIZE);
        for de_offset in (0..space_for_entries).step_by(*DIRENTRY_SIZE as usize) {
            let de = self.block.deserialize_from::<DirEntry>(de_offset)?;
            if de.inum == 0 {
                return Ok(de_offset);
            }
        }
        return Err(DirectoryLevelError::InvalidDirectoryOperation(
            "No free space for DirEntry.",
        ));
    }
    /// Method to find DirEntry with given name inside of block
    /// It returns found DirEntry with it's offset otherwise returns error
    pub fn lookup_name(&self, name: &str) -> Result<(DirEntry, u64), DirectoryLevelError> {
        // remove left space from end of block buffer
        let space_for_entries = self.block.len() - (self.block.len() % *DIRENTRY_SIZE);
        // loop over all DirEntries in block buffer
        for de_offset in (0..space_for_entries).step_by(*DIRENTRY_SIZE as usize) {
            let de = self.block.deserialize_from::<DirEntry>(de_offset)?;
            if DirectoryFileSystem::get_name_str(&de) == name {
                // Check if DirEntry has corresponding name
                return Ok((de, de_offset));
            }
        }
        return Err(DirectoryLevelError::InvalidDirectoryOperation(
            "No DirEntry found in block.",
        ));
    }
    /// Method to save given DirEntry to block buffer on position with given offset
    pub fn de_put(&mut self, de: &DirEntry, offset: u64) -> Result<(), DirectoryLevelError> {
        self.block.serialize_into(de, offset)?;
        return Ok(());
    }

    /// Method to free DirEntry with offset
    /// set the `inum` of this entry to 0 and the name of the entry to all zeroes, i.e. to `"0".repeat(DIRNAME_SIZE)`
    pub fn de_free(&mut self, offset: u64) -> Result<(), DirectoryLevelError> {
        let mut de = DirEntry::default();
        de.name = ['0'; DIRNAME_SIZE];
        de.inum = 0;
        self.block.serialize_into(&de, offset)?;
        return Ok(());
    }
    /// Return number of non free entries in the block
    pub fn entries_num(&self) -> Result<u64, DirectoryLevelError> {
        let mut counter: u64 = 0;
        let space_for_entries = self.block.len() - (self.block.len() % *DIRENTRY_SIZE);
        for de_offset in (0..space_for_entries).step_by(*DIRENTRY_SIZE as usize) {
            let de = self.block.deserialize_from::<DirEntry>(de_offset)?;
            if de.inum != 0
                && DirectoryFileSystem::get_name_str(&de) != "."
                && DirectoryFileSystem::get_name_str(&de) != ".."
            {
                counter += 1;
            }
        }
        return Ok(counter);
    }

    /// Return wrapped block back.
    pub fn return_block(self) -> Block {
        return *self.block;
    }
}

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
pub type FSName = DirectoryFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use crate::c_dirs_support::{DirEntryBlock, FSName};
    use cplfs_api::fs::{BlockSupport, DirectorySupport, FileSysSupport, InodeSupport};
    use cplfs_api::types::{
        Block, DInode, DirEntry, FType, Inode, InodeLike, SuperBlock, DIRECT_POINTERS,
        DIRENTRY_SIZE, DIRNAME_SIZE,
    };

    #[path = "utils.rs"]
    mod utils;

    static BLOCK_SIZE: u64 = 1000;
    static NBLOCKS: u64 = 10;
    static SUPERBLOCK_GOOD: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE,
        nblocks: NBLOCKS,
        ninodes: 6,
        inodestart: 1,
        ndatablocks: 5,
        bmapstart: 4,
        datastart: 5,
    };

    #[test]
    fn mkfs_dir_initialization_test() {
        let path = utils::disk_prep_path("mkfs_dir_initialization_test", "img");

        let fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let inode = fs.i_get(1).unwrap();

        assert_eq!(
            inode,
            Inode::new(
                1,
                DInode {
                    ft: FType::TDir,
                    nlink: 1,
                    size: 0,
                    direct_blocks: [0; DIRECT_POINTERS as usize],
                }
            )
        );
        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn set_get_name_str_test() {
        let ed = &mut DirEntry {
            inum: 0,
            name: ['c'; DIRNAME_SIZE],
        };
        // test bad inputs
        assert!(FSName::set_name_str(ed, "").is_none());
        assert!(FSName::set_name_str(ed, "LongStringToBeName").is_none());
        assert!(FSName::set_name_str(ed, ",").is_none());
        // test correct inputs
        assert_eq!(FSName::set_name_str(ed, ".").unwrap(), ());
        assert_eq!(FSName::set_name_str(ed, "..").unwrap(), ());
        assert_eq!(FSName::set_name_str(ed, "test").unwrap(), ());
        assert_eq!(FSName::get_name_str(ed), "test");
        // string with exact size DIRNAME_SIZE
        assert_eq!(FSName::set_name_str(ed, "testwithexacts").unwrap(), ());
        assert_eq!(FSName::get_name_str(ed), "testwithexacts");
    }

    #[test]
    fn new_de_test() {
        let path = utils::disk_prep_path("new_de_test", "img");

        let fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        let ed = FSName::new_de(1, "test").unwrap();

        assert_eq!(FSName::get_name_str(&ed), "test");

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn dir_entry_block_test() {
        let mut block = Block::new_zero(0, 50);
        block
            .serialize_into(&FSName::new_de(1, "test1").unwrap(), 0)
            .unwrap();
        block
            .serialize_into(&FSName::new_de(1, "test2").unwrap(), *DIRENTRY_SIZE)
            .unwrap();
        let de_block = DirEntryBlock::new(block);

        assert!(de_block.find_free().is_err());

        let mut block = Block::new_zero(0, 50);
        let de1 = FSName::new_de(1, "test1").unwrap();
        let de2 = FSName::new_de(2, "test2").unwrap();
        block.serialize_into(&de1, 0).unwrap();
        let mut de_block = DirEntryBlock::new(block);
        assert_eq!(de_block.find_free().unwrap(), *DIRENTRY_SIZE);
        assert_eq!(de_block.de_put(&de2, *DIRENTRY_SIZE).unwrap(), ());
        assert!(de_block.find_free().is_err());

        assert_eq!(de_block.lookup_name("test1").unwrap(), (de1, 0));
        assert_eq!(
            de_block.lookup_name("test2").unwrap(),
            (de2, *DIRENTRY_SIZE)
        );
        assert!(de_block.lookup_name("terer").is_err());
    }

    #[test]
    fn dirlink_test() {
        let path = utils::disk_prep_path("dirlink_test", "img");

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        // allocate 2,3,4 inodes
        let i_2 = fs.i_alloc(FType::TDir).unwrap();
        let i_3 = fs.i_alloc(FType::TDir).unwrap();
        let i_4 = fs.i_alloc(FType::TDir).unwrap();
        assert_eq!(i_2, 2);
        assert_eq!(i_3, 3);
        assert_eq!(i_4, 4);
        let mut inode = fs.i_get(i_2).unwrap();
        assert_eq!(
            inode,
            Inode::new(
                2,
                DInode {
                    ft: FType::TDir,
                    nlink: 0,
                    size: 0,
                    direct_blocks: [0; DIRECT_POINTERS as usize],
                }
            )
        );
        // link inode 3 to inode 2
        assert_eq!(fs.dirlink(&mut inode, "testa", 3).unwrap(), 0);
        assert_eq!(
            inode,
            Inode::new(
                2,
                DInode {
                    ft: FType::TDir,
                    nlink: 0,
                    size: *DIRENTRY_SIZE,
                    direct_blocks: [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                }
            )
        );
        assert_eq!(fs.dirlink(&mut inode, "testb", 4).unwrap(), *DIRENTRY_SIZE);
        fs.i_put(&inode).unwrap();
        assert_eq!(fs.i_get(inode.get_inum()).unwrap(), inode);
        assert_eq!(
            fs.dirlookup(&inode, "testa").unwrap(),
            (fs.i_get(i_3).unwrap(), 0)
        );
        assert_eq!(
            fs.dirlookup(&inode, "testb").unwrap(),
            (fs.i_get(i_4).unwrap(), *DIRENTRY_SIZE)
        );
        assert!(fs.dirlookup(&inode, "testunexist").is_err());

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn dirlink_with_allocated_blocks_test() {
        let path = utils::disk_prep_path("dirlink_with_allocated_blocks_test", "img");

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        for i in 0..5 {
            assert_eq!(fs.b_alloc().unwrap(), i);
        }
        // update root inode
        fs.i_put(&Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 1,
                size: (1.2 * SUPERBLOCK_GOOD.block_size as f64) as u64,
                direct_blocks: [5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        ))
        .unwrap();
        let mut root_inode = fs.i_get(1).unwrap();

        // allocate 2,3,4 inodes
        let i_2 = fs.i_alloc(FType::TDir).unwrap();
        let i_3 = fs.i_alloc(FType::TDir).unwrap();
        let i_4 = fs.i_alloc(FType::TDir).unwrap();
        assert_eq!(i_2, 2);
        assert_eq!(i_3, 3);
        assert_eq!(i_4, 4);
        // add direntries to root dir
        assert_eq!(fs.dirlink(&mut root_inode, "test2", 2).unwrap(), 0);
        assert_eq!(
            fs.dirlink(&mut root_inode, "..", 3).unwrap(),
            *DIRENTRY_SIZE
        );
        fs.i_put(&root_inode).unwrap();
        assert_eq!(
            fs.dirlookup(&root_inode, "test2").unwrap(),
            (fs.i_get(2).unwrap(), 0)
        );
        assert_eq!(
            fs.dirlookup(&root_inode, "..").unwrap(),
            (fs.i_get(3).unwrap(), *DIRENTRY_SIZE)
        );
        assert!(fs.dirlookup(&root_inode, "testunexist").is_err());

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }
}

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "c", feature = "all")))]
#[path = "../../api/fs-tests/c_test.rs"]
mod tests;
