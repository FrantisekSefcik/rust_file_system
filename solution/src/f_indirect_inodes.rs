//! File system with support for inodes that have indirect blocks too.
//! Reimplementation of the inodes from the base project.
//! This additional assignment requires completion of assignment e, so go and do that one first if you haven't yet.
//!
//! Create a filesystem that has a notion of inodes and blocks, by implementing the [`FileSysSupport`], the [`BlockSupport`], the [`InodeSupport`] and the [`InodeRWSupport`] traits together (again, all earlier traits are supertraits of the later ones).
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
//! [`InodeSupport`]: ../../cplfs_api/fs/trait.InodeSupport.html
//! [`InodeRWSupport`]: ../../cplfs_api/fs/trait.InodeRWSupport.html
//!
//! However, this time, you **cannot** use the `DInode` type I provided, but rather, you have to define your own type,  have it derive all the traits you need (check the `#[derive(...)]` for `DInode` in the provided API code), wrap it in your own `Inode` type, and have your own `Inode` type implement the `InodeLike` trait, so that it is still compatible with the tests.
//! So far, we have only supported inodes that have at most a fixed, direct number of data blocks (i.e. `DIRECT_POINTERS` data blocks) associated to them.
//! Reimplement your solution for the base project, but now support inodes that have an extra field (let's call this field the *indirect block* field), that points to a single data block; the so-called *indirect block*.
//! As long as your inode requires `DIRECT_POINTERS` data blocks or fewer, all code behaves as before and the indirect block field is set to 0.
//! As soon as block number `DIRECT_POINTERS+1` has to be allocated, the indirect block gets allocated along with it, to store a sequence of block numbers.
//! The address of block `DIRECT_POINTERS+1` gets stored as the first address in the indirect block.
//! Later, when data block `DIRECT_POINTERS+2` has to be allocated, it gets stored in the second slot on the indirect block, and so on. In other words, given that inode numbers are of type `u64` and should in principle be represented by 8 bytes at runtime, this indirect block allows files to allocate another `block_size/8` blocks.
//!
//! Some more specific pointers for the implementation:
//! - Try to come up with some helper functions that make it more convenient to work with indirect blocks
//! - Think of the scenario's where an indirect block might get (de)allocated. See if you can wrap this allocation in one of these previously mentioned helper functions.
//! - Since the `new` method in `InodeLike` is static and does not allow you to allocate new blocks, it should still not allow you to provide more than `DIRECT_POINTERS+1` blocks. The last block is then the block number of the indirect block. Similarly, the `get_block` method should simply return the number of the indirect block when queried for block *index* `DIRECT_POINTERS`, since there is no way for inodes to read from the device to figure out the actual block number.
//! - Do not forget to deallocate the indirect block itself, when truncating or freeing an inode.
//!
//! It should be possible to swap out the inodes you use in your filesystem so far without making any of the tests for previous assignments fail.
//! You could do this (rather than copying all of your code and starting over) if you want some extra assurance that your implementation is still correct (or at least, still correct when not indexing inodes past the `DIRECT_POINTERS`th block)
//! At the end, write some tests that convincingly show that your implementation indeed supports indirect pointers.
//!
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
use std::path::Path;

use thiserror::Error;

use cplfs_api::controller::Device;
use cplfs_api::error_given::APIError;
use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeRWSupport, InodeSupport};
use cplfs_api::types::{Block, Buffer, FType, InodeLike, SuperBlock, DIRECT_POINTERS};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use crate::a_block_support::{BlockFileSystem, BlockLevelError};

/// Copy of original DInode but new field `indirect_block` is added
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
pub struct DInode {
    /// Registers the file type
    pub ft: FType,
    /// Counts the number of links to this inode in the file system. The point of doing this is that if the inode is written back to disk when it has no links to it, it should be freed instead, thereby setting its file type to `T_FREE`.
    pub nlink: u16,
    /// Size of the file in bytes. Used to see when a read or write would go out of file bounds.
    pub size: u64,
    /// A list of up to `DIRECT_POINTERS` valid block addresses (counting from 0, *not* from the start of the data block region), to specify where the contents of this file are stored.
    pub direct_blocks: [u64; DIRECT_POINTERS as usize],
    /// Index of block where are stored indirect block indices
    pub indirect_block: u64,
}

