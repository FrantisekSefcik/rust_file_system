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
//! **TODO**: Replace the question mark below with YES, NO, or PARTIAL to
//! indicate the status of this assignment. If you want to tell something
//! about this assignment to the grader, e.g., you have a bug you can't fix,
//! or you want to explain your approach, write it down after the comments
//! section. If you had no major issues and everything works, there is no need to write any comments.
//!
//! COMPLETED: ?
//!
//! COMMENTS:
//!

use anyhow::Result;
use cplfs_api::controller::Device;
use thiserror::Error;

use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeSupport, DirectorySupport};
use cplfs_api::types::{Block, FType, Inode, SuperBlock, DirEntry, DInode, DIRECT_POINTERS, DIRENTRY_SIZE, DIRNAME_SIZE, InodeLike};
use std::path::Path;
use super::b_inode_support::InodeFileSystem;
use crate::b_inode_support::InodeLevelError;
use std::borrow::Borrow;
use cplfs_api::error_given::APIError;

/// This error can occurs during manipulating with Block File System
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

struct DirectoryFileSystem {
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
            }
        ))?;
        return Ok(fs);
    }

    fn mountfs(dev: Device) -> Result<Self, Self::Error> {
        return Ok(Self {
            inode_fs: InodeFileSystem::mountfs(dev)?,
        });
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
        let mut de =  DirEntry::default();
        de.inum = inum;
        return match Self::set_name_str(&mut de, name) {
            Some(_) => Some(de),
            None => None
        }
    }

    fn get_name_str(de: &DirEntry) -> String {
        // find position of \0 and return collected string
        return match  de.name.iter().position(|&x| x == '\0') {
            Some(p) => de.name[0 .. p].iter().collect(),
            None => de.name.iter().collect()
        }
    }

    fn set_name_str(de: &mut DirEntry, name: &str) -> Option<()> {
        // check if is too long or empty string
        if name.len() > DIRNAME_SIZE || name.len() == 0 {
            return None;
        }
        // if name is . or .. then it is OK, then save and return
        if name == "." || name == ".." {
            for (i,ch) in name.chars().enumerate() {
                de.name[i] = ch;
            }
            de.name[1] = '\0';
            return Some(());
        }
        // iterate over all chars and save to array
        for (i,ch) in name.chars().enumerate() {
            if !ch.is_alphabetic() { return None; }
            de.name[i] = ch;
        }
        if name.len() < DIRNAME_SIZE { // if name is shorter then 14 put \0 on the end
            de.name[name.len() as usize] = '\0';
        }
        return Some(());
    }

    fn dirlookup(&self, inode: &Self::Inode, name: &str) -> Result<(Self::Inode, u64), Self::Error> {
        if inode.get_nlink() == 0 {
            return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "Directory has no entries."))
        }
        let sb = self.sup_get()?;
        if !(&self.i_get(inode.get_inum())? == inode) {
            return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "Given inode is not same as one in memory."))
        }
        let mut offset = 0;
        let mut block_num = 0;
        let mut block = self.b_get(inode.get_block(block_num))?;
        for _ in 0 .. inode.get_nlink() {
            let mut de = block.deserialize_from::<DirEntry>(offset)?;
            if Self::get_name_str(&de) == name {
                return Ok((self.i_get(de.inum)?, block_num * sb.block_size + offset));
            }
            if offset + *DIRENTRY_SIZE * 2 > sb.block_size {
                block_num += 1;
                block = self.b_get(inode.get_block(block_num))?;
                offset = 0;
            } else {
                offset = offset + *DIRENTRY_SIZE;
            }
        }
        return Err(DirectoryLevelError::InvalidDirectoryOperation(
            "Directory has no entry with this name."))
    }

    fn dirlink(&mut self, inode: &mut Self::Inode, name: &str, inum: u64) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;

        let mut block_index = inode.get_size() / sb.block_size;
        let mut offset = inode.get_size() % sb.block_size;
        // check if new DirEntry fit into current block otherwise allocate new block
        if inode.get_size() % sb.block_size + *DIRENTRY_SIZE > sb.block_size {
            inode.disk_node.direct_blocks[(block_index + 1) as usize] = self.b_alloc()?;
            block_index = block_index + 1;
            offset = 0;
        }
        let mut block = self.b_get(inode.get_block(block_index))?;
        match Self::new_de(inum, name) {
            Some(de) => block.serialize_into(&de, offset),
            None => return Err(DirectoryLevelError::InvalidDirectoryOperation(
                "Unable to create DirEntry."
            ))
        };
        let de_offset = block_index * sb.block_size + offset;
        inode.disk_node.nlink += 1;
        inode.disk_node.size = de_offset + *DIRENTRY_SIZE;
        self.b_put(&block);
        return Ok(de_offset);
    }
}


/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
/// **TODO**: replace the below type by the type of your file system
pub type FSName = DirectoryFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeSupport, DirectorySupport};
    use cplfs_api::types::{DInode, FType, Inode, InodeLike, SuperBlock, DINODE_SIZE, DIRECT_POINTERS, DirEntry, DIRNAME_SIZE};
    use crate::c_dirs_support::{FSName, DirectoryFileSystem};
    use std::borrow::Borrow;

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

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let inode = fs.i_get(1).unwrap();

        assert_eq!(inode, Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 1,
                size: 0,
                direct_blocks: [0; DIRECT_POINTERS as usize],
            }
        ));
    }

    #[test]
    fn set_get_name_str() {

        let ed = &mut DirEntry{inum: 0, name: ['c'; DIRNAME_SIZE]};
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
    fn new_de_name_str() {
        let path = utils::disk_prep_path("new_de_name_str", "img");

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        let ed = FSName::new_de(1, "test").unwrap();

        assert_eq!(FSName::get_name_str(&ed), "test");
    }

}

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "c", feature = "all")))]
#[path = "../../api/fs-tests/c_test.rs"]
mod tests;
