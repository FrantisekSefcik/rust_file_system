//! File system with inode support
//!
//! Create a filesystem that has a notion of inodes and blocks, by implementing the [`FileSysSupport`], the [`BlockSupport`] and the [`InodeSupport`] traits together (again, all earlier traits are supertraits of the later ones).
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
//! [`InodeSupport`]: ../../cplfs_api/fs/trait.InodeSupport.html
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

use anyhow::Result;
use cplfs_api::controller::Device;
use cplfs_api::error_given::APIError;
use thiserror::Error;

use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeSupport};
use cplfs_api::types::{
    Block, DInode, FType, Inode, InodeLike, SuperBlock, DINODE_SIZE, DIRECT_POINTERS,
};
use std::path::Path;
use super::{a_block_support::BlockFileSystem};
use crate::a_block_support::BlockLevelError;

/// This error can occurs during manipulating with Block File System
#[derive(Error, Debug)]
pub enum InodeLevelError {
    /// Error caused when `SuperBlock` to initialize `BlockFileSystem` is not valid.
    #[error("Invalid SuperBlock in BlockFileSystem initialization")]
    InvalidSuperBlock,
    /// Error caused when Inode operation is not valid.
    #[error("Invalid inode operation: {0}")]
    InvalidInodeOperation(&'static str),
    /// Error caused when performing controller operations.
    #[error("Controller error: {0}")]
    ControllerError(#[from] APIError),
    /// Error caused when performing controller operations.
    #[error("Block error: {0}")]
    BlockError(#[from] BlockLevelError),
    ///This error has mostly been added for illustrative purposes, and can be useful for quickly drafting some code without thinking about the concrete error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// `InodeFileSystem` struct implements `FileSysSupport` and the `BlockSupport`. Structure wraps `Device` to offer block-level abstraction to operate with File System.
pub struct InodeFileSystem {
    /// Wrapped device as a boxed Device.
    pub block_fs: BlockFileSystem
}

/// Implementation of FileSysSupport in BlockFileSystem
impl FileSysSupport for InodeFileSystem {
    type Error = InodeLevelError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        return BlockFileSystem::sb_valid(sb);
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        if !Self::sb_valid(&sb) {
            return Err(InodeLevelError::InvalidSuperBlock);
        } else {
            // write super block to first block
            let mut device = Device::new(path, sb.block_size, sb.nblocks)?;
            let sb_data = bincode::serialize(sb).unwrap();
            let mut sb_block = Block::new_zero(0, sb.block_size);
            sb_block.write_data(&sb_data, 0)?;
            device.write_block(&sb_block)?;

            // initialize inodes
            let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
            // + 1 inode because first is not in use
            let num_blocks = (sb.ninodes + 1) / num_inodes_in_block + 1;
            for b in 0..num_blocks {
                let mut i_block = Block::new_zero(sb.inodestart + b, sb.block_size);
                for i in 0..num_inodes_in_block {
                    let mut dinode = DInode::default();
                    dinode.ft = FType::TFree;
                    i_block.serialize_into(&dinode, i * *DINODE_SIZE)?;
                }
                device.write_block(&i_block)?;
            }

            // initialize bitmap
            device.write_block(&Block::new_zero(sb.bmapstart, sb.block_size))?;

            // set zeros to each data block
            for i in 0..sb.ndatablocks {
                device.write_block(&Block::new_zero(sb.datastart + i, sb.block_size))?;
            }

            return Ok(Self {
                block_fs: BlockFileSystem{ device: Box::from(device)},
            });
        }
    }

    fn mountfs(dev: Device) -> Result<Self, Self::Error> {
        return Ok(Self {
            block_fs: BlockFileSystem::mountfs(dev)?,
        });
    }

    fn unmountfs(self) -> Device {
        return self.block_fs.unmountfs();
    }
}

/// Implementation of BlockSupport in BlockFileSystem
impl BlockSupport for InodeFileSystem {
    fn b_get(&self, i: u64) -> Result<Block, Self::Error> {
        return Ok(self.block_fs.b_get(i)?);
    }

    fn b_put(&mut self, b: &Block) -> Result<(), Self::Error> {
        return Ok(self.block_fs.b_put(b)?);
    }

    fn b_free(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.block_fs.b_free(i)?);
    }

    fn b_zero(&mut self, i: u64) -> Result<(), Self::Error> {
        return Ok(self.block_fs.b_zero(i)?);
    }

    fn b_alloc(&mut self) -> Result<u64, Self::Error> {
        return Ok(self.block_fs.b_alloc()?);
    }

    fn sup_get(&self) -> Result<SuperBlock, Self::Error> {
        return Ok(self.block_fs.sup_get()?);
    }

    fn sup_put(&mut self, sup: &SuperBlock) -> Result<(), Self::Error> {
        return Ok(self.block_fs.sup_put(sup)?);
    }
}

impl InodeSupport for InodeFileSystem {
    type Inode = Inode;

    fn i_get(&self, i: u64) -> Result<Self::Inode, Self::Error> {
        let sb = self.sup_get()?;
        if i >= sb.ninodes {
            return Err(InodeLevelError::InvalidInodeOperation(
                "Given Inode index is higher then number of all inodes.",
            ));
        }
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        let block = self.b_get(sb.inodestart + (i / num_inodes_in_block))?;
        let dinode = block.deserialize_from::<DInode>((i % num_inodes_in_block) * *DINODE_SIZE)?;
        return Ok(Inode::new(i, dinode));
    }

    fn i_put(&mut self, ino: &Self::Inode) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        let mut block = self.b_get(sb.inodestart + ino.inum / num_inodes_in_block)?;
        block.serialize_into(
            &ino.disk_node,
            (ino.inum % num_inodes_in_block) * *DINODE_SIZE,
        )?;
        self.b_put(&block)?;
        return Ok(());
    }

    fn i_free(&mut self, i: u64) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        let inode = self.i_get(i)?; // get inode with index i
        if inode.get_nlink() > 0 {
            // if inode is still referenced
            return Err(InodeLevelError::InvalidInodeOperation(
               "Unable to free Inode because it is still referenced anywhere else in the file system."));
        }
        // iterate all blocks and free them
        for b in 0..(inode.get_size() as f64 / sb.block_size as f64).ceil() as u64 {
            self.b_free(inode.get_block(b) - sb.datastart)?;
        }
        // put free inode
        self.i_put(&Inode::new(i, DInode::default()))?;
        return Ok(());
    }