lazy_static! {
    /// Size of an inode supporting indirect inodes, in bytes.
    pub static ref DINODE_SIZE : u64 = bincode::serialize(&DInode::default()).unwrap().len() as u64;
}

/// Wrapper around DInode supporting indirect inodes
#[derive(Debug, Default, PartialEq, Eq)]
pub struct IndirectInode {
    /// inode number
    pub inum: u64,
    /// the disk contents corresponding to `inum`
    pub disk_node: DInode,
}

impl IndirectInode {
    /// Create a new inode
    pub fn new(inum: u64, disk_node: DInode) -> IndirectInode {
        IndirectInode { inum, disk_node }
    }
    /// Return indirect block
    pub fn get_indirect_block(&self) -> u64 {
        self.disk_node.indirect_block
    }
}

/// Implementation of IndirectInode for new type of inode InodeLike
impl InodeLike for IndirectInode {
    fn new(inum: u64, ft: &FType, nlink: u64, size: u64, blocks: &[u64]) -> Option<Self> {
        if nlink > u16::MAX as u64 {
            return None;
        }
        // block size can by maximally DIRECT_POINTERS + 1
        if blocks.len() > (DIRECT_POINTERS + 1) as usize {
            return None;
        }

        let mut di = DInode {
            ft: *ft,
            nlink: nlink as u16,
            size,
            direct_blocks: [0; DIRECT_POINTERS as usize],
            indirect_block: 0,
        };
        // initialize blocks
        for i in 0..blocks.len() {
            if (i as u64) < DIRECT_POINTERS {
                di.direct_blocks[i] = blocks[i];
            } else {
                di.indirect_block = blocks[i];
            }
        }

        Some(IndirectInode::new(inum, di))
    }

    fn get_ft(&self) -> FType {
        self.disk_node.ft
    }
    fn get_nlink(&self) -> u64 {
        self.disk_node.nlink as u64
    }
    fn get_size(&self) -> u64 {
        self.disk_node.size
    }

    fn get_block(&self, i: u64) -> u64 {
        if DIRECT_POINTERS < i {
            return 0;
        }
        return if i == DIRECT_POINTERS {
            self.disk_node.indirect_block
        } else {
            self.disk_node.direct_blocks[i as usize]
        };
    }

    fn get_inum(&self) -> u64 {
        self.inum
    }
}

