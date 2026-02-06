//! The main library for the chess engine.
//!
//! This crate defines the core data structures and logic for representing
//! and manipulating a chess board.

pub mod board;
pub mod eval;
pub mod movegen;
pub mod perft;
pub mod search;
pub mod tt;
pub mod uci;