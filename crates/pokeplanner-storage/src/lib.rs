pub mod json_store;
pub mod traits;
#[cfg(test)]
mod tests;

pub use json_store::JsonFileStorage;
pub use traits::Storage;
