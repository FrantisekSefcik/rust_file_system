//! File system with inode support + read and write operations on inodes
//!
//! Create a filesystem that has a notion of inodes and blocks, by implementing the [`FileSysSupport`], the [`BlockSupport`] and the [`InodeSupport`] traits together (again, all earlier traits are supertraits of the later ones).
//! Additionally, implement the [`InodeRWSupport`] trait to provide operations to read from and write to inodes
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
//! [`InodeSupport`]: ../../cplfs_api/fs/trait.InodeSupport.html
//! [`InodeRWSupport`]: ../../cplfs_api/fs/trait.InodeRWSupport.html
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

use thiserror::Error;

use crate::b_inode_support::{InodeLevelError, InodeFileSystem};
use cplfs_api::error_given::APIError;
use cplfs_api::fs::{FileSysSupport, InodeSupport, BlockSupport, InodeRWSupport};
use cplfs_api::types::{SuperBlock, Inode, DInode, FType, DIRECT_POINTERS, Block, Buffer, InodeLike};
use std::path::Path;
use cplfs_api::controller::Device;

/// This error can occurs during manipulating with Directory File System
#[derive(Error, Debug)]
pub enum InodeRWError {
    /// Error caused when performing controller operations.
    #[error("Inode error: {0}")]
    InodeError(#[from] InodeLevelError),
    /// Error caused when Directory operation is not valid.
    #[error("{0}")]
    InodeReadWriteError(&'static str),
    /// Error caused when performing controller operations.
    #[error("Controller error: {0}")]
    ControllerError(#[from] APIError),
    ///This error has mostly been added for illustrative purposes, and can be useful for quickly drafting some code without thinking about the concrete error
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// `DirectoryFileSystem` wraps InodeFileSystem and add Directory support
pub struct InodeRWFileSystem {
    /// wrapped Inodes support layer
    inode_fs: InodeFileSystem,
}

/// `InodeFileSystem` struct implements `FileSysSupport` and the `BlockSupport`. Structure wraps `Device` to offer block-level abstraction to operate with File System.
/// Implementation of FileSysSupport in BlockFileSystem
impl FileSysSupport for InodeRWFileSystem {
    type Error = InodeRWError;

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
impl BlockSupport for InodeRWFileSystem {
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

impl InodeSupport for InodeRWFileSystem {
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

impl InodeRWSupport for InodeRWFileSystem {

    fn i_read(&self, inode: &Self::Inode, buf: &mut Buffer, off: u64, n: u64) -> Result<u64, Self::Error> {
        let sb = self.sup_get()?;
        // Return 0 if offset is on bound
        if off == inode.get_size() {
            return Ok(0);
        }
        // Error if offset is out of bounds
        if off > inode.get_size() {
            return Err(Self::Error::InodeReadWriteError(
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
            let mut block = self.b_get(inode.get_block(b_i))?;
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
            return Err(Self::Error::InodeReadWriteError(
                "Number of bytes to be written exceeds given Buffer length."));
        }
        // Error if start offset is out of bounds
        if off > inode.get_size() {
            return Err(Self::Error::InodeReadWriteError(
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
                inode.disk_node.direct_blocks[b_i as usize] = self.b_alloc()? + sb.datastart;
            }
            let mut block = self.b_get(inode.get_block(b_i))?;
            // Read data from buffer and  write to current block
            let mut tmp_data = vec![0; (end_block_offset - start_block_offset) as usize];
            buf.read_data(&mut tmp_data, buff_offset)?;
            block.write_data(&tmp_data, start_block_offset)?;
            self.b_put(&block);
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


/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
pub type FSName = InodeRWFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use cplfs_api::fs::{BlockSupport, FileSysSupport, InodeSupport, InodeRWSupport};
    use cplfs_api::types::{DInode, FType, Inode, InodeLike, SuperBlock, Buffer};

    use crate::e_inode_RW_support::FSName;

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
    fn i_read_test() {
        let path = utils::disk_prep_path("i_read_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        //Set up an inode with 3 blocks first
        for i in 0..5 {
            assert_eq!(my_fs.b_alloc().unwrap(), i);
        }
        let b2 = utils::n_block(5, BLOCK_SIZE, 2);
        my_fs.b_put(&b2).unwrap();
        let b3 = utils::n_block(6, BLOCK_SIZE, 3);
        my_fs.b_put(&b3).unwrap();
        let b4 = utils::n_block(7, BLOCK_SIZE, 4);
        my_fs.b_put(&b4).unwrap();
        let mut i2 = Inode::new(
            2,
            DInode {
                ft: FType::TFile,
                nlink: 0,
                size: 3 * BLOCK_SIZE,
                direct_blocks: [5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            }
        );
        my_fs.i_put(&i2).unwrap(); //Store the inode to disk as well

        //Some buffers
        let mut buf50 = Buffer::new_zero(50);
        let mut buf500 = Buffer::new_zero(500);

        //Try to perform some operations
        let mut read_result = vec![2; 33];
        read_result.append(&mut vec![3; 300]);
        read_result.append(&mut vec![4; 155]);
        assert_eq!(my_fs.i_read(&i2, &mut buf500, 267, 488).unwrap(), 488);
        assert_eq!(
            &buf500.contents_as_ref()[..488].len(),
            &read_result[..].len()
        );
        assert_eq!(&buf500.contents_as_ref()[..488], &read_result[..]);

        let mut read_result_2 = vec![2; 33];
        read_result_2.append(&mut vec![3; 17]);
        assert_eq!(my_fs.i_read(&i2, &mut buf50, 267, 50).unwrap(), 50);
        assert_eq!(&buf50.contents_as_ref()[..], &read_result_2[..]);

        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn i_write_test() {
        let path = utils::disk_prep_path("i_write_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD).unwrap();

        //Set up an inode with 3 blocks first
        for i in 0..4 {
            assert_eq!(my_fs.b_alloc().unwrap(), i);
        }
        let b2 = utils::n_block(5, BLOCK_SIZE, 2);
        my_fs.b_put(&b2).unwrap();
        let b3 = utils::n_block(6, BLOCK_SIZE, 3);
        my_fs.b_put(&b3).unwrap();
        let b4 = utils::n_block(7, BLOCK_SIZE, 4);
        my_fs.b_put(&b4).unwrap();
        let mut i2 = Inode::new(
            2,
            DInode {
                ft: FType::TFile,
                nlink: 0,
                size: (2.5 * (BLOCK_SIZE as f32)) as u64,
                direct_blocks: [5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            }
        );
        my_fs.i_put(&i2).unwrap(); //Store the inode to disk as well

        //Some buffers
        let mut buf50 = Buffer::new(vec![5; 50].into_boxed_slice());
        let mut buf500 = Buffer::new(vec![6; 500].into_boxed_slice());

        //Try to perform some operations:

        //One regular one that only changes the size
        let mut write_result = vec![4; 103];
        write_result.append(&mut vec![5; 50]);
        write_result.append(&mut vec![4; 147]);
        assert!(my_fs.i_write(&mut i2, &mut buf50, 703, 50).is_ok());
        assert_eq!(my_fs.i_get(2).unwrap(), i2);
        assert_eq!(753, i2.get_size());
        assert_eq!(my_fs.b_get(7).unwrap().contents_as_ref(), &write_result[..]);

        //And one that will require mapping in new blocks
        let mut write_result_2 = vec![6; 52];
        write_result_2.append(&mut vec![0; 248]);
        my_fs.i_write(&mut i2, &mut buf500, 753, 499).unwrap();
        assert_eq!(my_fs.i_get(2).unwrap(), i2);
        assert_eq!(1252, i2.get_size());
        assert_eq!(9, i2.get_block(3));
        assert_eq!(10, i2.get_block(4));
        assert_eq!(
            my_fs.b_get(10).unwrap().contents_as_ref(),
            &write_result_2[..]
        );
        my_fs.b_free(4).unwrap();

        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }
}

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "e", feature = "all")))]
#[path = "../../api/fs-tests/e_test.rs"]
mod tests;
