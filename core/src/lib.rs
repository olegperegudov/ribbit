//! Ribbit's speech pipeline without the app around it: provider stacks with
//! failover, speech-to-text, hallucination cleanup, the LLM editor and the
//! vocabulary. `pipeline::run` is the one entry point both the desktop app
//! and `ribbit-server` call.

pub mod debug_log;
pub mod fallback;
pub mod hallucinations;
pub mod pipeline;
pub mod postprocess;
pub mod private;
pub mod transcribe;
pub mod vocab;
