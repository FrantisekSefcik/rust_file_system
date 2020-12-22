//! File system with support for inode caching\
//! Reimplementation of the inodes from the base project.
//!
//! Create a filesystem that has a notion of inodes and blocks and allows you to have a certain number of inodes in an inode cache, by implementing the [`FileSysSupport`], the [`BlockSupport`], the [`InodeSupport`] and the [`InodeRWSupport`] and [`InodeCacheSupport`] traits together (again, all earlier traits are supertraits of the last two).
//!
//! [`FileSysSupport`]: ../../cplfs_api/fs/trait.FileSysSupport.html
//! [`BlockSupport`]: ../../cplfs_api/fs/trait.BlockSupport.html
//! [`InodeSupport`]: ../../cplfs_api/fs/trait.InodeSupport.html
//! [`InodeRWSupport`]: ../../cplfs_api/fs/trait.InodeRWSupport.html
//! [`InodeCacheSupport`]: ../../cplfs_api/fs/trait.InodeCacheSupport.html
//!
//! You have to support caching of inodes through the use of an *inode cache*, i.e. we want to make sure that:
//! - Inodes that have been read before do not have to be read from the disk again, if they are still in the cache. This is a performance improvement.
//! - If the same inode is read from the disk multiple, and multiple copies of it are in use at the same time, then each one of these copies refers to the *same* inode, and not to independent, owned instances of it, as was the case in the base project. This is a usability improvement. In other words, if one inode is read from the disk into the cache and a reference to this cache entry is kept in the code in different locations, changes to the inode in one location should be visible in all other locations, i.e. clients of our code do not have to be as careful anymore updating inodes.
//!
//! Caching provides some practical issues in Rust, given that the type system does not allow us to have multiple mutable references to a single cache entry at the same time.
//! The naive solution where `i_get` returns some form of mutable reference to an inode in the cache hence does not work.
//! Essentially, the problem is that it is impossible to know statically that the cache entries will be used in a safe manner.
//! Clearly, we have to enforce ownership and borrowing at runtime.
//! One possible way of doing this consists of two parts:
//! - Use a `RefCell` to make sure that borrowing rules are only checked at runtime, i.e. use the `borrow` and `borrow_mut` methods on the `RefCell` type to perform borrow checking at runtime. `RefCell` allows for *interior mutability*, in the sense that a regular reference to a value of type `RefCell` still allows its contents to be mutated. This is safe, since `RefCell` checks the borrowing rules at runtime regardless.
//! Read more about this [here](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html).
//! - `RefCell` has some limitations; it still only allows a single party to have ownership of its values. This will not suffice if we want to keep multiple copies of a cached entry in memory. To this end, we can wrap our `RefCell`s in the `Rc` (reference count) type; this type allows us to have multiple (immutable) copies of the value it wraps. A `Rc` value keeps track of the number of owners at each point in time, and will only free its contents when the last owner goes out of scope. Read more about this [here](https://doc.rust-lang.org/book/ch15-04-rc.html). This type interacts very nicely with `RefCell`, since an immutable reference suffices to be allowed to mutate the `RefCell`'s contents.
//!
//! Using a combination of these two types, we can now create a shareable wrapper for our original inode type as follows:
//! ```ignore
//!use std::cell::RefCell;
//!use std::rc::Rc;
//! #[derive(Debug, Default, PartialEq, Eq, Clone)]
//! pub struct InodeCached(Rc<RefCell<Inode>>);
//! ```
//! You should still implement `InodeLike` so that you can use this wrapper in your trait implementations. Additionally, think of some useful helper methods to define on this type.
//!
//! If we create a fixed-size cache data structure (pick an appropriate type for this cache in your implementation) that contains entries of this `InodeCached` type, we can actually hand out multiple copies (by cloning the `Rc` value, this implementation of `clone` is how `InodeCached` is able to derive the `Clone` trait in the above code), and make sure that they are used safely (thanks to the dynamic checking of `RefCell`).
//! This solution is still not entirely realistic, as the cache's contents will be dynamically allocated and spread across the heap when we create new `InodeCached` instances from `Inodes`, but it is already a big step in the right direction.
//!
//! Now it is your turn. Implement the aforementioned cache structure, add it to your previous filesystem implementation with inodes, and make sure to implement the `InodeCacheSupport` trait.
//! Additionally, go back and fix the implementations of the functions in the `InodeSupport` to make them aware of our caching schema.
//! The following changes are required to the functions that you implemented before as part of the `InodeSupport` trait:
//! - `i_get` takes an immutable reference to `self`, and will hence be incapable of making any changes to the cache. For this reason, the `InodeCacheSupport` trait provides a new method `i_get_mut`, which takes a mutable reference to self, and hence allows updating the cache as part of the read process. More concretely `i_get` will look for an inode entry in the inode cache only, return a reference to it if it finds it and error otherwise. On the other hand, `i_get_mut` will first look in the cache and copy the behavior of `i_get`, but rather than returning an error on lookup failure, read the inode number from the disk instead. See the documentation of `i_get_mut` for more information.
//! - `i_put` still takes a reference to an inode and writes it back to the disk. The only difference is that the provided reference is now a reference to a cached inode, but this should not matter much for your implementation
//! - `i_free`: the new implementation of `i_free` differs from the old implementation (without caching) like `i_get_mut` differs from `i_get`.
//! The new implementation first tries to free the inode `i` from the cache. If the node is found, the following happens:
//!     - Returns an error if the node is still referenced elsewhere (again, you can check this through the `strong_count` method on the `Rc` type)
//!     - Does nothing and returns with an `Ok` if there are other links to this inode still (as was the case before)
//!     - Errors when trying to free an already free inode (as was the case before)
//!     - If the previous 3 cases do not occur, we can actually free the inode, as specified in `i_free`. Make sure the freed inode is written back to disk in the end.
//! If the inode is not cached, the disk inode is fetched from disk (*WARNING*: this disk inode should **NOT** end up in the cache, as we are about to free it anyways). The previous checks are then repeated, and the freed disk inode is persisted.
//! - One change to `i_alloc` is that the allocated inode will now be read into the cache too (but not returned), replacing a pre-existing free entry for the same inode if necessary.
//! We have to do this to avoid a remaining free entry in the cache for the allocated inode shadowing our allocated entry on disk. The implementation of `i_alloc` can remain otherwise unchanged, because of the following invariant of our system: *no free nodes will ever be mutated in the cache*. In other words, if `i_alloc` encounters a free inode on disk, it knows that there should not be a non-free version of this inode in the cache. This allows the implementation of `i_alloc` to disregard the cache contents.
//! - `i_trunc`, `i_read` and `i_write` do not change substantially.
//!
//! At the end, write some tests that convincingly show that your implementation indeed supports cached inodes.
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
//! ...
//!

