// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// TODO: std::error::Error::description is deprecated now, resolve it later.
#![allow(deprecated)]

quick_error! {
    /// The base error type for raft
    #[derive(Debug)]
    pub enum Error {
        /// An IO error occurred
        Io(err: std::io::Error) {
            from()
            cause(err)
            description(err.description())
        }
        /// A storage error occurred.
        Store(err: StorageError) {
            from()
            cause(err)
            description(err.description())
        }
        /// Raft cannot step the local message.
        StepLocalMsg {
            description("raft: cannot step raft local message")
        }
        /// The raft peer is not found and thus cannot step.
        StepPeerNotFound {
            description("raft: cannot step as peer not found")
        }
        /// The proposal of changes was dropped.
        ProposalDropped {
            description("raft: proposal dropped")
        }
        /// The configuration is invalid.
        ConfigInvalid(desc: String) {
            description(desc)
        }
        /// A protobuf message codec failed in some manner.
        CodecError(err: protobuf::ProtobufError) {
            from()
            cause(err)
            description(err.description())
            display("protobuf codec error {:?}", err)
        }
        /// The node exists, but should not.
        Exists(id: u64, set: &'static str) {
            display("The node {} already exists in the {} set.", id, set)
        }
        /// The node does not exist, but should.
        NotExists(id: u64, set: &'static str) {
            display("The node {} is not in the {} set.", id, set)
        }
        /// The request snapshot is dropped.
        RequestSnapshotDropped {
            description("raft: request snapshot dropped")
        }
    }
}

impl PartialEq for Error {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::match_same_arms))]
    fn eq(&self, other: &Error) -> bool {
        match (self, other) {
            (&Error::StepPeerNotFound, &Error::StepPeerNotFound) => true,
            (&Error::ProposalDropped, &Error::ProposalDropped) => true,
            (&Error::Store(ref e1), &Error::Store(ref e2)) => e1 == e2,
            (&Error::Io(ref e1), &Error::Io(ref e2)) => e1.kind() == e2.kind(),
            (&Error::StepLocalMsg, &Error::StepLocalMsg) => true,
            (&Error::ConfigInvalid(ref e1), &Error::ConfigInvalid(ref e2)) => e1 == e2,
            (&Error::RequestSnapshotDropped, &Error::RequestSnapshotDropped) => true,
            _ => false,
        }
    }
}

quick_error! {
    /// An error with the storage.
    #[derive(Debug)]
    pub enum StorageError {
        /// The storage was compacted and not accessible
        Compacted {
            description("log compacted")
        }
        /// The log is not available.
        Unavailable {
            description("log unavailable")
        }
        /// The snapshot is out of date.
        SnapshotOutOfDate {
            description("snapshot out of date")
        }
        /// The snapshot is being created.
        SnapshotTemporarilyUnavailable {
            description("snapshot is temporarily unavailable")
        }
    }
}

impl PartialEq for StorageError {
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::match_same_arms))]
    fn eq(&self, other: &StorageError) -> bool {
        match (self, other) {
            (&StorageError::Compacted, &StorageError::Compacted) => true,
            (&StorageError::Unavailable, &StorageError::Unavailable) => true,
            (&StorageError::SnapshotOutOfDate, &StorageError::SnapshotOutOfDate) => true,
            (
                &StorageError::SnapshotTemporarilyUnavailable,
                &StorageError::SnapshotTemporarilyUnavailable,
            ) => true,
            _ => false,
        }
    }
}

/// A result type that wraps up the raft errors.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_equal() {
        assert_eq!(Error::StepPeerNotFound, Error::StepPeerNotFound);
        assert_eq!(
            Error::Store(StorageError::Compacted),
            Error::Store(StorageError::Compacted)
        );
        assert_eq!(
            Error::Io(io::Error::new(io::ErrorKind::UnexpectedEof, "oh no!")),
            Error::Io(io::Error::new(io::ErrorKind::UnexpectedEof, "oh yes!"))
        );
        assert_ne!(
            Error::Io(io::Error::new(io::ErrorKind::NotFound, "error")),
            Error::Io(io::Error::new(io::ErrorKind::BrokenPipe, "error"))
        );
        assert_eq!(Error::StepLocalMsg, Error::StepLocalMsg);
        assert_eq!(
            Error::ConfigInvalid(String::from("config error")),
            Error::ConfigInvalid(String::from("config error"))
        );
        assert_ne!(
            Error::ConfigInvalid(String::from("config error")),
            Error::ConfigInvalid(String::from("other error"))
        );
        assert_eq!(
            Error::from(io::Error::new(io::ErrorKind::Other, "oh no!")),
            Error::from(io::Error::new(io::ErrorKind::Other, "oh yes!"))
        );
        assert_ne!(
            Error::StepPeerNotFound,
            Error::Store(StorageError::Compacted)
        );
    }

    #[test]
    fn test_storage_error_equal() {
        assert_eq!(StorageError::Compacted, StorageError::Compacted);
        assert_eq!(StorageError::Unavailable, StorageError::Unavailable);
        assert_eq!(
            StorageError::SnapshotOutOfDate,
            StorageError::SnapshotOutOfDate
        );
        assert_eq!(
            StorageError::SnapshotTemporarilyUnavailable,
            StorageError::SnapshotTemporarilyUnavailable
        );
        assert_ne!(StorageError::Compacted, StorageError::Unavailable);
    }
}
