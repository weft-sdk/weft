//! What `weft init` knows: which transports and libraries exist for a language, how to recognise a project, and
//! the text of the files it writes. Asking the user and writing the files is the CLI's job.

mod catalog;
mod detect;
mod files;

pub use catalog::{library, transport, transports, Library, Transport, TransportKind};
pub use detect::{detect, Detected, Language, PackageManager};
pub use files::{
    example_contract, sanitize_service_name, validate_service_name, weft_yaml, Choices,
};
