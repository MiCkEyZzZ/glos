//! Спецификация формата файлов ГЛОС версия 1.0
//!
//! Бинарное представление .glos файлов, содержащих IQ данные и метаданные.
//! Все многобайтовые числа хранятся в порядке big-endian (сетевая
//! последовательность).

use crc32fast::Hasher;
use glos_types::{Compression, GlosError, GlosHeader, GlosResult, IqBlock, IqFormat, SdrType};

use crate::{read_u32_local, read_u64_local, write_u32_local, write_u64_local};

/// Магическое число для идентификации GLOS файлов: b"GLOS"
pub const GLOS_MAGIC: [u8; 4] = [b'G', b'L', b'O', b'S'];

/// Текущая версия формата
pub const GLOS_VERSION: u8 = 1;

/// Размер фиксированного заголовка (128 байт)
pub const GLOS_HEADER_SIZE: usize = 128;

/// Минимальный размер блока IQ данных
pub const GLOS_MIN_BLOCK_SIZE: usize = 32;

/// Максимальный размер блока IQ данных (1 МБ)
pub const GLOS_MAX_BLOCK_SIZE: usize = 1024 * 1024;

pub trait GlosHeaderExt {
    /// Создание нового заголовка с настройками по умолчанию.
    fn new(
        sdr_type: SdrType,
        sample_rate: u32,
        center_freq: u64,
    ) -> Self
    where
        Self: Sized;
    /// Сериализация заголовка в 128 байт
    fn serialize(&self) -> GlosResult<[u8; GLOS_HEADER_SIZE]>;
    /// Десериализация заголовка из 128 байт
    fn deserialize(buf: &[u8; GLOS_HEADER_SIZE]) -> GlosResult<Self>
    where
        Self: Sized;
    fn is_little_endian(&self) -> bool;
}

pub trait IqBlockExt {
    /// Создаёт новый блок IQ данными.
    fn new(
        timestamp_ns: u64,
        sample_count: u32,
        data: Vec<u8>,
    ) -> Self
    where
        Self: Sized;
    /// Создаёт блок с предварительно сжатыми данными.
    fn new_compressed(
        timestamp_ns: u64,
        sample_count: u32,
        compressed_data: Vec<u8>,
    ) -> Self
    where
        Self: Sized;
    /// Сжимает данные блока с помощью LZ4.
    fn compress(&mut self) -> GlosResult<()>;
    /// Распаковать данные блока (если сжаты)
    fn decompress(&mut self) -> GlosResult<()>;
    /// Проверяет соответствие `sample_count * iq_format.sample_size() ==
    /// data.len()`.
    fn validate_sample_count(
        &self,
        iq_format: IqFormat,
    ) -> GlosResult<()>;
    /// Сериализует блок в байты с CRC.
    fn serialize(&self) -> GlosResult<Vec<u8>>;
    /// Десериализует блок из ьайтового среза.
    fn deserialize(
        buf: &[u8],
        compression: Compression,
    ) -> GlosResult<(Self, usize)>
    where
        Self: Sized;
    /// Возвращает несжатые данные (автоматически распаковывает если нужно).
    fn get_uncompressed_data(&self) -> GlosResult<Vec<u8>>;
}

impl GlosHeaderExt for GlosHeader {
    fn new(
        sdr_type: SdrType,
        sample_rate: u32,
        center_freq: u64,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        GlosHeader {
            version: GLOS_VERSION,
            flags: 0,
            sdr_type,
            iq_format: IqFormat::Int16,
            compression: Compression::None,
            sample_rate,
            center_freq,
            gain_db: 0.0,
            timestamp_start: now,
            timestamp_end: 0,
            total_samples: 0,
        }
    }

