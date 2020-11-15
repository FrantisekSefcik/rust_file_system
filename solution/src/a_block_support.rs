//! File system with block support
//!
//! Create a filesystem that only has a notion of blocks, by implementing the [`FileSysSupport`] and the [`BlockSupport`] traits together (you have no other choice, as the first one is a supertrait of the second).
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
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

// Turn off the warnings we get from the below example imports, which are currently unused.
// TODO: this should be removed once you are done implementing this file. You can remove all of the below imports you do not need, as they are simply there to illustrate how you can import things.
#![allow(unused_imports)]

use std::env::VarError;
// We import std::error and std::format so we can say error::Error instead of
// std::error::Error, etc.
use anyhow::Result;
use std::error;
use std::error::Error;
use std::fmt;
use std::path::Path;
use thiserror::Error;

use cplfs_api::controller::Device;
use cplfs_api::error_given::APIError;
// If you want to import things from the API crate, do so as follows:
use cplfs_api::fs::BlockSupport;
use cplfs_api::fs::FileSysSupport;
use cplfs_api::types::{Block, Inode, SuperBlock};
use std::borrow::Borrow;

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out your file system name.
/// **TODO**: replace the below type by the type of your file system

/// This error can occurs during manipulating with File System
#[derive(Error, Debug)]
pub enum FSError {
    /// Error caused when checking SuperBlock validity.
    #[error("Issue in FileSystem initialization with SuperBlock")]
    InvalidSuperBlock(&'static str),
    /// Error caused when checking SuperBlock validity.
    #[error("Issue with block operation: {0}")]
    BlockOperationError(&'static str),
    /// Dsafdsafasd
    #[error("{0}")]
    FromAPIerror(#[from] APIError),
    ///This error has mostly been added for illustrative purposes, and can be useful for quickly drafting some code without thinking about the concrete error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

///my
pub struct FileSystem {
    device: Box<Device>,
}

impl FileSysSupport for FileSystem {
    type Error = FSError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        if sb.inodestart > sb.bmapstart || sb.bmapstart > sb.datastart {
            return false;
        }
        if sb.nblocks < (2 + sb.ninodes / 2 + sb.ndatablocks) {
            return false;
        }
        return true;
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        if !FileSystem::sb_valid(&sb) {
            return Err(FSError::InvalidSuperBlock(
                "Invalid Super Block to initialize File System",
            ));
        } else {
            let mut device = Device::new(path, sb.block_size, sb.nblocks)?;
            let sb_data = bincode::serialize(sb).unwrap();
            let mut sb_block = Block::new_zero(0, sb.block_size);
            sb_block.write_data(&sb_data, 0)?;
            device.write_block(&sb_block)?;
            device.write_block(&Block::new_zero(sb.bmapstart, sb.block_size))?;
            for i in 0..sb.ndatablocks {
                device.write_block(&Block::new_zero(sb.datastart + i, sb.block_size))?;
            }
            return Ok(Self {
                device: Box::from(device),
            });
        }
    }

    fn mountfs(dev: Device) -> Result<Self, Self::Error> {
        let sb_data = dev.read_block(0)?;
        let sb = sb_data.deserialize_from::<SuperBlock>(0).unwrap();
        if !FileSystem::sb_valid(&sb) {
            return Err(FSError::InvalidSuperBlock(
                "Invalid Super Block to initialize File System",
            ));
        } else {
            return Ok(Self {
                device: Box::from(dev),
            });
        }
    }

    fn unmountfs(self) -> Device {
        return *self.device;
    }
}

impl BlockSupport for FileSystem {
    fn b_get(&self, i: u64) -> Result<Block, Self::Error> {
        return Ok(self.device.read_block(i)?);
    }

    fn b_put(&mut self, b: &Block) -> Result<(), Self::Error> {
        self.device.write_block(b)?;
        return Ok(());
    }

    fn b_free(&mut self, i: u64) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;

        if i > sb.ndatablocks {
            return Err(FSError::BlockOperationError("Block to free out of index"));
        }
        let mut bmap_block = BitwiseBlock::new(self.b_get(sb.bmapstart).unwrap());
        if !bmap_block.get_bit(i) {
            return Err(FSError::BlockOperationError("Block is already free."));
        }
        bmap_block.put_bit(i, false);
        self.b_put(&bmap_block.return_block())?;
        return Ok(());
    }

    fn b_zero(&mut self, i: u64) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        if i > sb.ndatablocks {
            return Err(FSError::BlockOperationError(
                "Block index out of data block size.",
            ));
        }
        self.b_put(&Block::new_zero(sb.datastart + i, sb.block_size))?;
        return Ok(());
    }

    fn b_alloc(&mut self) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        let mut bmap_block = BitwiseBlock::new(self.b_get(sb.bmapstart).unwrap());
        let mask: u8 = 255;
        let num_of_bmap_bytes = sb.ndatablocks / 8 + 1;
        for i in 0..num_of_bmap_bytes {
            let byte = bmap_block.get_byte(i);
            if byte ^ mask > 0 {
                let til_bit = if i == num_of_bmap_bytes - 1 {
                    sb.ndatablocks % 8
                } else {
                    8
                };
                for x in 0..til_bit {
                    if !bmap_block.get_bit(i * 8 + x) {
                        bmap_block.put_bit(i * 8 + x, true);
                        self.b_put(&bmap_block.return_block())?;
                        self.b_zero(i * 8 + x)?;
                        return Ok(i * 8 + x);
                    }
                }
            }
        }
        return Err(FSError::BlockOperationError("No free block to allocate."));
    }