use std::borrow::{Borrow, BorrowMut};
use std::cell::{Ref, RefCell, RefMut};
use std::path::Path;
use std::rc::Rc;

use thiserror::Error;

use cplfs_api::controller::Device;
use cplfs_api::fs::{
    BlockSupport, FileSysSupport, InodeCacheSupport, InodeRWSupport, InodeSupport,
};
use cplfs_api::types::{
    Block, Buffer, DInode, FType, Inode, InodeLike, SuperBlock, DIRECT_POINTERS,
};

use crate::b_inode_support::InodeLevelError;
use crate::e_inode_RW_support::{InodeRWError, InodeRWFileSystem};

/// New Struct that wraps Inode into Rc<RefCell<Inode>>
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct InodeCached(Rc<RefCell<Inode>>);

/// Implemented two new methods to pass Ref of Inode to the lower layers, because implementation is clear if we only pass Inode to lower layers (InodeSupport and InodeRWSupport) when implementation stayed unchanged on this layer.
impl InodeCached {
    fn new(inode: Inode) -> InodeCached {
        return Self(Rc::new(RefCell::new(inode)));
    }
    /// return reference to Inode
    fn get_inode(&self) -> Ref<Inode> {
        (*self.0).borrow()
    }
    /// return mutable reference to Inode
    fn get_inode_mut(&self) -> RefMut<Inode> {
        (*self.0).borrow_mut()
    }
}

