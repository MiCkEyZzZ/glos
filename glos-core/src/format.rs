//! Спецификация формата файлов ГЛОС версия 1.0
//!
//! Бинарное представление .glos файлов, содержащих IQ данные и метаданные.
//! Все многобайтовые числа хранятся в порядке big-endian (сетевая
//! последовательность).

use crc32fast::Hasher;

use crate::error::{GlosError, GlosResult};

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

/// Тип SDR устройства
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SdrType {
    /// Hack RF One
    HackRf = 0,
    /// ADALM-PlutoSDR
    PlutoSdr = 1,
    /// USRP B200 family
    UsrpB200 = 2,
    /// Unknown device
    Unknown = 255,
}

/// Формат IQ выборок
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IqFormat {
    /// 8-битные целые числа (I8, Q8) — компактно
    Int8 = 0,
    /// 16-битные целые числа (I16, Q16) — выше точность
    Int16 = 1,
    /// 32-битные числа с плавающей точкой (F32, F32) — полная точность
    Float32 = 2,
}

/// Тип сжатия IQ данных
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    /// Без сжатия
    None = 0,
    /// Сжатие LZ4
    Lz4 = 1,
}

/// Заголовок GLOS файла (фиксированный размер 128 байт)
#[derive(Debug, Clone)]
pub struct GlosHeader {
    /// Версия формата ГЛОС
    pub version: u8,
    /// Флаги (bit 0: little-endian если установлен)
    pub flags: u8,
    /// Тип SDR устройства
    pub sdr_type: SdrType,
    /// Формат IQ данных
    pub iq_format: IqFormat,
    /// Метод сжатия
    pub compression: Compression,
    /// Частота дискретизации в Гц
    pub sample_rate: u32,
    /// Несущая частота в Гц
    pub center_freq: u64,
    /// Усиление приёмника в дБ (f32)
    pub gain_db: f32,
    /// Время начала сессии (Unix timestamp, секунды)
    pub timestamp_start: u64,
    /// Время окончания сессии (0 если запись продолжается)
    pub timestamp_end: u64,
    /// Общее количество IQ выборок в файле
    pub total_samples: u64,
}

/// Блок IQ данных (переменный размер)
#[derive(Debug, Clone)]
pub struct IqBlock {
    /// Метка времени блока в наносекундах (для точности)
    pub timestamp_ns: u64,
    /// Количество IQ выборок в блоке
    pub sample_count: u32,
    /// Данные IQ выборок (формат зависит от заголовка)
    pub data: Vec<u8>,
    /// Флаг: данные в `data` находятся в сжатом виде
    pub is_compressed: bool,
}

impl SdrType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => SdrType::HackRf,
            1 => SdrType::PlutoSdr,
            2 => SdrType::UsrpB200,
            _ => SdrType::Unknown,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl IqFormat {
    pub fn from_u8(v: u8) -> GlosResult<Self> {
        match v {
            0 => Ok(IqFormat::Int8),
            1 => Ok(IqFormat::Int16),
            2 => Ok(IqFormat::Float32),
            _ => Err(GlosError::FormatViolation(format!(
                "Unknown IQ format: {v}"
            ))),
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Размер одной IQ пары в байтах
    pub fn sample_size(&self) -> usize {
        match self {
            IqFormat::Int8 => 2,    // 1 байт I + 1 байт Q
            IqFormat::Int16 => 4,   // 2 байта I + 2 байта Q
            IqFormat::Float32 => 8, // 4 байта I + 4 байта Q
        }
    }
}

impl Compression {
    pub fn from_u8(v: u8) -> GlosResult<Self> {
        match v {
            0 => Ok(Compression::None),
            1 => Ok(Compression::Lz4),
            _ => Err(GlosError::FormatViolation(format!(
                "Unknown compression: {v}"
            ))),
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

impl GlosHeader {
    /// Creating a new header with default settings.
    pub fn new(
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

    /// Сериализация заголовка в 128 байт
    pub fn serialize(&self) -> GlosResult<[u8; GLOS_HEADER_SIZE]> {
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

    /// Десериализация заголовка из 128 байт
    pub fn deserialize(buf: &[u8; GLOS_HEADER_SIZE]) -> GlosResult<Self> {
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

    pub fn is_little_endian(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

impl IqBlock {
    /// Создаёт новый блок IQ данными.
    pub fn new(
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

    /// Создаёт блок с предварительно сжатыми данными.
    pub fn new_compressed(
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

    /// Сжимает данные блока с помощью LZ4.
    pub fn compress(&mut self) -> GlosResult<()> {
        if self.is_compressed {
            return Ok(());
        }

        self.data = lz4_flex::compress_prepend_size(&self.data);
        self.is_compressed = true;

        Ok(())
    }

    /// Распаковать данные блока (если сжаты)
    pub fn decompress(&mut self) -> GlosResult<()> {
        if !self.is_compressed {
            return Ok(()); // Не сжато
        }

        let decompressed = lz4_flex::decompress_size_prepended(&self.data)
            .map_err(|e| GlosError::Corrupted(format!("LZ4 decompression failed: {e}")))?;

        self.data = decompressed;
        self.is_compressed = false;

        Ok(())
    }

    /// Проверяет соответствие `sample_count * iq_format.sample_size() ==
    /// data.len()`.
    pub fn validate_sample_count(
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

    /// Сериализует блок в байты с CRC.
    pub fn serialize(&self) -> GlosResult<Vec<u8>> {
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

    /// Десериализует блок из ьайтового среза.
    pub fn deserialize(
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

    /// Возвращает несжатые данные (автоматически распаковывает если нужно).
    pub fn get_uncompressed_data(&self) -> GlosResult<Vec<u8>> {
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

// вместо macro_rules! read_u32 / read_u64
fn read_u32_local(
    buf: &[u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
) -> u32 {
    let b = [buf[*off], buf[*off + 1], buf[*off + 2], buf[*off + 3]];
    *off += 4;
    if is_le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    }
}

fn read_u64_local(
    buf: &[u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
) -> u64 {
    let b = [
        buf[*off],
        buf[*off + 1],
        buf[*off + 2],
        buf[*off + 3],
        buf[*off + 4],
        buf[*off + 5],
        buf[*off + 6],
        buf[*off + 7],
    ];
    *off += 8;
    if is_le {
        u64::from_le_bytes(b)
    } else {
        u64::from_be_bytes(b)
    }
}

// вместо macro_rules! write_u32 / write_u64
fn write_u32_local(
    buf: &mut [u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
    val: u32,
) {
    if is_le {
        buf[*off..*off + 4].copy_from_slice(&val.to_le_bytes());
    } else {
        buf[*off..*off + 4].copy_from_slice(&val.to_be_bytes());
    }
    *off += 4;
}

fn write_u64_local(
    buf: &mut [u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
    val: u64,
) {
    if is_le {
        buf[*off..*off + 8].copy_from_slice(&val.to_le_bytes());
    } else {
        buf[*off..*off + 8].copy_from_slice(&val.to_be_bytes());
    }
    *off += 8;
}

#[cfg(test)]
mod tests {
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