    fn serialize(&self) -> GlosResult<[u8; GLOS_HEADER_SIZE]> {
        let mut buf = [0u8; GLOS_HEADER_SIZE];
        let mut off = 0;

        buf[off..off + 4].copy_from_slice(&GLOS_MAGIC);
        off += 4;

        buf[off] = self.version;
        off += 1;

        buf[off] = self.flags;
        off += 1;

        off += 6; // padding

        buf[off] = self.sdr_type.as_u8();
        off += 1;

        buf[off] = self.iq_format.as_u8();
        off += 1;

        buf[off] = self.compression.as_u8();
        off += 1;

        off += 1; // padding

        let is_le = (self.flags & 0x01) != 0;

        // вызовы (заменяют write_u32!(...) / write_u64!(...))
        write_u32_local(&mut buf, &mut off, is_le, self.sample_rate);
        write_u64_local(&mut buf, &mut off, is_le, self.center_freq);
        write_u32_local(&mut buf, &mut off, is_le, self.gain_db.to_bits());
        write_u64_local(&mut buf, &mut off, is_le, self.timestamp_start);
        write_u64_local(&mut buf, &mut off, is_le, self.timestamp_end);
        write_u64_local(&mut buf, &mut off, is_le, self.total_samples);

        // CRC32 всегда big-endian, покрывает [0..72)
        let crc = crc32_checksum(&buf[0..72]);
        buf[72..76].copy_from_slice(&crc.to_be_bytes());

        // [76..128] — reserved, уже нули
        Ok(buf)
    }

    fn deserialize(buf: &[u8; GLOS_HEADER_SIZE]) -> GlosResult<Self> {
        let mut off = 0;

        if buf[off..off + 4] != GLOS_MAGIC {
            return Err(GlosError::invalid_magic("Invalid GLOS magic number"));
        }
        off += 4;

        let version = buf[off];
        if version != GLOS_VERSION {
            return Err(GlosError::UnsupportedVersion {
                found: version,
                expected: GLOS_VERSION,
            });
        }
        off += 1;

        let flags = buf[off];
        let is_le = (flags & 0x01) != 0;
        off += 1;

        off += 6; // padding

        let sdr_type = SdrType::from_u8(buf[off]);
        off += 1;

        let iq_format = IqFormat::from_u8(buf[off])?;
        off += 1;

        let compression = Compression::from_u8(buf[off])?;
        off += 1;

        off += 1; // padding

        // вызовы (заменяют let sample_rate = read_u32!(); и т.д.)
        let sample_rate = read_u32_local(buf, &mut off, is_le);
        let center_freq = read_u64_local(buf, &mut off, is_le);
        let gain_db = f32::from_bits(read_u32_local(buf, &mut off, is_le));
        let timestamp_start = read_u64_local(buf, &mut off, is_le);
        let timestamp_end = read_u64_local(buf, &mut off, is_le);
        let total_samples = read_u64_local(buf, &mut off, is_le);

        // CRC всегда big-endian
        let stored_crc = u32::from_be_bytes([buf[72], buf[73], buf[74], buf[75]]);
        let calculated_crc = crc32_checksum(&buf[0..72]);
        if stored_crc != calculated_crc {
            return Err(GlosError::CrcMismatch {
                expected: calculated_crc,
                found: stored_crc,
            });
        }

        Ok(GlosHeader {
            version,
            flags,
            sdr_type,
            iq_format,
            compression,
            sample_rate,
            center_freq,
            gain_db,
            timestamp_start,
            timestamp_end,
            total_samples,
        })
    }

