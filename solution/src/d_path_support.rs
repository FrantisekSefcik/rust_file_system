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
//! indicate the status of this assignment. If you want to tell something
//! about this assignment to the grader, e.g., you have a bug you can't fix,
//! or you want to explain your approach, write it down after the comments
//! section. If you had no major issues and everything works, there is no need to write any comments.
//!
//! COMPLETED: YES
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
use cplfs_api::types::{Block, DirEntry, FType, Inode, InodeLike, SuperBlock};

use crate::b_inode_support::InodeLevelError;
use crate::c_dirs_support::{DirEntryBlock, DirectoryFileSystem, DirectoryLevelError};
use std::borrow::Borrow;

/// This error can occurs during manipulating with Path File System
#[derive(Error, Debug)]
pub enum PathsError {
    /// Error caused when performing inodes operations.
    #[error("Inode error: {0}")]
    InodeError(#[from] InodeLevelError),
    /// Error caused when performing directory operations.
    #[error("Directory error: {0}")]
    DirectoryError(#[from] DirectoryLevelError),
    /// Error caused when format of Path is invalid.
    #[error("Invalid path format: {0}")]
    InvalidPathFormat(&'static str),
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
    /// Current working directory (cwd) path represented by `String`
    cwd: String,
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
            cwd: "/".to_string(),
        };
        let mut root_inode = fs.i_get(1)?;
        fs.dirlink(&mut root_inode, ".", 1)?;
        fs.dirlink(&mut root_inode, "..", 1)?;
        fs.i_put(&root_inode)?;
        return Ok(fs);
    }

    fn mountfs(dev: Device) -> Result<Self, Self::Error> {
        let mut fs = Self {
            dir_fs: DirectoryFileSystem::mountfs(dev)?,
            cwd: "/".to_string(),
        };
        let mut root_inode = fs.i_get(1)?;
        fs.dirlink(&mut root_inode, ".", 1)?;
        fs.dirlink(&mut root_inode, "..", 1)?;
        fs.i_put(&root_inode)?;
        return Ok(fs);
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

    fn dirlookup(
        &self,
        inode: &Self::Inode,
        name: &str,
    ) -> Result<(Self::Inode, u64), Self::Error> {
        return Ok(self.dir_fs.dirlookup(inode, name)?);
    }

    fn dirlink(
        &mut self,
        inode: &mut Self::Inode,
        name: &str,
        inum: u64,
    ) -> Result<u64, Self::Error> {
        return Ok(self.dir_fs.dirlink(inode, name, inum)?);
    }
}

impl PathSupport for PathsFileSystem {
    fn valid_path(path: &str) -> bool {
        // Error if path is empty
        if path.len() == 0 {
            return false;
        }
        // Error if path ends with / and it is not only root path
        if path.len() > 1 && path.chars().last().unwrap() == '/' {
            return false;
        }
        // Error if path is not relative or absolute
        if path.chars().nth(0).unwrap() != '/' && path.chars().nth(0).unwrap() != '.' {
            return false;
        }
        // Get all names on path and loop over them
        let names = path.split('/').filter(|x| x.len() > 0);
        let mut de = DirEntry::default();
        for name in names {
            // check if all names are valid
            if Self::set_name_str(&mut de, name) == None {
                return false;
            }
        }
        return true;
    }

    fn get_cwd(&self) -> String {
        return self.cwd.clone();
    }

    fn set_cwd(&mut self, path: &str) -> Option<()> {
        match self.get_current_path(path) {
            Ok(cwd) => {
                self.cwd = cwd;
                Some(())
            }
            Err(_) => None,
        }
    }

    fn resolve_path(&self, path: &str) -> Result<Self::Inode, Self::Error> {
        // If path is relative then find inode of cwd otherwise return inode of root path
        let inode = if path.chars().nth(0).unwrap() == '.' {
            self.navigate(self.i_get(1)?, &self.get_cwd())?
        } else {
            self.i_get(1)?
        };
        // finally find inode on given path from starting inode
        return Ok(self.navigate(inode, path)?);
    }

    fn mkdir(&mut self, path: &str) -> Result<Self::Inode, Self::Error> {
        // Get name of directory to be created
        let name = Self::get_last_name(&path);
        // Error if directory name is "." or ".."
        if name == "." || name == ".." {
            return Err(Self::Error::InvalidPathFormat("Given path is invalid"));
        }
        // Resolve directory inode where new directory should be created
        let mut parent_inode = self.resolve_path(&Self::cd_parent(path.to_owned()))?;
        // In finall initialize new directory with new allocated inode
        let inum = self.i_alloc(FType::TDir)?;
        let mut inode = self.i_get(inum)?;
        self.dirlink(&mut parent_inode, &name, inum)?;
        self.dirlink(&mut inode, ".", inum)?;
        self.dirlink(&mut inode, "..", parent_inode.get_inum())?;
        parent_inode.disk_node.nlink += 1;
        inode.disk_node.nlink += 1;
        self.i_put(&parent_inode)?;
        self.i_put(&inode)?;

        return Ok(inode);
    }

    fn unlink(&mut self, path: &str) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // get name of directory
        let name = Self::get_last_name(path);
        // error if the path ends in "." or ".."; these entries cannot be removed
        if name == "." || name == ".." {
            return Err(Self::Error::InvalidPathFormat("Given path is invalid"));
        }
        // get Inode of parent directory where sub-directory should be deleted
        let parent_inode = self.resolve_path(&Self::cd_parent(path.to_owned()))?;
        // lookup for directory in parent directory, error if no directory with name found
        let (mut inode, de_offset) = self.dirlookup(&parent_inode, &name)?;
        // error if inode is type dir and is not empty
        if inode.get_ft() == FType::TDir && self.entries_num(&inode)? > 0 {
            return Err(Self::Error::InvalidPathOperation(
                "Directory to be unlinked is not empty",
            ));
        }
        // get block with DirEntry on offset
        let block = self.b_get(parent_inode.get_block(de_offset / sb.block_size))?;
        // wrap block to DirEntryBlock and free DirEntry with offset
        let mut dir_entry_block = DirEntryBlock::new(block);
        dir_entry_block.de_free(de_offset % sb.block_size)?;
        self.b_put(&dir_entry_block.return_block())?;
        // update inode nlink
        inode.disk_node.nlink -= 1;
        self.i_put(&inode)?;
        self.i_put(&parent_inode)?;
        // free inode if inode is without links
        if inode.get_nlink() == 0 {
            // decrease nlink in parent inode if any
            match self.dirlookup(&inode, "..") {
                Ok((mut inode_a, _)) => {
                    inode_a.disk_node.nlink -= 1;
                    self.i_put(&inode_a)?;
                }
                Err(_) => (),
            }
            self.i_free(inode.get_inum())?;
        }
        return Ok(());
    }
}

/// Helper functionality to work with paths
pub trait PathHelper: PathSupport {
    /// Returns cleaned absolute path with respect to current working directory if path is relative
    /// - return error if path is invalid
    /// - if a relative path is provided, then that path is interpreted with respect to the current working directory.
    /// - cancel out every "." (stay in same directory) and ".." (go to parent directory)
    fn get_current_path(&self, path: &str) -> Result<String, Self::Error>;

