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
use cplfs_api::types::{Block, FType, Inode, SuperBlock, DirEntry};
use std::path::Path;
use super::b_inode_support::InodeFileSystem;
use crate::b_inode_support::InodeLevelError;

/// This error can occurs during manipulating with Block File System
#[derive(Error, Debug)]
pub enum DirectoryLevelError {
    /// Error caused when performing controller operations.
    #[error("Inode error: {0}")]
    InodeError(#[from] InodeLevelError),
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
        return Ok(Self {
            inode_fs: InodeFileSystem::mkfs(path, sb)?,
        });
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
        unimplemented!()
    }

    fn get_name_str(de: &DirEntry) -> String {
        unimplemented!()
    }

    fn set_name_str(de: &mut DirEntry, name: &str) -> Option<()> {
        unimplemented!()
    }

    fn dirlookup(&self, inode: &Self::Inode, name: &str) -> Result<(Self::Inode, u64), Self::Error> {
        unimplemented!()
    }

    fn dirlink(&mut self, inode: &mut Self::Inode, name: &str, inum: u64) -> Result<u64, Self::Error> {
        unimplemented!()
    }
}


/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
/// **TODO**: replace the below type by the type of your file system
pub type FSName = ();

// **TODO** define your own tests here.

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "c", feature = "all")))]
#[path = "../../api/fs-tests/c_test.rs"]
mod tests;