    fn is_little_endian(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

impl IqBlockExt for IqBlock {
    fn new(
        timestamp_ns: u64,
        sample_count: u32,
        data: Vec<u8>,
    ) -> Self {
        IqBlock {
            timestamp_ns,
            sample_count,
            data,
            is_compressed: false,
        }
    }

    fn new_compressed(
        timestamp_ns: u64,
        sample_count: u32,
        compressed_data: Vec<u8>,
    ) -> Self {
        IqBlock {
            timestamp_ns,
            sample_count,
            data: compressed_data,
            is_compressed: true,
        }
    }

    fn compress(&mut self) -> GlosResult<()> {
        if self.is_compressed {
            return Ok(());
        }

        self.data = lz4_flex::compress_prepend_size(&self.data);
        self.is_compressed = true;

        Ok(())
    }

    fn decompress(&mut self) -> GlosResult<()> {
        if !self.is_compressed {
            return Ok(()); // Не сжато
        }

        let decompressed = lz4_flex::decompress_size_prepended(&self.data)
            .map_err(|e| GlosError::Corrupted(format!("LZ4 decompression failed: {e}")))?;

        self.data = decompressed;
        self.is_compressed = false;

        Ok(())
    }

    fn validate_sample_count(
        &self,
        iq_format: IqFormat,
    ) -> GlosResult<()> {
        if self.is_compressed {
            return Ok(());
        }

        let expected = self.sample_count as usize * iq_format.sample_size();

        if self.data.len() != expected {
            return Err(GlosError::FormatViolation(format!(
                "sample_count={} × sample_size={} = {} ≠ data.len()={}",
                self.sample_count,
                iq_format.sample_size(),
                expected,
                self.data.len(),
            )));
        }

        Ok(())
    }

    fn serialize(&self) -> GlosResult<Vec<u8>> {
        let block_size = 4 + 4 + 8 + self.data.len() + 4; // size+count+ts+data+crc

        if block_size > GLOS_MAX_BLOCK_SIZE {
            return Err(GlosError::InvalidBlockSize(block_size));
        }

        let mut buf = Vec::with_capacity(block_size);
        let content_size = (4 + 8 + self.data.len()) as u32;

        buf.extend_from_slice(&content_size.to_be_bytes());
        buf.extend_from_slice(&self.sample_count.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_ns.to_be_bytes());
        buf.extend_from_slice(&self.data);

        let crc = crc32_checksum(&buf[4..]); // CRC покрывает [4..end-4]

        buf.extend_from_slice(&crc.to_be_bytes());

        Ok(buf)
    }

    fn deserialize(
        buf: &[u8],
        compression: Compression,
    ) -> GlosResult<(Self, usize)> {
        if buf.len() < 20 {
            return Err(GlosError::corrupted("Block too small"));
        }

        // Размер содержимого блока
        let content_size = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

        if 4 + content_size + 4 > buf.len() {
            return Err(GlosError::corrupted("Incomplete block"));
        }

        let sample_count = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);

        // Время блока
        let timestamp_ns = u64::from_be_bytes([
            buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
        ]);

        // IQ данные
        let data_len = content_size
            .checked_sub(12)
            .ok_or_else(|| GlosError::corrupted("Invalid content_size"))?;
        let data = buf[16..16 + data_len].to_vec();

        // CRC32 покрывает байты [4..4 + content_size]
        let stored_crc = u32::from_be_bytes([
            buf[4 + content_size],
            buf[4 + content_size + 1],
            buf[4 + content_size + 2],
            buf[4 + content_size + 3],
        ]);
        let calculated_crc = crc32_checksum(&buf[4..4 + content_size]);

        if stored_crc != calculated_crc {
            return Err(GlosError::CrcMismatch {
                expected: calculated_crc,
                found: stored_crc,
            });
        }

        // is_compressed определяется из заголовка файла, а не эвристикой
        let is_compressed = compression == Compression::Lz4;

        let total_bytes = 4 + content_size + 4;

        Ok((
            IqBlock {
                timestamp_ns,
                sample_count,
                data,
                is_compressed,
            },
            total_bytes,
        ))
    }

    fn get_uncompressed_data(&self) -> GlosResult<Vec<u8>> {
        if self.is_compressed {
            lz4_flex::decompress_size_prepended(&self.data)
                .map_err(|e| GlosError::Corrupted(format!("LZ4 decompression failed: {e}")))
        } else {
            Ok(self.data.clone())
        }
    }
}

/// CRC32 (IEEE 802.3 / crc32fast)
pub fn crc32_checksum(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use glos_types::{GlosHeader, IqBlock};

    use super::*;

    #[test]
    fn test_header_round_trip() {
        let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);

        header.gain_db = 40.0;
        header.total_samples = 1_000_000;

        let serialized = header.serialize().unwrap();

        assert_eq!(std::mem::size_of_val(&serialized), GLOS_HEADER_SIZE);

        let deserialized = GlosHeader::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.sdr_type, SdrType::HackRf);
        assert_eq!(deserialized.sample_rate, 2_000_000);
        assert_eq!(deserialized.center_freq, 1_602_000_000);
        assert_eq!(deserialized.gain_db, 40.0);
        assert_eq!(deserialized.total_samples, 1_000_000);
    }

    #[test]
    fn test_header_little_endian() {
        let mut header = GlosHeader::new(SdrType::PlutoSdr, 10_000_000, 1_575_000_000);

        header.flags = 0x01;
        header.gain_db = 35.5;

        let serialized = header.serialize().unwrap();
        let deserialized = GlosHeader::deserialize(&serialized).unwrap();

        assert!(deserialized.is_little_endian());
        assert_eq!(deserialized.sample_rate, 10_000_000);
        assert_eq!(deserialized.center_freq, 1_575_000_000);
        assert_eq!(deserialized.gain_db, 35.5);
    }

    #[test]
    fn test_header_corrupted_crc() {
        let header = GlosHeader::new(SdrType::PlutoSdr, 10_000_000, 1_575_000_000);
        let mut serialized = header.serialize().unwrap();

        serialized[72] ^= 0xFF;

        let result = GlosHeader::deserialize(&serialized);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CRC"));
    }

    #[test]
    fn test_header_byte_layout() {
        let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
        header.gain_db = 40.0;
        header.iq_format = IqFormat::Int16;
        header.compression = Compression::None;
        header.timestamp_start = 1_704_067_200;
        header.timestamp_end = 0;
        header.total_samples = 0;

        let bytes = header.serialize().unwrap();

        assert_eq!(&bytes[0..4], b"GLOS", "magic");
        assert_eq!(bytes[4], 1, "version");
        assert_eq!(bytes[5], 0, "flags = big-endian");
        assert_eq!(bytes[12], 0, "sdr_type = HackRf");
        assert_eq!(bytes[13], 1, "iq_format = Int16");
        assert_eq!(bytes[14], 0, "compression = None");
        // sample_rate = 2_000_000 = 0x001E8480
        assert_eq!(&bytes[16..20], &[0x00, 0x1E, 0x84, 0x80], "sample_rate BE");
        // Перепарсируем чтобы проверить остальные поля
        let reparsed = GlosHeader::deserialize(&bytes).unwrap();
        assert_eq!(reparsed.gain_db, 40.0);
        assert_eq!(reparsed.timestamp_start, 1_704_067_200);
    }

    #[test]
    fn test_iq_block_round_trip_int8() {
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8]; // 4 Int8 IQ пары
        let block = IqBlock::new(123_456_789, 4, data.clone());

        let serialized = block.serialize().unwrap();
        let (deserialized, bytes_read) =
            IqBlock::deserialize(&serialized, Compression::None).unwrap();

        assert_eq!(deserialized.timestamp_ns, 123_456_789);
        assert_eq!(deserialized.sample_count, 4);
        assert_eq!(deserialized.data, data);
        assert!(!deserialized.is_compressed);
        assert_eq!(bytes_read, serialized.len());

        // Валидация: 4 выборки × 2 байта = 8 байт
        deserialized.validate_sample_count(IqFormat::Int8).unwrap();
    }

    #[test]
    fn test_iq_block_round_trip_int16() {
        // 5 Int16 IQ пар = 20 байт
        let data = vec![0u8; 20];
        let block = IqBlock::new(0, 5, data.clone());

        let serialized = block.serialize().unwrap();
        let (deserialized, _) = IqBlock::deserialize(&serialized, Compression::None).unwrap();

        assert_eq!(deserialized.sample_count, 5);
        deserialized.validate_sample_count(IqFormat::Int16).unwrap();
    }

    #[test]
    fn test_iq_block_corrupted_crc() {
        let data = vec![1u8, 2, 3, 4];
        let block = IqBlock::new(1_234_567_890, 2, data);
        let mut serialized = block.serialize().unwrap();
        serialized[20] ^= 0xFF;

        let result = IqBlock::deserialize(&serialized, Compression::None);
        assert!(result.is_err());
    }

    #[test]
    fn test_iq_block_is_compressed_from_header() {
        // Compression::Lz4 → is_compressed = true без эвристики
        let data = vec![42u8; 100];
        let block = IqBlock::new(0, 50, data);
        let serialized = block.serialize().unwrap();

        let (parsed_none, _) = IqBlock::deserialize(&serialized, Compression::None).unwrap();
        assert!(!parsed_none.is_compressed, "должен быть false для None");

        let (parsed_lz4, _) = IqBlock::deserialize(&serialized, Compression::Lz4).unwrap();
        assert!(parsed_lz4.is_compressed, "должен быть true для Lz4");
    }

    #[test]
    fn test_validate_sample_count() {
        // Верный случай: Int16 × 10 = 40 байт
        let ok = IqBlock::new(0, 10, vec![0u8; 40]);
        ok.validate_sample_count(IqFormat::Int16).unwrap();

        // Ошибка: 10 выборок × 4 ≠ 30 байт
        let bad = IqBlock::new(0, 10, vec![0u8; 30]);
        assert!(bad.validate_sample_count(IqFormat::Int16).is_err());

        // Сжатый блок — пропускает проверку
        let compressed = IqBlock {
            is_compressed: true,
            ..IqBlock::new(0, 10, vec![0u8; 5])
        };
        compressed.validate_sample_count(IqFormat::Int16).unwrap();
    }

    #[test]
    fn test_compression_lz4_round_trip() {
        let data = vec![42u8; 10_000]; // повторяющиеся данные
        let mut block = IqBlock::new(123_456_789, 2_500, data.clone());

        let original_size = block.data.len();
        block.compress().unwrap();
        assert!(
            block.data.len() < original_size,
            "LZ4 должен уменьшить размер"
        );
        assert!(block.is_compressed);

        block.decompress().unwrap();
        assert_eq!(block.data, data);
        assert!(!block.is_compressed);
    }

    #[test]
    fn test_compression_idempotent() {
        let data = vec![0u8; 1000];
        let mut block = IqBlock::new(0, 500, data.clone());

        block.compress().unwrap();
        let size1 = block.data.len();
        block.compress().unwrap(); // no-op
        assert_eq!(block.data.len(), size1);

        block.decompress().unwrap();
        block.decompress().unwrap(); // no-op
        assert_eq!(block.data, data);
    }

    #[test]
    fn test_iq_format_sizes() {
        assert_eq!(IqFormat::Int8.sample_size(), 2);
        assert_eq!(IqFormat::Int16.sample_size(), 4);
        assert_eq!(IqFormat::Float32.sample_size(), 8);
    }

    #[test]
    fn test_iq_block_with_compression_serialize() {
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut block = IqBlock::new(999, 4, data.clone());
        block.compress().unwrap();

        let serialized = block.serialize().unwrap();
        let (deserialized, _) = IqBlock::deserialize(&serialized, Compression::Lz4).unwrap();
        assert!(deserialized.is_compressed);

        let uncompressed = deserialized.get_uncompressed_data().unwrap();
        assert_eq!(uncompressed, data);
    }
}
