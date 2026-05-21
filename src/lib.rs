//! evergeekcli — a library for searching and summarizing Evernote notes.
//!
//! The binary (`evergeek`) is a thin shell over this library. The
//! [`evernote::EvernoteClient`] trait is the single boundary to the Evernote
//! API; nothing outside the `evernote` module touches Thrift/EDAM types.

pub mod auth;
pub mod cli;
pub mod commands;
pub mod config;
pub mod edam;
pub mod enml;
pub mod error;
pub mod evernote;
pub mod model;
pub mod output;
