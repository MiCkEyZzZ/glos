pub mod binary;
pub mod error;
pub mod format;
pub mod serialization;

pub use binary::*;
pub use error::*;
pub use format::*;
pub use serialization::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lybrary_exports() {
        assert_eq!(GLOS_VERSION, 1);
        assert_eq!(GLOS_HEADER_SIZE, 128);
    }
}