    fn sup_get(&self) -> Result<SuperBlock, Self::Error> {
        return Ok(self
            .device
            .read_block(0)?
            .deserialize_from::<SuperBlock>(0)?);
    }

    fn sup_put(&mut self, sup: &SuperBlock) -> Result<(), Self::Error> {
        let sb_data = bincode::serialize(sup).unwrap();
        let mut block = self.b_get(0).unwrap();
        block.write_data(&sb_data, 0)?;
        self.b_put(&block)?;
        return Ok(());
    }
}

struct BitwiseBlock {
    pub block: Box<Block>,
}

impl BitwiseBlock {
    pub fn new(block: Block) -> BitwiseBlock {
        return BitwiseBlock {
            block: Box::from(block),
        };
    }

    pub fn get_byte(&self, i: u64) -> u8 {
        let mut raw_data = vec![0; 1];
        self.block.read_data(&mut raw_data, i).unwrap();
        return *raw_data.get(0).unwrap();
    }

    pub fn put_byte(&mut self, i: u64, val: u8) {
        let mut raw_data = vec![val];
        self.block.write_data(&mut raw_data, i).unwrap();
    }

    pub fn get_bit(&self, i: u64) -> bool {
        let offset = i / 8;
        let bit_offset = i % 8;
        let byte = self.get_byte(offset);
        let mask: u8 = 1 << bit_offset;
        if (byte & mask) == 0 {
            return false;
        } else {
            return true;
        }
    }

    pub fn put_bit(&mut self, i: u64, val: bool) {
        if self.get_bit(i) == val {
            return;
        }
        let offset = i / 8;
        let bit_offset = i % 8;
        let byte = self.get_byte(offset);
        let mask: u8 = 1 << bit_offset;
        if val {
            self.put_byte(offset, byte | mask)
        } else {
            self.put_byte(offset, byte ^ mask)
        }
    }

    pub fn return_block(self) -> Block {
        return *self.block;
    }
}

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out your file system name.
pub type FSName = FileSystem;

// Here we define a submodule, called `my_tests`, that will contain your unit
// tests for this module.
// **TODO** define your own tests here. I have written down one test as an example of the syntax.
// You can define more tests in different modules, and change the name of this module
//
// The `test` in the `#[cfg(test)]` annotation ensures that this code is only compiled when we're testing the code.
// To run these tests, run the command `cargo test` in the `solution` directory
//
// To learn more about testing, check the Testing chapter of the Rust
// Book: https://doc.rust-lang.org/book/testing.html