    /// Return last directory from path (same as `cd ..` command)
    /// - if path is "/" then return "/"
    fn cd_parent(path: String) -> String;

    /// Return Inode of last directory on the path
    fn navigate(&self, start_inode: Inode, path: &str) -> Result<Inode, Self::Error>;

    /// Add name to end of path (same as `cd name` command)
    fn cd_child(path: String, name: &str) -> String;

    /// Return name of directory from path
    fn get_last_name(path: &str) -> String;

    /// get number of entries in inode
    fn entries_num(&self, inode: &Inode) -> Result<u64, Self::Error>;
}


impl PathHelper for PathsFileSystem {
    fn get_current_path(&self, path: &str) -> Result<String, Self::Error> {
        if !Self::valid_path(path) {
            return Err(Self::Error::InvalidPathFormat("Given path is invalid"));
        }
        let mut cwd = self.get_cwd();
        if path.chars().nth(0).unwrap() == '/' {
            cwd = "/".to_owned();
        }
        let names = path.split('/').filter(|x| x.len() > 0);
        for name in names {
            cwd = match name {
                "." => cwd,
                ".." => Self::cd_parent(cwd),
                _ => Self::cd_child(cwd, name),
            }
        }
        return Ok(cwd);
    }

    fn cd_parent(path: String) -> String {
        if path == "/" || path == "." || path == ".." {
            return path;
        }
        let names = path
            .split('/')
            .filter(|x| x.len() > 0)
            .collect::<Vec<&str>>();
        let result = "/".to_string();
        return if path.chars().nth(0).unwrap() == '/' {
            result + &names[0..names.len() - 1].join("/")
        } else {
            names[0..names.len() - 1].join("/")
        };
    }

