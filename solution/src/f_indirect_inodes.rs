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
use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeRWSupport, InodeSupport};
use cplfs_api::types::{Block, Buffer, DIRECT_POINTERS, FType, Inode, InodeLike, SuperBlock};
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::a_block_support::{BitwiseBlock, BlockFileSystem, BlockLevelError};
use crate::e_inode_RW_support::InodeRWError;

/// any documentation
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
pub struct DInode {
    /// Registers the file type
    pub ft: FType,
    /// Counts the number of links to this inode in the file system. The point of doing this is that if the inode is written back to disk when it has no links to it, it should be freed instead, thereby setting its file type to `T_FREE`.
    pub nlink: u16,
    /// Size of the file in bytes. Used to see when a read or write would go out of file bounds.
    pub size: u64,
    /// A list of up to `DIRECT_POINTERS + 1` valid block addresses (counting from 0, *not* from the start of the data block region), to specify where the contents of this file are stored.
    pub direct_blocks: [u64; (DIRECT_POINTERS + 1) as usize],
}

lazy_static! {
    /// Size of an inode in your system, in bytes.
    /// This size can only be found out at runtime, which is the reason why we have to wrap this code in a `lazy_static` macro.
    /// Notice the use of the `ref` keyword; `DINODE_SIZE` is a reference to an `u64` number, that will only be filled in at runtime.
    /// Used to determine the number of inodes per block, which is important for filesystem initialization.
    pub static ref DINODE_SIZE : u64 = bincode::serialize(&DInode::default()).unwrap().len() as u64;
}


/// s fda d
pub struct IndirectInode {
    /// inode number
    pub inum: u64,
    /// the disk contents corresponding to `inum`
    pub disk_node: DInode,
}

impl IndirectInode {
    /// Create a new inode
    pub fn new(inum: u64, disk_node: DInode) -> IndirectInode {
        IndirectInode { inum, disk_node}
    }

    fn set_block(&mut self, i: u64, bnum: u64) -> Option<()> {
        if DIRECT_POINTERS <= i {
            return None;
        }
        self.disk_node.direct_blocks[i as usize] = bnum;
        Some(())
    }
}

impl InodeLike for IndirectInode {
    fn new(inum: u64, ft: &FType, nlink: u64, size: u64, blocks: &[u64]) -> Option<Self> {
        if nlink > u16::MAX as u64 {
            return None;
        }

        if blocks.len() > (DIRECT_POINTERS + 1) as usize {
            return None;
        }


        let mut db = [0; (DIRECT_POINTERS + 1) as usize];
        for i in 0..blocks.len() {
            db[i] = blocks[i];
        }

        let di = DInode {
            ft: *ft,
            nlink: nlink as u16,
            size,
            direct_blocks: db,
        };
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
        self.disk_node.direct_blocks[i as usize]
    }

    fn get_inum(&self) -> u64 {
        self.inum
    }
}


