//! The local language model worker. The `hushpen-llm` binary links llama.cpp; the app never does
//! (it talks to this process over the pipe protocol in `hushpen-core`).

pub mod backend;
pub mod entry;
pub mod llama;
pub mod server;