/// This error can occurs during manipulating with Indirect Inode File System
#[derive(Error, Debug)]
pub enum IndirectInodeLevelError {
    /// Error caused when `SuperBlock` to initialize file system is not valid.
    #[error("Invalid SuperBlock in BlockFileSystem initialization")]
    InvalidSuperBlock,
    /// Error caused when Inode operation is not valid.
    #[error("Invalid inode operation: {0}")]
    InvalidInodeOperation(&'static str),
    /// Error caused when performing inode read write operations.
    #[error("Inode error: {0}")]
    InodeRWError(&'static str),
    /// Error caused when performing controller operations.
    #[error("Controller error: {0}")]
    ControllerError(#[from] APIError),
    /// Error caused when performing controller operations.
    #[error("Block error: {0}")]
    BlockError(#[from] BlockLevelError),
}

/// `IndirectInodeFileSystem` struct wraps BlockFileSystem.
pub struct IndirectInodeFileSystem {
    /// Wrapped BlockFileSystem.
    pub block_fs: BlockFileSystem,
}

/// Implementation of FileSysSupport in IndirectInodeFileSystem
/// Nothing changed compared to InodeFileSystem implementation, only new type of DInode is used
impl FileSysSupport for IndirectInodeFileSystem {
    type Error = IndirectInodeLevelError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        return BlockFileSystem::sb_valid(sb);
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        if !Self::sb_valid(&sb) {
            return Err(IndirectInodeLevelError::InvalidSuperBlock);
        } else {
            // create device
            let mut device = Device::new(path, sb.block_size, sb.nblocks)?;
            // write super block to first block on disc
            let mut sb_block = Block::new_zero(0, sb.block_size);
            sb_block.serialize_into(&sb, 0)?;
            device.write_block(&sb_block)?;
            // initialize inodes
            let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
            // + 1 inode because first is not in use
            let num_blocks = (sb.ninodes + 1) / num_inodes_in_block + 1;
            // loop over all blocks to be used for storing inodes
            for b in 0..num_blocks {
                let mut i_block = Block::new_zero(sb.inodestart + b, sb.block_size);
                // loop over all inodes in current block and save free inodes
                for i in 0..num_inodes_in_block {
                    let mut dinode = DInode::default();
                    dinode.ft = FType::TFree;
                    i_block.serialize_into(&dinode, i * *DINODE_SIZE)?;
                }
                device.write_block(&i_block)?;
            }
            // write zeros to block of bitmap
            device.write_block(&Block::new_zero(sb.bmapstart, sb.block_size))?;
            // set all datablocks to zeros
            for i in 0..sb.ndatablocks {
                device.write_block(&Block::new_zero(sb.datastart + i, sb.block_size))?;
            }
            return Ok(Self {
                block_fs: BlockFileSystem {
                    device: Box::from(device),
                },
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

/// Implementation of BlockSupport in IndirectInodeFileSystem
/// Nothing was changed compare to BlockFileSystem
impl BlockSupport for IndirectInodeFileSystem {
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

/// Implementation of BlockSupport in IndirectInodeFileSystem
impl InodeSupport for IndirectInodeFileSystem {
    type Inode = IndirectInode;

    /// Nothing changed to previous InodeFileSystem implementation
    fn i_get(&self, i: u64) -> Result<Self::Inode, Self::Error> {
        let sb = self.sup_get()?;
        // Error if given inode index is out of bounds
        if i >= sb.ninodes {
            return Err(Self::Error::InvalidInodeOperation(
                "Given Inode index is higher then number of all inodes.",
            ));
        }
        // get block in which is inode placed
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        let block = self.b_get(sb.inodestart + (i / num_inodes_in_block))?;
        // read DInode from block and from offset and return DInode wrapped in Inode
        let dinode = block.deserialize_from::<DInode>((i % num_inodes_in_block) * *DINODE_SIZE)?;
        return Ok(Self::Inode::new(i, dinode));
    }

    /// Nothing changed to previous InodeFileSystem implementation
    fn i_put(&mut self, ino: &Self::Inode) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // get block in which inode should be placed
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        let mut block = self.b_get(sb.inodestart + ino.inum / num_inodes_in_block)?;
        // write DInode to corresponding offset in block
        block.serialize_into(
            &ino.disk_node,
            (ino.inum % num_inodes_in_block) * *DINODE_SIZE,
        )?;
        self.b_put(&block)?;
        return Ok(());
    }

    /// Differ to previous InodeFileSystem implementation only in freeing of indirect block
    fn i_free(&mut self, i: u64) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        let inode = self.i_get(i)?; // get inode with index i
                                    // Error if inode is already free
        if inode.get_ft() == FType::TFree {
            return Err(IndirectInodeLevelError::InvalidInodeOperation(
                "Inode is already free.",
            ));
        }
        // do nothing if inode is still referenced
        if inode.get_nlink() > 0 {
            return Ok(());
        }
        // if inode can be freed then
        // iterate all blocks and free them
        for b in 0..(inode.get_size() as f64 / sb.block_size as f64).ceil() as u64 {
            self.b_free(self.get_indirect_block(&inode, b)? - sb.datastart)?;
        }
        // here is difference from previous implementation
        // free also  indirect block if allocated
        if inode.get_block(DIRECT_POINTERS) != 0 {
            self.b_free(inode.get_block(DIRECT_POINTERS) - sb.datastart)?;
        }
        // replace inode by free inode
        self.i_put(&Self::Inode::new(i, DInode::default()))?;
        return Ok(());
    }

    /// Differ to previous InodeFileSystem implementation only in saving new type of Inode
    fn i_alloc(&mut self, ft: FType) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        // set start block index
        let mut block_index: u64 = sb.inodestart;
        let mut block = self.b_get(block_index)?;
        // iterate over all inodes
        for i in 1..sb.ninodes {
            // get next block if no more inodes in current block
            if i % num_inodes_in_block == 0 {
                block_index = block_index + 1;
                block = self.b_get(block_index)?;
            }
            // read inode on offset form block
            let offset = (i % num_inodes_in_block) * *DINODE_SIZE;
            let inode = block.deserialize_from::<DInode>(offset).unwrap();
            // if inode is free then initialize new one and return its index
            if inode.ft == FType::TFree {
                block.serialize_into(
                    &DInode {
                        ft: ft,
                        nlink: 0,
                        size: 0,
                        direct_blocks: [0; DIRECT_POINTERS as usize],
                        indirect_block: 0,
                    },
                    offset,
                )?;
                self.b_put(&block)?;
                return Ok(i);
            }
        }
        // Error if no free inode is available
        return Err(IndirectInodeLevelError::InvalidInodeOperation(
            "No free inode to allocate",
        ));
    }

    /// Differ to previous InodeFileSystem implementation only in freeing of indirect block
    fn i_trunc(&mut self, inode: &mut Self::Inode) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // iterate all blocks and free them
        for i in 0..(inode.get_size() as f64 / sb.block_size as f64).ceil() as u64 {
            self.b_free(self.get_indirect_block(inode, i)? - sb.datastart)?;
        }
        // here is difference from previous implementation
        // free also  indirect block
        if inode.get_block(DIRECT_POINTERS) != 0 {
            self.b_free(inode.get_block(DIRECT_POINTERS) - sb.datastart)?;
        }
        // reset all counters and save inode
        inode.disk_node.direct_blocks = [0; DIRECT_POINTERS as usize];
        inode.disk_node.nlink = 0;
        inode.disk_node.size = 0;
        inode.disk_node.indirect_block = 0;
        self.i_put(inode)?;
        return Ok(());
    }
}

impl InodeRWSupport for IndirectInodeFileSystem {
    /// Differ to previous InodeRWFileSystem implementation only in accessing blocks indices with helper function `get_indirect_block` from `IndirectBlockHelper`
    fn i_read(
        &self,
        inode: &Self::Inode,
        buf: &mut Buffer,
        off: u64,
        n: u64,
    ) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        // Return 0 if offset is on bound
        if off == inode.get_size() {
            return Ok(0);
        }
        // Error if offset is out of bounds
        if off > inode.get_size() {
            return Err(Self::Error::InodeRWError(
                "Number of bytes to be read exceeds given Buffer length.",
            ));
        }
        // Cut number of bytes to be written if buffer size is less then n
        let n = if buf.len() < n { buf.len() } else { n };
        // Get range of blocks where data to be read are allocated
        let start_block = off / sb.block_size;
        let end_block = (off + n) / sb.block_size;
        // Define first block offset where to start read
        let mut start_block_offset = off % sb.block_size;
        // Define first buff offset where to start write
        let mut buff_offset = 0;
        // Loop over block
        for b_i in start_block..(end_block + 1) {
            // Get last byte position in the block while data will be read
            // In current block we read data from interval (start_block_offset, end_block_offset)
            let end_block_offset = if start_block_offset + (n - buff_offset) >= sb.block_size {
                sb.block_size
            } else {
                start_block_offset + (n - buff_offset)
            };
            // get current block
            // difference we do not use previous `inode.get_block()` style but rather helper function
            // `get_indirect_block` to access direct blocks and also indirect blocks
            let block = self.b_get(self.get_indirect_block(inode, b_i)?)?;
            // Read from block and Write data to buffer
            let mut tmp_data = vec![0; (end_block_offset - start_block_offset) as usize];
            block.read_data(&mut tmp_data, start_block_offset)?;
            buf.write_data(&tmp_data, buff_offset)?;
            // Update buffer start offset and block start offset
            buff_offset += end_block_offset - start_block_offset;
            start_block_offset = 0;
        }
        return Ok(n);
    }

    /// Differ to previous InodeRWFileSystem implementation only in accessing and storing blocks indices with helper functions `get_indirect_block` and `set_indirect_block` from `IndirectBlockHelper`
    fn i_write(
        &mut self,
        inode: &mut Self::Inode,
        buf: &Buffer,
        off: u64,
        n: u64,
    ) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // Error if number of bytes to be written is higher then buffer length
        if buf.len() < n {
            return Err(Self::Error::InodeRWError(
                "Number of bytes to be written exceeds given Buffer length.",
            ));
        }
        // Error if start offset is out of bounds
        if off > inode.get_size() {
            return Err(Self::Error::InodeRWError("Offset is out of bounds."));
        }
        // Count number of currently allocated blocks
        let num_allocated_blocks = (inode.get_size() as f64 / sb.block_size as f64).ceil() as u64;
        // Get range of blocks where data should be written
        let start_block = off / sb.block_size;
        let end_block = (off + n) / sb.block_size;
        // Get first block offset where to start write
        let mut start_block_offset = off % sb.block_size;
        // Define start offset for buffer
        let mut buff_offset = 0;
        for b_i in start_block..(end_block + 1) {
            // Get end byte position in block while data will be written
            // In current block we write data to interval (start_block_offset, end_block_offset)
            let end_block_offset = if start_block_offset + (n - buff_offset) >= sb.block_size {
                sb.block_size
            } else {
                start_block_offset + (n - buff_offset)
            };
            // If current block is not allocated yet then allocate it
            if b_i >= num_allocated_blocks {
                let bnum = self.b_alloc()? + sb.datastart;
                // ! difference we do not use previous `inode.disk_node.direct_blocks[b_i as usize]` style
                // but rather helper function `set_indirect_block` to store direct blocks and also indirect blocks
                self.set_indirect_block(inode, b_i, bnum)?;
            }
            // ! difference we do not use previous `inode.get_block()` style but rather helper function
            // `get_indirect_block` to access direct blocks and also indirect blocks
            let mut block = self.b_get(self.get_indirect_block(inode, b_i)?)?;
            // Read data from buffer and  write to current block
            let mut tmp_data = vec![0; (end_block_offset - start_block_offset) as usize];
            buf.read_data(&mut tmp_data, buff_offset)?;
            block.write_data(&tmp_data, start_block_offset)?;
            self.b_put(&block)?;
            // Update buffer start offset and block start offset
            buff_offset += end_block_offset - start_block_offset;
            start_block_offset = 0;
        }
        // Update size of inode
        if off + n > inode.get_size() {
            inode.disk_node.size = off + n;
        }
        self.i_put(&inode)?;
        return Ok(());
    }
}

/// IndirectBlockHelper serves as helper to manipulate with inodes data block
pub trait IndirectBlockHelper: InodeRWSupport {
    /// Set block number with index `i` to `inode`
    /// If `i` is less then DIRECT_POINTERS then add `bnum` to direct_blocks
    /// If `i` is higher or equal to DIRECT_POINTERS then save `bnum` to indirect block
    /// Allocate indirect block if number of blocks is over DIRECT_POINTERS size
    fn set_indirect_block(
        &mut self,
        inode: &mut Self::Inode,
        i: u64,
        bnum: u64,
    ) -> Result<(), Self::Error>;

    /// Get block number with index `i` from `inode`
    /// If `i` is less then DIRECT_POINTERS then get `bnum` from direct_blocks
    /// If `i` is higher or equal to DIRECT_POINTERS then read `bnum` from indirect block
    fn get_indirect_block(&self, inode: &Self::Inode, i: u64) -> Result<u64, Self::Error>;
}

impl IndirectBlockHelper for IndirectInodeFileSystem {
    fn set_indirect_block(
        &mut self,
        inode: &mut Self::Inode,
        i: u64,
        bnum: u64,
    ) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // check if index exceed capacity of direct_blocks
        if i < DIRECT_POINTERS {
            inode.disk_node.direct_blocks[i as usize] = bnum;
        } else {
            // if indirect block is not allocated yet then allocate new and set to indirect_block
            if inode.get_block(DIRECT_POINTERS) == 0 {
                let a = self.b_alloc()? + sb.datastart;
                inode.disk_node.indirect_block = a;
            }
            // wrap indirect_block block to IndirectBlock helper
            let mut indirect_block = IndirectBlock::new(self.b_get(inode.get_indirect_block())?);
            // save bnum to block
            indirect_block.put_block_num(i - DIRECT_POINTERS, bnum)?;
            // finally save block back
            self.b_put(&indirect_block.return_block())?;
        }
        Ok(())
    }

    fn get_indirect_block(&self, inode: &Self::Inode, i: u64) -> Result<u64, Self::Error> {
        // check if index is higher then size of direct_blocks
        if i < DIRECT_POINTERS {
            Ok(inode.disk_node.direct_blocks[i as usize])
        } else {
            // wrap indirect_block block to IndirectBlock helper and read block number form block
            let indirect_block = IndirectBlock::new(self.b_get(inode.get_indirect_block())?);
            Ok(indirect_block.get_block_num(i - DIRECT_POINTERS)?)
        }
    }
}

/// Wrapped block to create abstraction for indirect block
pub struct IndirectBlock {
    /// block as boxed Block
    pub block: Box<Block>,
}

impl IndirectBlock {
    /// Create new IndirectBlock, having given `Block`.
    pub fn new(block: Block) -> IndirectBlock {
        return IndirectBlock {
            block: Box::from(block),
        };
    }

    /// Read block number value in `Block` buffer with *i*th position.
    pub fn get_block_num(&self, i: u64) -> Result<u64, IndirectInodeLevelError> {
        let offset = i * 64;
        Ok(self.block.deserialize_from::<u64>(offset)?)
    }

    /// Write value to *i*th position in `Block` buffer.
    pub fn put_block_num(&mut self, i: u64, val: u64) -> Result<(), IndirectInodeLevelError> {
        let offset = i * 64;
        self.block.serialize_into(&val, offset)?;
        Ok(())
    }

    /// Return wrapped block back.
    pub fn return_block(self) -> Block {
        return *self.block;
    }
}

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.m
pub type FSName = IndirectInodeFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeRWSupport, InodeSupport};
    use cplfs_api::types::{Buffer, FType, InodeLike, SuperBlock, DIRECT_POINTERS};