/// This error can occurs during manipulating with Block File System
#[derive(Error, Debug)]
pub enum IndirectInodeLevelError {
    /// Error caused when `SuperBlock` to initialize `BlockFileSystem` is not valid.
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
    ///This error has mostly been added for illustrative purposes, and can be useful for quickly drafting some code without thinking about the concrete error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// `InodeFileSystem` struct implements `FileSysSupport` and the `BlockSupport`. Structure wraps `Device` to offer block-level abstraction to operate with File System.
pub struct IndirectInodeFileSystem {
    /// Wrapped BlockFileSystem.
    pub block_fs: BlockFileSystem,
}

/// Implementation of FileSysSupport in BlockFileSystem
impl FileSysSupport for IndirectInodeFileSystem {
    type Error = IndirectInodeLevelError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        return BlockFileSystem::sb_valid(sb);
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        if !Self::sb_valid(&sb) {
            return Err(IndirectInodeLevelError::InvalidSuperBlock);
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

/// Implementation of BlockSupport in BlockFileSystem
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

impl InodeSupport for IndirectInodeFileSystem {
    type Inode = IndirectInode;

    fn i_get(&self, i: u64) -> Result<Self::Inode, Self::Error> {
        let sb = self.sup_get()?;
        if i >= sb.ninodes {
            return Err(IndirectInodeLevelError::InvalidInodeOperation(
                "Given Inode index is higher then number of all inodes.",
            ));
        }
        let num_inodes_in_block = sb.block_size / *DINODE_SIZE;
        let block = self.b_get(sb.inodestart + (i / num_inodes_in_block))?;
        let dinode = block.deserialize_from::<DInode>((i % num_inodes_in_block) * *DINODE_SIZE)?;
        return Ok(Self::Inode::new(i, dinode, ));
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
        if inode.get_ft() == FType::TFree {
            // if inode is already free
            return Err(IndirectInodeLevelError::InvalidInodeOperation(
                "Inode is already free.",
            ));
        }
        if inode.get_nlink() > 0 {
            // if inode is still referenced
            return Ok(());
        }
        // iterate all blocks and free them
        for b in 0..(inode.get_size() as f64 / sb.block_size as f64).ceil() as u64 {
            if b == DIRECT_POINTERS {continue;}
            self.b_free(self.get_block(&inode, b)? - sb.datastart)?;
        }
        // free also  indirect block
        if inode.get_block( DIRECT_POINTERS) != 0 {
            self.b_free(inode.get_block(DIRECT_POINTERS) - sb.datastart)?;
        }
        // put free inode
        self.i_put(&Self::Inode::new(i, DInode::default()))?;
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
                        direct_blocks: [0; (DIRECT_POINTERS + 1) as usize],
                    },
                    offset,
                )?;
                self.b_put(&block)?;
                return Ok(i);
            }
        }
        return Err(IndirectInodeLevelError::InvalidInodeOperation(
            "No free inode to allocate",
        ));
    }

    fn i_trunc(&mut self, inode: &mut Self::Inode) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // iterate all blocks and free them
        for i in 0..(inode.get_size() as f64 / sb.block_size as f64).ceil() as u64 {
            if i == DIRECT_POINTERS {continue}
            self.b_free(self.get_block(inode, i)? - sb.datastart)?;
        }
        // free also  indirect block
        if inode.get_block(DIRECT_POINTERS) != 0 {
            self.b_free(inode.get_block(DIRECT_POINTERS) - sb.datastart)?;
        }
        // put inode
        inode.disk_node.direct_blocks = [0; (DIRECT_POINTERS + 1) as usize];
        inode.disk_node.nlink = 0;
        inode.disk_node.size = 0;
        self.i_put(inode)?;
        return Ok(());
    }
}

impl InodeRWSupport for IndirectInodeFileSystem {
    fn i_read(&self, inode: &Self::Inode, buf: &mut Buffer, off: u64, n: u64) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        // Return 0 if offset is on bound
        if off == inode.get_size() {
            return Ok(0);
        }
        // Error if offset is out of bounds
        if off > inode.get_size() {
            return Err(Self::Error::InodeRWError(
                "Number of bytes to be read exceeds given Buffer length."));
        }
        // Cut number of bytes to be written if buffer size is less then n
        let n = if buf.len() < n { buf.len() } else { n };
        // Get range of blocks where data to be read are allocated
        let start_block = off / sb.block_size;
        let end_block = (off + n)  / sb.block_size;
        // Define first block offset where to start read
        let mut start_block_offset = off % sb.block_size;
        // Define first buff offset where to start write
        let mut buff_offset = 0;
        // Loop over block
        for b_i in start_block..(end_block + 1) {
            // Get end byte position in block while data will be read
            // From current block we read data from interval (start_block_offset, end_block_offset)
            let end_block_offset = if start_block_offset + (n - buff_offset) >= sb.block_size {
                sb.block_size
            } else {
                start_block_offset + (n - buff_offset)
            };
            // get current block
            let block = self.b_get(self.get_block(inode, b_i)?)?;
            // Read and Write data to buffer
            let mut tmp_data = vec![0; (end_block_offset - start_block_offset) as usize];
            block.read_data(&mut tmp_data, start_block_offset)?;
            buf.write_data(&tmp_data, buff_offset)?;
            // Update buffer start offset and block start offset
            buff_offset += end_block_offset - start_block_offset;
            start_block_offset = 0;
        }
        return Ok(n);
    }

    fn i_write(&mut self, inode: &mut Self::Inode, buf: &Buffer, off: u64, n: u64) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        // Error if number of bytes to be written is higher then buffer length
        if buf.len() < n {
            return Err(Self::Error::InodeRWError(
                "Number of bytes to be written exceeds given Buffer length."));
        }
        // Error if start offset is out of bounds
        if off > inode.get_size() {
            return Err(Self::Error::InodeRWError(
                "Offset is out of bounds."));
        }
        // Count number of currently allocated blocks
        let num_allocated_blocks = (inode.get_size() as f64 / sb.block_size as f64).ceil() as u64;
        // Get range of blocks where data should be written
        let start_block = off / sb.block_size;
        let end_block = (off + n)  / sb.block_size;
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
                self.set_block(inode, b_i, bnum)?;
            }
            let mut block = self.b_get(self.get_block(inode, b_i)?)?;
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

