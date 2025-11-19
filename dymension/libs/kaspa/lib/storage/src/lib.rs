mod rocksdb_impl;
mod traits;

pub use rocksdb_impl::BridgeRocksDB;
pub use traits::{BridgeStorage, StorageError};

pub type StorageResult<T> = Result<T, StorageError>;
