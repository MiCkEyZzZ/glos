//! Библиотека основного формата GLOS
//!
//! Эталонная реализация формата файлов GLOS для хранения данных IQ-сигналов
//! GNSS.
//!
//! # Быстрый старт
//!
//! ```no_run
//! use glos_types::{GlosHeader, IqBlock, SdrType};
//! use glos_core::{GlosHeaderExt, IqBlockExt};
//! use std::fs::File;
//! use std::io::Write;
//!
//! let mut file = File::create("signal.glos")?;
//! let header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
//! file.write_all(&header.serialize()?)?;
//!
//! let block = IqBlock::new(0, 1000, vec![0; 4000]);
//! file.write_all(&block.serialize()?)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ``

pub mod binary;
pub mod format;
pub mod replayer;
pub mod serialization;

pub use binary::*;
pub use format::*;
pub use replayer::*;
pub use serialization::*;

/// Версия библиотеки.
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
