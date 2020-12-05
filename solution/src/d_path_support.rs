//! File system with path support
//!
//! Create a filesystem that has a notion of blocks, inodes, directory inodes and paths, by implementing the [`FileSysSupport`], the [`BlockSupport`], the [`InodeSupport`], the [`DirectorySupport`] and the [`PathSupport`] traits together (again, all earlier traits are supertraits of the later ones).
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
//! [`InodeSupport`]: ../../cplfs_api/fs/trait.InodeSupport.html
//! [`DirectorySupport`]: ../../cplfs_api/fs/trait.DirectorySupport.html
//! [`PathSupport`]: ../../cplfs_api/fs/trait.PathSupport.html
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
//! ...
//!

use std::path::Path;

use thiserror::Error;

use cplfs_api::controller::Device;
use cplfs_api::error_given::APIError;
use cplfs_api::fs::{BlockSupport, DirectorySupport, FileSysSupport, InodeSupport, PathSupport};
use cplfs_api::types::{Block, DInode, DIRECT_POINTERS, DirEntry, FType, Inode, SuperBlock};

use crate::b_inode_support::InodeLevelError;
use crate::c_dirs_support::{DirectoryFileSystem, DirectoryLevelError};

/// This error can occurs during manipulating with Path File System
#[derive(Error, Debug)]
pub enum PathsError {
    /// Error caused when performing inodes operations.
    #[error("Inode error: {0}")]
    InodeError(#[from] InodeLevelError),
    /// Error caused when performing directory operations.
    #[error("Directory error: {0}")]
    DirectoryError(#[from] DirectoryLevelError),
    /// Error caused when Paths operation is not valid.
    #[error("Invalid paths operation: {0}")]
    InvalidPathOperation(&'static str),
    /// Error caused when performing controller operations.
    #[error("Controller error: {0}")]
    ControllerError(#[from] APIError),
    ///This error has mostly been added for illustrative purposes, and can be useful for quickly drafting some code without thinking about the concrete error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// `DirectoryFileSystem` wraps InodeFileSystem and add Directory support
pub struct PathsFileSystem {
    /// wrapped Inodes support layer
    dir_fs: DirectoryFileSystem,
}

/// Implementation of FileSysSupport in PathsFileSystem
impl FileSysSupport for PathsFileSystem {
    type Error = PathsError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        return DirectoryFileSystem::sb_valid(sb);
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        let mut fs = Self {
            dir_fs: DirectoryFileSystem::mkfs(path, sb)?,
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
            dir_fs: DirectoryFileSystem::mountfs(dev)?,
        });
    }

    fn unmountfs(self) -> Device {
        return self.dir_fs.unmountfs();
    }
}

/// Implementation of BlockSupport in PathsFileSystem
impl BlockSupport for PathsFileSystem {
    fn b_get(&self, i: u64) -> Result<Block, Self::Error> {
        return Ok(self.dir_fs.b_get(i)?);
    }

    fn b_put(&mut self, b: &Block) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.b_put(b)?);
    }

    fn b_free(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.b_free(i)?);
    }

    fn b_zero(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.b_zero(i)?);
    }

    fn b_alloc(&mut self) -> Result<u64, Self::Error> {
        return Ok(self.dir_fs.b_alloc()?);
    }

    fn sup_get(&self) -> Result<SuperBlock, Self::Error> {
        return Ok(self.dir_fs.sup_get()?);
    }

    fn sup_put(&mut self, sup: &SuperBlock) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.sup_put(sup)?);
    }
}

/// Implementation of InodeSupport in PathsFileSystem
impl InodeSupport for PathsFileSystem {
    type Inode = Inode;

    fn i_get(&self, i: u64) -> Result<Self::Inode, Self::Error> {
        return Ok(self.dir_fs.i_get(i)?);
    }

    fn i_put(&mut self, ino: &Self::Inode) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.i_put(ino)?);
    }

    fn i_free(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.i_free(i)?);
    }

    fn i_alloc(&mut self, ft: FType) -> Result<u64, Self::Error> {
        return Ok(self.dir_fs.i_alloc(ft)?);
    }

    fn i_trunc(&mut self, inode: &mut Self::Inode) -> Result<(), Self::Error> {
        return Ok(self.dir_fs.i_trunc(inode)?);
    }
}

/// Implementation of DirectorySupport in PathsFileSystem
impl DirectorySupport for PathsFileSystem {
    fn new_de(inum: u64, name: &str) -> Option<DirEntry> {
        return DirectoryFileSystem::new_de(inum, name);
    }

    fn get_name_str(de: &DirEntry) -> String {
        return DirectoryFileSystem::get_name_str(de);
    }

    fn set_name_str(de: &mut DirEntry, name: &str) -> Option<()> {
        return DirectoryFileSystem::set_name_str(de, name);
    }

    fn dirlookup(&self, inode: &Self::Inode, name: &str) -> Result<(Self::Inode, u64), Self::Error> {
        return Ok(self.dir_fs.dirlookup(inode, name)?);
    }

    fn dirlink(&mut self, inode: &mut Self::Inode, name: &str, inum: u64) -> Result<u64, Self::Error> {
        return Ok(self.dir_fs.dirlink(inode, name, inum)?);
    }
}

impl PathSupport for PathsFileSystem {
    fn valid_path(path: &str) -> bool {
        unimplemented!()
    }

    fn get_cwd(&self) -> String {
        unimplemented!()
    }

    fn set_cwd(&mut self, path: &str) -> Option<()> {
        unimplemented!()
    }

    fn resolve_path(&self, path: &str) -> Result<Self::Inode, Self::Error> {
        unimplemented!()
    }

    fn mkdir(&mut self, path: &str) -> Result<Self::Inode, Self::Error> {
        unimplemented!()
    }

    fn unlink(&mut self, path: &str) -> Result<(), Self::Error> {
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
#[cfg(all(test, any(feature = "d", feature = "all")))]
#[path = "../../api/fs-tests/d_test.rs"]
mod tests;