    fn navigate(&self, start_inode: Inode, path: &str) -> Result<Inode, Self::Error> {
        if !Self::valid_path(path) {
            return Err(Self::Error::InvalidPathFormat("Given path is invalid"));
        }
        // Collect all names on the path
        let names = path
            .split('/')
            .filter(|x| x.len() > 0)
            .collect::<Vec<&str>>();
        let mut inode = start_inode;
        // Loop over all names on path and check if DirEntry exist for particular name in directory
        for name in names {
            inode = self.dirlookup(inode.borrow(), name)?.0;
            if inode.get_ft() != FType::TDir {
                return Err(Self::Error::InvalidPathFormat(
                    "On given path is inode of not TDir type",
                ));
            }
        }
        return Ok(inode);
    }

    fn cd_child(path: String, name: &str) -> String {
        if path == "/" {
            return path + name;
        }
        return path + "/" + name;
    }

    fn get_last_name(path: &str) -> String {
        if path == "/" {
            return path.to_owned();
        }
        let names = path.split('/').collect::<Vec<&str>>();
        return names[names.len() - 1].to_owned();
    }

    fn entries_num(&self, inode: &Inode) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        let mut counter = 0;
        let num_blocks = (inode.get_size() as f64 / sb.block_size as f64).ceil() as u64;
        // if any allocated blocks then find free space for DirEntry
        if num_blocks > 0 {
            // loop allocated blocks
            for b_i in 0..num_blocks {
                let de_block = DirEntryBlock::new(self.b_get(inode.get_block(b_i))?);
                counter += de_block.entries_num()?;
            }
        }
        return Ok(counter);
    }
}

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
pub type FSName = PathsFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use crate::d_path_support::{FSName, PathHelper};
    use cplfs_api::fs::{DirectorySupport, FileSysSupport, InodeSupport, PathSupport};
    use cplfs_api::types::{DInode, FType, Inode, InodeLike, SuperBlock, DIRENTRY_SIZE};

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
    fn mkfs_test() {
        let path = utils::disk_prep_path("mkfs_test", "img");

        let fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let root_inode = fs.i_get(1).unwrap();

        assert_eq!(
            root_inode,
            Inode::new(
                1,
                DInode {
                    ft: FType::TDir,
                    nlink: 1,
                    size: 2 * *DIRENTRY_SIZE,
                    direct_blocks: [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                }
            )
        );

        assert_eq!(
            fs.dirlookup(&root_inode, ".").unwrap().0,
            fs.i_get(1).unwrap()
        );
        assert_eq!(
            fs.dirlookup(&root_inode, "..").unwrap().0,
            fs.i_get(1).unwrap()
        );

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn valid_path_test() {
        assert!(!FSName::valid_path(""));
        assert!(FSName::valid_path("/"));
        assert!(!FSName::valid_path("any/path/"));
        assert!(!FSName::valid_path("non"));
        assert!(FSName::valid_path("/non/.././ano"));
        assert!(FSName::valid_path("./non/.././ano"));
        assert!(FSName::valid_path("../non/.././ano"));
        assert!(!FSName::valid_path("../non/.././"));
        assert!(!FSName::valid_path("/@@/ano"));
    }

    #[test]
    fn path_helper_test() {
        assert_eq!(
            FSName::cd_parent("/ano/ano/../nie/ano".to_string()),
            "/ano/ano/../nie"
        );
        assert_eq!(FSName::cd_parent("./any".to_string()), ".");
        assert_eq!(
            FSName::cd_child("/ano/ano".to_string(), "nie"),
            "/ano/ano/nie"
        );
    }

    #[test]
    fn set_cwd_test() {
        let path = utils::disk_prep_path("set_cwd_test", "img");

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        fs.set_cwd("/ano/ano/nie");
        assert_eq!(fs.get_cwd(), "/ano/ano/nie");
        fs.set_cwd("../../dom");
        assert_eq!(fs.get_cwd(), "/ano/dom");
        fs.set_cwd("../../..");
        assert_eq!(fs.get_cwd(), "/");
        fs.set_cwd("./ano/nie");
        assert_eq!(fs.get_cwd(), "/ano/nie");
        fs.set_cwd("./../d/../r/../u");
        assert_eq!(fs.get_cwd(), "/ano/u");
        fs.set_cwd("/a/./b/../c");
        assert_eq!(fs.get_cwd(), "/a/c");

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn mkdir_test() {
        let path = utils::disk_prep_path("mkdir_test", "img");

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        fs.mkdir("/newdir").unwrap();

        let root_inode = fs.i_get(1).unwrap();

        assert_eq!(
            root_inode,
            Inode::new(
                1,
                DInode {
                    ft: FType::TDir,
                    nlink: 2,
                    size: 3 * *DIRENTRY_SIZE,
                    direct_blocks: [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                }
            )
        );
        assert_eq!(
            fs.dirlookup(&root_inode, "newdir").unwrap().0,
            fs.i_get(2).unwrap()
        );

        assert_eq!(fs.resolve_path("/newdir").unwrap(), fs.i_get(2).unwrap());

        fs.mkdir("./newdir/another").unwrap();

        assert_eq!(
            fs.resolve_path("/newdir/another").unwrap(),
            fs.i_get(3).unwrap()
        );
        fs.set_cwd("/newdir");
        assert_eq!(fs.resolve_path("./another").unwrap(), fs.i_get(3).unwrap());
        assert_eq!(
            fs.resolve_path("./../newdir/./another").unwrap(),
            fs.i_get(3).unwrap()
        );
        assert_eq!(
            fs.resolve_path("/../newdir/./../newdir/another").unwrap(),
            fs.i_get(3).unwrap()
        );
        assert!(fs.resolve_path("/../newdir/./../newdir/another/").is_err());

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn unlink_test() {
        let path = utils::disk_prep_path("unlink_test", "img");

        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        fs.mkdir("/dir").unwrap();

        let root_inode = fs.i_get(1).unwrap();

        assert_eq!(
            root_inode,
            Inode::new(
                1,
                DInode {
                    ft: FType::TDir,
                    nlink: 2,
                    size: 3 * *DIRENTRY_SIZE,
                    direct_blocks: [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                }
            )
        );
        assert_eq!(
            fs.dirlookup(&root_inode, "dir").unwrap().0,
            fs.i_get(2).unwrap()
        );

        fs.i_put(&root_inode).unwrap();

        assert_eq!(fs.resolve_path("/dir").unwrap(), fs.i_get(2).unwrap());

        fs.unlink("/dir").unwrap();
        assert!(fs.dirlookup(&root_inode, "dir").is_err());
        assert_eq!(fs.i_get(1).unwrap().get_nlink(), 1);
        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }
}

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "d", feature = "all")))]
#[path = "../../api/fs-tests/d_test.rs"]
mod tests;