impl InodeLike for InodeCached {
    fn new(inum: u64, ft: &FType, nlink: u64, size: u64, blocks: &[u64]) -> Option<Self> {
        if nlink > u16::MAX as u64 {
            return None;
        }
        if blocks.len() > DIRECT_POINTERS as usize {
            return None;
        }

        let mut db = [0; DIRECT_POINTERS as usize];
        for i in 0..blocks.len() {
            db[i] = blocks[i];
        }

        let di = DInode {
            ft: *ft,
            nlink: nlink as u16,
            size,
            direct_blocks: db,
        };
        Some(InodeCached::new(Inode::new(inum, di)))
    }

    fn get_ft(&self) -> FType {
        (*self.0).borrow().disk_node.ft
    }
    fn get_nlink(&self) -> u64 {
        (*self.0).borrow().disk_node.nlink as u64
    }
    fn get_size(&self) -> u64 {
        (*self.0).borrow().disk_node.size
    }
    fn get_block(&self, i: u64) -> u64 {
        if DIRECT_POINTERS <= i {
            return 0;
        }
        (*self.0).borrow().disk_node.direct_blocks[i as usize]
    }

    fn get_inum(&self) -> u64 {
        (*self.0).borrow().inum
    }
}

/// This error can occurs during manipulating with Inode Cache File System
#[derive(Error, Debug)]
pub enum InodeCacheError {
    /// Error caused when performing inode cache operations.
    #[error("Inode cache error: {0}")]
    InodeCacheError(&'static str),
    /// Error caused when performing inode operations.
    #[error("Inode error: {0}")]
    InodeError(#[from] InodeLevelError),
    /// Error caused when performing inode read write operations.
    #[error("Inode error: {0}")]
    InodeRWError(#[from] InodeRWError),
}

/// `DirectoryFileSystem` wraps InodeFileSystem and add Directory support
pub struct InodeCacheFileSystem {
    /// wrapped Inodes support layer
    inode_fs: InodeRWFileSystem,
    /// cache is implemented as Vec because we can define capacity of it and we can emit particular CachedInode
    /// as first I implemented cahce as HashMap what seems to me as more effective solution in searching inodes by its inum, but there was problem with emitting elements.
    inode_cache: Vec<<Self as InodeSupport>::Inode>,
}

/// `InodeFileSystem` struct implements `FileSysSupport` and the `BlockSupport`. Structure wraps `Device` to offer block-level abstraction to operate with File System.
/// Implementation of FileSysSupport in BlockFileSystem
impl FileSysSupport for InodeCacheFileSystem {
    type Error = InodeCacheError;

    fn sb_valid(sb: &SuperBlock) -> bool {
        return InodeRWFileSystem::sb_valid(sb);
    }

    fn mkfs<P: AsRef<Path>>(path: P, sb: &SuperBlock) -> Result<Self, Self::Error> {
        // inode cache is here initialized with fixed capacity of 5
        return Ok(Self {
            inode_fs: InodeRWFileSystem::mkfs(path, sb)?,
            inode_cache: Vec::with_capacity(5),
        });
    }

    fn mountfs(dev: Device) -> Result<Self, Self::Error> {
        return Ok(Self {
            inode_fs: InodeRWFileSystem::mountfs(dev)?,
            inode_cache: Vec::with_capacity(5),
        });
    }

    fn unmountfs(self) -> Device {
        return self.inode_fs.unmountfs();
    }
}

/// Implementation of BlockSupport in BlockFileSystem
impl BlockSupport for InodeCacheFileSystem {
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

impl InodeSupport for InodeCacheFileSystem {
    type Inode = InodeCached;

    fn i_get(&self, i: u64) -> Result<Self::Inode, Self::Error> {
        // Get inode from cache if is cached otherwise return error
        return if self.is_cached(i) {
            let index = self.get_inode_cache_index(i)?;
            Ok(self.inode_cache[index as usize].clone())
        } else {
            Err(Self::Error::InodeCacheError("Inode is not in cache."))
        };
    }

    fn i_put(&mut self, ino: &Self::Inode) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.i_put(ino.get_inode().borrow())?);
    }

    fn i_free(&mut self, i: u64) -> Result<(), Self::Error> {
        // if inode is in cache then remove inode also from cache
        if self.is_cached(i) {
            let index = self.get_inode_cache_index(i)?;
            // Error if inode is still referenced
            if Rc::strong_count(&self.inode_cache[index as usize].0) > 1 {
                return Err(Self::Error::InodeCacheError(
                    "Inode to be free is still referenced.",
                ));
            }
            // Return Ok if inode is still linked
            if self.inode_cache[index as usize].get_nlink() > 0 {
                return Ok(());
            }
            // Remove inode from cache and free inode on disk
            self.inode_cache.remove(index as usize);
            Ok(self.inode_fs.i_free(i)?)
        } else {
            // if inode is not in cache only free inode on disk
            Ok(self.inode_fs.i_free(i)?)
        }
    }

    fn i_alloc(&mut self, ft: FType) -> Result<u64, Self::Error> {
        // first allocate new inode
        let i = self.inode_fs.i_alloc(ft)?;
        let inode = InodeCached::new(self.inode_fs.i_get(i)?);
        // it this inode is already cached then re-cache by newly allocated inode
        // otherwise only add inode to the cache
        if self.is_cached(i) {
            let index = self.get_inode_cache_index(i)?;
            self.inode_cache[index as usize] = inode;
        } else {
            self.add_inode_to_cache(inode)?;
        }
        return Ok(i);
    }

    fn i_trunc(&mut self, inode: &mut Self::Inode) -> Result<(), Self::Error> {
        return Ok(self.inode_fs.i_trunc(&mut inode.get_inode_mut())?);
    }
}

impl InodeRWSupport for InodeCacheFileSystem {
    fn i_read(
        &self,
        inode: &Self::Inode,
        buf: &mut Buffer,
        off: u64,
        n: u64,
    ) -> Result<u64, Self::Error> {
        Ok(self
            .inode_fs
            .i_read(inode.get_inode().borrow(), buf, off, n)?)
    }

    fn i_write(
        &mut self,
        inode: &mut Self::Inode,
        buf: &Buffer,
        off: u64,
        n: u64,
    ) -> Result<(), Self::Error> {
        Ok(self
            .inode_fs
            .i_write(inode.get_inode_mut().borrow_mut(), buf, off, n)?)
    }
}

impl InodeCacheSupport for InodeCacheFileSystem {
    fn i_get_mut(&mut self, i: u64) -> Result<Self::Inode, Self::Error> {
        // return inode if it is cached
        if self.is_cached(i) {
            return Ok(self.inode_cache[self.get_inode_cache_index(i)? as usize].clone());
        }
        // otherwise read inode from disk and cache inode
        let inode = InodeCached::new(self.inode_fs.i_get(i)?);
        let index = self.add_inode_to_cache(inode)?;
        return Ok(self.inode_cache[index as usize].clone());
    }

    fn is_cached(&self, inum: u64) -> bool {
        // iterate over all cached inodes and return true if any was found
        for inode in self.inode_cache.iter() {
            if inode.get_inum() == inum {
                return true;
            }
        }
        return false;
    }

    fn mkfs_cached<P: AsRef<Path>>(
        path: P,
        sb: &SuperBlock,
        nb_cache_entries: u64,
    ) -> Result<Self, Self::Error> {
        // initialize cache with fixed size of nb_cache_entries
        return Ok(Self {
            inode_fs: InodeRWFileSystem::mkfs(path, sb)?,
            inode_cache: Vec::with_capacity(nb_cache_entries as usize),
        });
    }

    fn mountfs_cached(dev: Device, nb_cache_entries: u64) -> Result<Self, Self::Error> {
        // initialize cache with fixed size of nb_cache_entries
        return Ok(Self {
            inode_fs: InodeRWFileSystem::mountfs(dev)?,
            inode_cache: Vec::with_capacity(nb_cache_entries as usize),
        });
    }
}

/// Trait implementing some helper function for inode cache in format of `Vec`
pub trait VecCacheHelper: InodeCacheSupport {
    /// Add inode to cache
    /// In case, cache is full iterate over already cached inodes and replace first without any reference
    /// If there is still place in cache, push Inode to Vec
    /// Return index to the new cached inode
    /// Error if there is no inode without references to it
    fn add_inode_to_cache(&mut self, inode: Self::Inode) -> Result<u64, Self::Error>;

    /// Iterate over all cahced inodes and return Vec index to this inode
    /// Error if inode is not cached
    fn get_inode_cache_index(&self, inum: u64) -> Result<u64, Self::Error>;
}

impl VecCacheHelper for InodeCacheFileSystem {
    fn add_inode_to_cache(&mut self, inode: Self::Inode) -> Result<u64, Self::Error> {
        // If capacity is full remove any element
        if self.inode_cache.len() >= self.inode_cache.capacity() {
            // iterate over all elements until anyone is without references
            for (i, cached_inode) in self.inode_cache.iter().enumerate() {
                if Rc::strong_count(&cached_inode.0) <= 1 {
                    let c = self.inode_cache.remove(i);
                    // save removed inode to the disk
                    self.i_put(c.borrow())?;
                    // and insert new inode on the same place
                    self.inode_cache.insert(i, inode);
                    return Ok(i as u64);
                }
            }
            return Err(Self::Error::InodeCacheError(
                "Not found free place in the cache",
            ));
        } else {
            // otherwise push inode to the cache
            self.inode_cache.push(inode);
            return Ok(self.inode_cache.len() as u64 - 1);
        }
    }

    fn get_inode_cache_index(&self, inum: u64) -> Result<u64, Self::Error> {
        // iterate over cached inodes until there is inode with same inum
        for (i, cached_inode) in self.inode_cache.iter().enumerate() {
            if cached_inode.get_inum() == inum {
                return Ok(i as u64);
            }
        }
        return Err(Self::Error::InodeCacheError(
            "Not found Inode with inum in the cache",
        ));
    }
}

/// You are free to choose the name for your file system. As we will use
/// automated tests when grading your assignment, indicate here the name of
/// your file system data type so we can just use `FSName` instead of
/// having to manually figure out the name.
pub type FSName = InodeCacheFileSystem;

#[cfg(test)]
#[path = "../../api/fs-tests"]
mod test_with_utils {
    use std::collections::HashMap;
    use std::rc::Rc;

    use cplfs_api::fs::{FileSysSupport, InodeCacheSupport, InodeSupport};
    use cplfs_api::types::{DInode, FType, Inode, InodeLike, SuperBlock};

    use crate::g_caching_inodes::{FSName, InodeCached};

    #[path = "utils.rs"]
    mod utils;

    static BLOCK_SIZE: u64 = 300;
    static NBLOCKS: u64 = 11;
    static BLOCK_SIZE_C: u64 = 1000; //make blocks somewhat smaller on this one, should still be sufficient for a reasonable inode
    static SUPERBLOCK_GOOD_C: SuperBlock = SuperBlock {
        block_size: BLOCK_SIZE_C,
        nblocks: NBLOCKS,
        ninodes: 10,
        inodestart: 1,
        ndatablocks: 6,
        bmapstart: 4,
        datastart: 5,
    };

    #[test]
    fn inode_cached_test() {
        let i1 = InodeCached::new(Inode::new(
            2,
            DInode {
                ft: FType::TFile,
                nlink: 0,
                size: (2.5 * (BLOCK_SIZE as f32)) as u64,
                direct_blocks: [5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            },
        ));
        assert_eq!(i1.get_size(), (2.5 * (BLOCK_SIZE as f32)) as u64);
        assert_eq!(i1.get_nlink(), 0);
        assert_eq!(i1.get_block(0), 5);
        assert_eq!(i1.get_inum(), 2);
        assert_eq!(Rc::strong_count(&i1.0), 1);
        // add another copy to the scope
        let i2 = i1.clone();
        assert_eq!(i2.get_size(), (2.5 * (BLOCK_SIZE as f32)) as u64);
        assert_eq!(i2.get_nlink(), 0);
        assert_eq!(Rc::strong_count(&i1.0), 2);
        {
            let _i3 = i1.clone();
            assert_eq!(Rc::strong_count(&i1.0), 3);
        }
        // i3 is now out of scope
        assert_eq!(Rc::strong_count(&i1.0), 2);
    }

    #[test]
    fn btree_test() {
        let mut map: HashMap<u64, &str> = HashMap::with_capacity(5);
        map.insert(1, "a");
        map.insert(2, "b");
        map.insert(3, "c");
        assert_eq!(map.contains_key(&1), true);
        assert_eq!(map.contains_key(&4), false);
        assert_eq!(map.get(&1), Some(&"a"));
        assert_eq!(map.capacity(), 7);
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn vec_test() {
        let mut map: Vec<&str> = Vec::with_capacity(5);
        map.insert(0, "a");
        map.insert(1, "b");
        map.insert(2, "c");
        assert_eq!(map.get(1), Some(&"b"));
        assert_eq!(map.capacity(), 5);
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn alloc_test() {
        let path = utils::disk_prep_path("alloc_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD_C).unwrap();

        for i in 0..5 {
            assert!(!my_fs.is_cached(i + 1));
            assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), i + 1);
            assert!(my_fs.is_cached(i + 1));
        }
        assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), 6);
        assert!(my_fs.is_cached(6));
        assert!(!my_fs.is_cached(1));
        // should be error
        assert!(my_fs.i_get(1).is_err());
        // get second cached inode
        let i2 = my_fs.i_get_mut(2).unwrap();
        assert_eq!(i2.get_ft(), FType::TDir);
        // inode 2 should be still in cache and inode 6 should be removed during allocation
        assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), 7);
        assert!(my_fs.is_cached(2));
        assert!(!my_fs.is_cached(6));
        // get inode which is not cached
        assert!(my_fs.i_get(1).is_err());
        assert!(my_fs.i_get_mut(1).is_ok());
        assert!(my_fs.is_cached(3));
        // inode 1 is cached so i_get should work same as i_get_mut
        assert_eq!(my_fs.i_get_mut(1).unwrap(), my_fs.i_get(1).unwrap());

        let dev = my_fs.unmountfs();
        utils::disk_destruct(dev);
    }

