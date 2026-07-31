// Native-memory plumbing, grouped by the runtime layer each targets:
pub mod v8; // GC accounting
pub mod glibc; // heap restoration