// If you want to write more complicated tests that create actual files on your system, take a look at `utils.rs` in the assignment, and how it is used in the `fs_tests` folder to perform the tests. I have imported it below to show you how it can be used.
// The `utils` folder has a few other useful methods too (nothing too crazy though, you might want to write your own utility functions, or use a testing framework in rust, if you want more advanced features)
#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use std::path::{Path, PathBuf};

    use cplfs_api::controller::Device;
    use cplfs_api::fs::{BlockSupport, FileSysSupport};
    use cplfs_api::types::{Block, SuperBlock};

    use crate::a_block_support::{BitwiseBlock, FSName, FileSystem};
    use std::borrow::Borrow;

    #[path = "utils.rs"]
    mod utils;

    static BLOCK_SIZE: u64 = 1000;
    static NBLOCKS: u64 = 10;
    static SUPERBLOCK_GOOD: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE, //Note; assumes at least 2 inodes fit in one block. This should be the case for any reasonable inode implementation you might come up with
        nblocks: NBLOCKS,
        ninodes: 6,
        inodestart: 1,
        ndatablocks: 5,
        bmapstart: 4,
        datastart: 5,
    };

    static SUPERBLOCK_BAD_INODES: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE,
        nblocks: NBLOCKS,
        ninodes: 1000,
        inodestart: 1,
        ndatablocks: 5,
        bmapstart: 4,
        datastart: 5,
    };

    static SUPERBLOCK_BAD_ORDER: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE,
        nblocks: NBLOCKS,
        ninodes: 1000,
        inodestart: 1,
        ndatablocks: 5,
        bmapstart: 5,
        datastart: 6,
    };

    #[test]
    fn valid_super_block_test() {
        assert!(!FileSystem::sb_valid(&SUPERBLOCK_BAD_INODES));
        assert!(!FileSystem::sb_valid(&SUPERBLOCK_BAD_ORDER));
        assert!(FileSystem::sb_valid(&SUPERBLOCK_GOOD));
    }

    #[test]
    fn mkfs_test() {
        let path = utils::disk_prep_path("mkfs_test", "image_file");
        //Some failing mkfs calls
        assert!(FileSystem::mkfs(&path, &SUPERBLOCK_BAD_INODES).is_err());
        assert!(FileSystem::mkfs(&path, &SUPERBLOCK_BAD_ORDER).is_err());
        let fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn mount_fs_wrong_sb_test() {
        let path = utils::disk_prep_path("mount_fs_wrong_sb_test", "image_file");
        let mut device = utils::disk_setup(&path, BLOCK_SIZE, NBLOCKS);
        let sb_data = bincode::serialize(&SUPERBLOCK_BAD_INODES).unwrap();
        let mut sb = Block::new_zero(0, SUPERBLOCK_BAD_INODES.block_size);
        sb.write_data(&sb_data, 0).unwrap();
        device.write_block(&sb).unwrap();

        assert!(FileSystem::mountfs(device).is_err());

        utils::disk_unprep_path(&path);
    }

    #[test]
    fn mount_fs_good_sb_test() {
        let path = utils::disk_prep_path("mount_fs_good_sb_test", "image_file");
        let mut device = utils::disk_setup(&path, BLOCK_SIZE, NBLOCKS);
        let a = bincode::serialize(&SUPERBLOCK_GOOD).unwrap();
        let mut b = Block::new_zero(0, SUPERBLOCK_GOOD.block_size);
        b.write_data(&a, 0).unwrap();
        device.write_block(&b).unwrap();

        let fs = FileSystem::mountfs(device).unwrap();
        let sb = fs
            .device
            .read_block(0)
            .unwrap()
            .deserialize_from::<SuperBlock>(0)
            .unwrap();
        assert_eq!(sb, SUPERBLOCK_GOOD);

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn get_sup_block_test() {
        let path = utils::disk_prep_path("get_sup_block_test", "image_file");
        let fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        assert_eq!(fs.sup_get().unwrap(), SUPERBLOCK_GOOD);
        assert_ne!(fs.sup_get().unwrap(), SUPERBLOCK_BAD_INODES);

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn put_get_block_test() {
        let path = utils::disk_prep_path("put_get_block_test", "image_file");
        let mut fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        let block = Block::new_zero(SUPERBLOCK_GOOD.datastart, SUPERBLOCK_GOOD.block_size);
        fs.b_put(&block).unwrap();
        assert_eq!(fs.b_get(SUPERBLOCK_GOOD.datastart).unwrap(), block);

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn alloc_block_test() {
        let path = utils::disk_prep_path("alloc_block_test", "image_file");
        let mut fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        assert_eq!(fs.b_alloc().unwrap(), 0);
        assert_eq!(fs.b_alloc().unwrap(), 1);
        fs.b_free(0).unwrap();
        assert_eq!(fs.b_alloc().unwrap(), 0);
        let block = BitwiseBlock::new(fs.b_get(SUPERBLOCK_GOOD.bmapstart).unwrap());
        assert_eq!(format!("{:#010b}", block.get_byte(0)), "0b00000011");

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn no_more_free_block_to_alloc_test() {
        let path = utils::disk_prep_path("no_more_free_block_to_alloc_test", "image_file");
        let mut fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        for _ in 0..SUPERBLOCK_GOOD.ndatablocks {
            fs.b_alloc().unwrap();
        }
        assert!(fs.b_alloc().is_err());
        let block = BitwiseBlock::new(fs.b_get(SUPERBLOCK_GOOD.bmapstart).unwrap());
        assert_eq!(format!("{:#010b}", block.get_byte(0)), "0b00011111");

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn free_block_test() {
        let path = utils::disk_prep_path("free_block_test", "image_file");
        let mut fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        // index out of size
        assert!(fs.b_free(SUPERBLOCK_GOOD.ndatablocks + 2).is_err());
        // free already bree block
        assert!(fs.b_free(5).is_err());

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn sup_put_test() {
        let path = utils::disk_prep_path("sup_put_test", "image_file");
        let mut fs = FileSystem::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        // index out of size
        fs.sup_put(&SUPERBLOCK_BAD_INODES).unwrap();
        // free already bree block
        assert_eq!(fs.sup_get().unwrap(), SUPERBLOCK_BAD_INODES);

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn bitwise_block_test() {
        let block = Block::new_zero(0, 100);
        let mut bitwise_block = BitwiseBlock::new(block);

        assert_eq!(bitwise_block.get_bit(0), false);
        // set first and last bit in first byte
        bitwise_block.put_bit(0, true);
        assert_eq!(bitwise_block.get_bit(0), true);
        assert_eq!(format!("{:#010b}", bitwise_block.get_byte(0)), "0b00000001");
        bitwise_block.put_bit(7, true);
        assert_eq!(format!("{:#010b}", bitwise_block.get_byte(0)), "0b10000001");
        // (0 => 1) set first bit in second byte
        bitwise_block.put_bit(8, true);
        assert_eq!(format!("{:#010b}", bitwise_block.get_byte(1)), "0b00000001");
        // (1 => 1)
        bitwise_block.put_bit(8, true);
        assert_eq!(format!("{:#010b}", bitwise_block.get_byte(1)), "0b00000001");
        // (1 => 0) set first bit in second byte back to 0
        bitwise_block.put_bit(8, false);
        assert_eq!(format!("{:#010b}", bitwise_block.get_byte(1)), "0b00000000");
        // (0 => 0) try change 0 to 0
        bitwise_block.put_bit(8, false);
        assert_eq!(format!("{:#010b}", bitwise_block.get_byte(1)), "0b00000000");
        // returned block with correct modifications
        let mut raw_data = vec![0; 2];
        let b_block = bitwise_block.return_block();
        b_block.read_data(&mut raw_data, 0).unwrap();
        assert_eq!(format!("{:#010b}", raw_data.get(0).unwrap()), "0b10000001");
        assert_eq!(format!("{:#010b}", raw_data.get(1).unwrap()), "0b00000000");
    }

    #[test]
    fn unit_test() {
        //The below method set up the parent folder "a_parent_unique_name" within the root directory  of this solution crate
        //Also delete the file "image_file" within this folder if it already exists, so that it does not interfere with any later `mkfs` calls (this is useful if your previous test run failed, and the file did not get deleted)
        //*WARNING* !Make sure that this folder name "a_parent_unique_name" is actually unique over different tests, because tests are executed in parallel by default!
        //Returns the concatenated path, so that you can use the path further on, e.g. when creating a `Device` or `FileSystem`

        //! `let path = utils::disk_prep_path("a_parent_unique_name", "image_file");`

        //Things you want to test go here (check my tests in the API folder for examples)
        //! ...
        //! ...

        // If some disk actually created the file under `path` in your code, then you can uncomment the following call to clean it up:
        //!  `utils::disk_unprep_path(&path);`
        // This removes the image file and the parent directory at the end, so that no garbage is left in your file system
        //*WARNING* if a Device `dev` is still in scope for the path `path`, then the above call will block (the device holds a lock on the memory-mapped file)
        //You then have to use the following call instead:

        //! `utils::disk_destruct(dev);`

        //This makes the device go out of scope first, before tearing down the parent folder and image file, thereby avoiding deadlock
    }
}

// Here we define a submodule, called `tests`, that will contain our unit tests
// Take a look at the specified path to figure out which tests your code has to pass.
// As with all other files in the assignment, the testing module for this file is stored in the API crate (this is the reason for the 'path' attribute in the code below)
// The reason I set it up like this is that it allows me to easily add additional tests when grading your projects, without changing any of your files, but you can still run my tests together with yours by specifying the right features (see below) :)
// directory.
//
// To run these tests, run the command `cargo test --features="X"` in the `solution` directory, with "X" a space-separated string of the features you are interested in testing.
//
// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
//The below configuration tag specifies the following things:
// 'cfg' ensures this module is only included in the source if all conditions are met
// 'all' is true iff ALL conditions in the tuple hold
// 'test' is only true when running 'cargo test', not 'cargo build'
// 'any' is true iff SOME condition in the tuple holds
// 'feature = X' ensures that the code is only compiled when the cargo command includes the flag '--features "<some-features>"' and some features includes X.
// I declared the necessary features in Cargo.toml
// (Hint: this hacking using features is not idiomatic behavior, but it allows you to run your own tests without getting errors on mine, for parts that have not been implemented yet)
// The reason for this setup is that you can opt-in to tests, rather than getting errors at compilation time if you have not implemented something.
// The "a" feature will run these tests specifically, and the "all" feature will run all tests.
#[cfg(all(test, any(feature = "a", feature = "all")))]
#[path = "../../api/fs-tests/a_test.rs"]
mod tests;
