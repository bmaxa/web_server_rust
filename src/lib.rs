#![feature(rustc_private)]
#![feature(arbitrary_self_types)]
#![feature(slice_patterns)]
//#![feature(step_by)]
#![feature(iterator_step_by)]
#![feature(libc)]
#![feature(rust_2018_preview)]
#![feature(try_blocks)]
#![crate_type = "lib"]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#[link(name="atomic")]
extern crate libc;
#[macro_use]
extern crate downcast_rs;
#[macro_use]
extern crate text_io;

pub mod sockets;
pub mod service;
pub mod server;
pub mod threads;
pub mod http_message;
#[macro_use]
mod macros;
mod utils;
mod web_server;