    use crate::f_indirect_inodes::{
        DInode, FSName, IndirectBlock, IndirectBlockHelper, IndirectInode,
    };

    #[path = "utils.rs"]
    mod utils;

    static BLOCK_SIZE: u64 = 300;
    static NBLOCKS: u64 = 40;
    static SUPERBLOCK_GOOD: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE,
        nblocks: NBLOCKS,
        ninodes: 6,
        inodestart: 1,
        ndatablocks: 30,
        bmapstart: 4,
        datastart: 5,
    };

    #[test]
    fn indirect_block_test() {
        // Tests for IndirectBlock wrapper
        let path = utils::disk_prep_path("indirect_block_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let bnum = my_fs.b_alloc().unwrap();
        let mut i_block = IndirectBlock::new(my_fs.b_get(bnum).unwrap());
        // save block number to block and read same block number
        assert!(i_block.put_block_num(0, 1).is_ok());
        assert_eq!(i_block.get_block_num(0).unwrap(), 1);

        assert!(i_block.put_block_num(1, 85).is_ok());
        assert_eq!(i_block.get_block_num(1).unwrap(), 85);
        // index out of bounds
        assert!(i_block.put_block_num(BLOCK_SIZE / 64 + 1, 85).is_err());

        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn indirect_block_helper_test() {
        // Tests for IndirectBlock wrapper
        let path = utils::disk_prep_path("indirect_block_helper_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let mut inode = my_fs.i_get(1).unwrap();
        // not allocated indirect block
        assert_eq!(inode.get_block(DIRECT_POINTERS), 0);
        // save all direct pointers
        for i in 0..DIRECT_POINTERS {
            my_fs.set_indirect_block(&mut inode, i, i).unwrap();
        }
        for i in 0..DIRECT_POINTERS {
            assert_eq!(my_fs.get_indirect_block(&mut inode, i).unwrap(), i);
        }
        assert_eq!(inode.get_block(DIRECT_POINTERS), 0);
        // save first indirect block
        assert!(my_fs
            .set_indirect_block(&mut inode, DIRECT_POINTERS, 45)
            .is_ok());
        // should be allocated block to storing indirect blocks
        assert_eq!(inode.get_indirect_block(), SUPERBLOCK_GOOD.datastart);
        assert_eq!(
            my_fs.get_indirect_block(&inode, DIRECT_POINTERS).unwrap(),
            45
        );

        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn read_write_test() {
        // Tests for IndirectBlock wrapper
        let path = utils::disk_prep_path("read_write_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        //Set up an inode with 3 blocks first
        for i in 0..3 {
            assert_eq!(my_fs.b_alloc().unwrap(), i);
        }
        let b2 = utils::n_block(5, BLOCK_SIZE, 2);
        my_fs.b_put(&b2).unwrap();
        let b3 = utils::n_block(6, BLOCK_SIZE, 3);
        my_fs.b_put(&b3).unwrap();
        let b4 = utils::n_block(7, BLOCK_SIZE, 4);
        my_fs.b_put(&b4).unwrap();
        let mut i2 = IndirectInode::new(
            2,
            DInode {
                ft: FType::TFile,
                nlink: 0,
                size: (2.5 * (BLOCK_SIZE as f32)) as u64,
                direct_blocks: [5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                indirect_block: 0,
            },
        );
        my_fs.i_put(&i2).unwrap(); //Store the inode to disk as well

        //Some buffers
        let mut buf4000 = Buffer::new(vec![7; 4000].into_boxed_slice());

        // try to write to more then DIRECT_POINTERs blocks
        let write_result = vec![7; BLOCK_SIZE as usize];
        assert!(my_fs.i_write(&mut i2, &mut buf4000, 600, 4000).is_ok());
        assert_eq!(my_fs.i_get(2).unwrap(), i2);
        assert_eq!(4600, i2.get_size());
        // in first indirect blocks should be all 7
        assert_eq!(
            my_fs
                .b_get(my_fs.get_indirect_block(&i2, DIRECT_POINTERS).unwrap())
                .unwrap()
                .contents_as_ref(),
            &write_result[..]
        );
        // try to read any data from border between direct and indirect blocks
        let mut buf800 = Buffer::new_zero(800);
        let read_result = vec![7; 800];
        assert_eq!(my_fs.i_read(&i2, &mut buf800, 3280, 800).unwrap(), 800);
        assert_eq!(&buf800.contents_as_ref()[..800], &read_result[..]);
        // check if inodes id truncated
        assert!(my_fs.i_trunc(&mut i2).is_ok());
        assert_eq!(my_fs.i_get(2).unwrap(), i2);
        assert_eq!(0, i2.get_size());
        assert_eq!(0, i2.get_indirect_block());
        for i in 0..DIRECT_POINTERS {
            assert_eq!(0, i2.get_block(i));
        }
        // check if block are freed
        assert!(my_fs.i_free(6).is_err());
        assert!(my_fs.i_free(20).is_err());
        // try to write some data again
        assert!(my_fs.i_write(&mut i2, &mut buf4000, 0, 4000).is_ok());
        assert_eq!(4000, i2.get_size());
        // assert all direct blocks
        for i in 0..DIRECT_POINTERS {
            assert_eq!(
                my_fs.get_indirect_block(&i2, i).unwrap(),
                i + SUPERBLOCK_GOOD.datastart
            )
        }
        // also first indirect block and pointer to indirect blocks
        assert_eq!(
            my_fs.get_indirect_block(&i2, DIRECT_POINTERS).unwrap(),
            12 + SUPERBLOCK_GOOD.datastart
        );
        assert_eq!(i2.get_indirect_block(), 13 + SUPERBLOCK_GOOD.datastart);
        // free inode
        assert!(my_fs.i_free(2).is_ok());
        // all blocks should be free
        for i in 0..(4000 / BLOCK_SIZE) + 1 {
            assert!(my_fs.b_free(i).is_err());
        }
        assert_eq!(my_fs.i_get(2).unwrap().get_ft(), FType::TFree);

        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }
}
// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "f", feature = "all")))]
#[path = "../../api/fs-tests/f_test.rs"]
mod tests;