trait IndirectBlockHelper: InodeRWSupport {

    fn set_block(&mut self, inode: &mut Self::Inode, i: u64, bnum: u64) -> Result<(), Self::Error>;

    fn get_block(&self, inode: &Self::Inode, i: u64) -> Result<u64, Self::Error>;
}

impl IndirectBlockHelper for IndirectInodeFileSystem {

    fn set_block(&mut self, inode: &mut Self::Inode, i: u64, bnum: u64) -> Result<(), Self::Error> {
        let sb = self.sup_get()?;
        if i < DIRECT_POINTERS {
            inode.disk_node.direct_blocks[i as usize] = bnum;
        } else {
            if inode.get_block(DIRECT_POINTERS) == 0 {
                let a = self.b_alloc()? + sb.datastart;
                inode.disk_node.direct_blocks[DIRECT_POINTERS as usize] = a;
            }
            let mut indirect_block = IndirectBlock::new(
                self.b_get(inode.get_block(DIRECT_POINTERS))?);
            indirect_block.put_block_num(i - DIRECT_POINTERS, bnum)?;
            self.b_put(&indirect_block.return_block());
        }
        Ok(())
    }

    fn get_block(&self, inode: &Self::Inode, i: u64) -> Result<u64, Self::Error> {
        if i < DIRECT_POINTERS {
            Ok(inode.disk_node.direct_blocks[i as usize])
        } else {
            let indirect_block = IndirectBlock::new(self.b_get(inode.get_block(DIRECT_POINTERS))?);
            Ok(indirect_block.get_block_num(i - DIRECT_POINTERS)?)
        }
    }
}

/// any doc
pub struct IndirectBlock {
    /// block as boxed Block
    pub block: Box<Block>,
}

impl IndirectBlock {
    /// Create new BitwiseBlock, having given `Block`.
    pub fn new(block: Block) -> IndirectBlock {
        return IndirectBlock {
            block: Box::from(block),
        };
    }

    /// Read byte value in `Block` buffer with *i*th position.
    pub fn get_block_num(&self, i: u64) -> Result<u64, IndirectInodeLevelError> {
        let offset = i * 64;
        Ok(self.block.deserialize_from::<u64>( offset)?)
    }

    /// Write value to *i*th byte in `Block` buffer.
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
    use cplfs_api::types::{Buffer, DInode, FType, Inode, InodeLike, SuperBlock};

    use crate::f_indirect_inodes::{FSName, IndirectBlock};

    #[path = "utils.rs"]
    mod utils;

    static BLOCK_SIZE: u64 = 300;
    static NBLOCKS: u64 = 11;
    static SUPERBLOCK_GOOD: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE,
        nblocks: NBLOCKS,
        ninodes: 6,
        inodestart: 1,
        ndatablocks: 6,
        bmapstart: 4,
        datastart: 5,
    };

    #[test]
    fn indirect_block_test() {
        let path = utils::disk_prep_path("indirect_block_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        let bnum = my_fs.b_alloc().unwrap();
        let mut i_block = IndirectBlock::new(my_fs.b_get(bnum).unwrap());

        let bnum2 = my_fs.b_alloc().unwrap();
        assert_eq!(bnum2, 1);
        assert!(i_block.put_block_num(0,bnum2).is_ok());
        assert_eq!(i_block.get_block_num(0).unwrap(), 1);


        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }
}
// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "f", feature = "all")))]
#[path = "../../api/fs-tests/f_test.rs"]
mod tests;