    fn i_alloc(&mut self, ft: FType) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        let mut b: u64 = sb.inodestart;
        let mut block = self.b_get(b)?;
        for i in 1..sb.ninodes {
            // get next block
            if i % num_inodes_in_block == 0 {
                b = b + 1;
                block = self.b_get(b)?;
            }
            let offset = (i % num_inodes_in_block) * *DINODE_SIZE;
            let inode = block.deserialize_from::<DInode>(offset).unwrap();
            if inode.ft == FType::TFree {
                block.serialize_into(
                    &DInode {
                        ft: ft,
                        nlink: 0,
                        size: 0,
                        direct_blocks: [0; DIRECT_POINTERS as usize],
                    },
                    offset,
                )?;
                self.b_put(&block)?;
                return Ok(i);
            }
        }
        return Err(InodeLevelError::InvalidInodeOperation(
            "No free inode to allocate",
        ));
    }

    fn i_trunc(&mut self, inode: &mut Self::Inode) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // iterate all blocks and free them
        for i in 0..(inode.get_size() as f64 / sb.block_size as f64).ceil() as u64 {
            self.b_free(inode.disk_node.direct_blocks[i as usize] - sb.datastart)?;
            inode.disk_node.direct_blocks[i as usize] = 0;
        }
        // put inode
        inode.disk_node.nlink = 0;
        inode.disk_node.size = 0;
        self.i_put(inode)?;
        return Ok(());
    }
}

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
pub type FSName = InodeFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeSupport};
    use cplfs_api::types::{DInode, FType, Inode, InodeLike, SuperBlock, DINODE_SIZE};

    use crate::b_inode_support::FSName;

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

    #[test]
    fn mkfs_inode_initializaiton_test() {
        let path = utils::disk_prep_path("mkfs_inode_initializaiton_test", "image_file");

        let fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();
        let inode_block = fs.b_get(SUPERBLOCK_GOOD.inodestart).unwrap();
        assert_eq!(
            inode_block.deserialize_from::<DInode>(0).unwrap().ft,
            FType::TFree
        );
        assert_eq!(
            inode_block
                .deserialize_from::<DInode>(*DINODE_SIZE)
                .unwrap()
                .ft,
            FType::TFree
        );
        // bad offset
        assert_ne!(
            inode_block.deserialize_from::<DInode>(8).unwrap().ft,
            FType::TFree
        );

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn get_put_inodes_test() {
        let path = utils::disk_prep_path("get_put_inodes_test", "image_file");
        //Some failing mkfs calls
        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        assert_eq!(fs.i_get(1).unwrap().get_ft(), FType::TFree);

        let inode = Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 13,
                size: 142,
                direct_blocks: [2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        );

        let inode2 = Inode::new(
            2,
            DInode {
                ft: FType::TDir,
                nlink: 10,
                size: 5,
                direct_blocks: [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        );

        fs.i_put(&inode).unwrap();
        fs.i_put(&inode2).unwrap();

        assert_eq!(fs.i_get(1).unwrap(), inode);
        assert_eq!(fs.i_get(2).unwrap(), inode2);

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn free_alloc_inodes_test() {
        let path = utils::disk_prep_path("free_alloc_inodes_test", "image_file");
        //Some failing mkfs calls
        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        assert_eq!(fs.i_get(0).unwrap().get_ft(), FType::TFree);

        let inode = Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 0,
                size: 2 * SUPERBLOCK_GOOD.block_size,
                direct_blocks: [5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        );

        let inode2 = Inode::new(
            2,
            DInode {
                ft: FType::TDir,
                nlink: 2,
                size: (1.2 * SUPERBLOCK_GOOD.block_size as f64) as u64,
                direct_blocks: [7, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        );
        // alloc first two blocks for first inode
        assert_eq!(fs.b_alloc().unwrap(), 0);
        assert_eq!(fs.b_alloc().unwrap(), 1);
        fs.i_put(&inode).unwrap();
        fs.i_put(&inode2).unwrap();
        fs.i_free(1).unwrap();
        // blocks should be free
        assert!(fs.b_free(0).is_err());
        assert!(fs.b_free(1).is_err());

        assert_eq!(fs.i_get(1).unwrap().get_ft(), FType::TFree);
        assert_eq!(fs.i_get(2).unwrap(), inode2);
        // test allocation of inode
        assert_eq!(fs.i_alloc(FType::TFile).unwrap(), 1);
        assert_eq!(fs.i_get(1).unwrap().get_ft(), FType::TFile);
        assert!(fs.i_free(1).is_ok());
        assert!(fs.i_free(2).is_err());

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }
    #[test]
    fn truncate_inodes_test() {
        let path = utils::disk_prep_path("free_inodes_test", "image_file");
        //Some failing mkfs calls
        let mut fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        for i in 0..3 {
            assert_eq!(fs.b_alloc().unwrap(), i);
        }

        let inode = Inode::new(
            1,
            DInode {
                ft: FType::TDir,
                nlink: 0,
                size: 2 * SUPERBLOCK_GOOD.block_size,
                direct_blocks: [5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        );

        let inode2 = Inode::new(
            2,
            DInode {
                ft: FType::TDir,
                nlink: 2,
                size: (1.2 * SUPERBLOCK_GOOD.block_size as f64) as u64,
                direct_blocks: [7, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        );
        // alloc first two blocks for first inode
        fs.i_put(&inode).unwrap();
        fs.i_put(&inode2).unwrap();
        fs.i_trunc(&mut fs.i_get(1).unwrap()).unwrap();
        // blocks should be free
        let inode_test = fs.i_get(1).unwrap();

        assert_eq!(inode_test.get_ft(), FType::TDir);
        assert_eq!(inode_test.get_size(), 0);
        assert_eq!(inode_test.get_nlink(), 0);
        assert_eq!(inode_test.disk_node.direct_blocks, [0; 12]);

        assert!(fs.i_trunc(&mut fs.i_get(2).unwrap()).is_err());

        let dev = fs.unmountfs();
        utils::disk_destruct(dev);
    }
}

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "b", feature = "all")))]
#[path = "../../api/fs-tests/b_test.rs"]
mod tests;