    #[test]
    fn put_get_test() {
        let path = utils::disk_prep_path("put_get_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD_C).unwrap();

        for i in 0..5 {
            assert!(!my_fs.is_cached(i + 1));
            assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), i + 1);
            assert!(my_fs.is_cached(i + 1));
        }

        // test if cached inode after removing is saved to disk
        let i1 = my_fs.i_get(1).unwrap();
        i1.0.borrow_mut().disk_node.nlink += 1;
        assert_eq!(i1.get_nlink(), 1);
        // cache new inode
        drop(i1);
        assert!(!my_fs.is_cached(6));
        assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), 6);
        assert!(my_fs.is_cached(6));
        assert!(!my_fs.is_cached(1));
        let i1 = my_fs.i_get_mut(1).unwrap();
        assert_eq!(i1.get_nlink(), 1);

        // test if i_put works
        i1.0.borrow_mut().disk_node.nlink += 1;
        my_fs.i_put(&i1).unwrap();
        assert_eq!(my_fs.inode_fs.i_get(1).unwrap().get_nlink(), 2); // testing by original Inode
        assert_eq!(my_fs.i_get(1).unwrap().get_nlink(), 2);
    }

    #[test]
    fn free_test() {
        let path = utils::disk_prep_path("free_test", "img");
        let mut my_fs = FSName::mkfs(&path, &SUPERBLOCK_GOOD_C).unwrap();

        for i in 0..3 {
            assert!(!my_fs.is_cached(i + 1));
            assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), i + 1);
            assert!(my_fs.is_cached(i + 1));
        }

        // test to free cached inode
        my_fs.i_free(1).unwrap();
        assert!(my_fs.i_free(1).is_err()); // error if already free
        assert!(!my_fs.is_cached(1)); // inode is not more cached
        assert_eq!(my_fs.i_alloc(FType::TDir).unwrap(), 1); // again allocated

        // test to free still referenced inode
        let _i1 = my_fs.i_get(1).unwrap();
        assert!(my_fs.i_free(1).is_err());

        // test to free not cached inode
        assert_eq!(my_fs.inode_fs.i_alloc(FType::TDir).unwrap(), 4); // allocate without caching
        assert!(!my_fs.is_cached(4));
        my_fs.i_free(4).unwrap();
        assert_eq!(my_fs.i_get_mut(4).unwrap().get_ft(), FType::TFree)
    }
}

// WARNING: DO NOT TOUCH THE BELOW CODE -- IT IS REQUIRED FOR TESTING -- YOU WILL LOSE POINTS IF I MANUALLY HAVE TO FIX YOUR TESTS
#[cfg(all(test, any(feature = "g", feature = "all")))]
#[path = "../../api/fs-tests/g_test.rs"]
mod tests;
