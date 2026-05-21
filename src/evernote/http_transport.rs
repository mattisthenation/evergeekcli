//! Thrift-over-HTTP transport: buffers written bytes and POSTs them to the
//! Evernote endpoint on `flush()`, loading the response for subsequent reads.
//!
//! Implemented in Task 13.
