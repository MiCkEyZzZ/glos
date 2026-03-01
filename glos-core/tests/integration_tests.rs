use std::fs;

use glos_core::{
    serialization::{read_all_blocks, GlosReader, GlosWriter},
    GlosHeaderExt, IqBlockExt, GLOS_VERSION,
};
use glos_types::{Compression, GlosHeader, IqBlock, IqFormat, SdrType};
use tempfile::NamedTempFile;

// ===========================================================================
// Helpers — детерминированные тест-данные
// ===========================================================================

/// Детерминированный заголовок (timestamp_start фиксирован, не Now).
fn deterministic_header() -> GlosHeader {
    let mut h = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);

    h.gain_db = 40.0;
    h.iq_format = IqFormat::Int16;
    h.compression = Compression::None;
    h.timestamp_start = 1_704_067_200; // 2024-01-01 00:00:00 UTC
    h.timestamp_end = 0;
    h.total_samples = 0;
    h
}

/// Детерминированный блок Int16 IQ данных (пилообразный паттерн).
fn deterministic_block(
    ts_ns: u64,
    count: u32,
) -> IqBlock {
    let data: Vec<u8> = (0..count as usize)
        .flat_map(|i| {
            let i_val = ((i % 128) as i16) * 256;
            let q_val = -((i % 128) as i16) * 256;
            let mut bytes = [0u8; 4];
            bytes[0..2].copy_from_slice(&i_val.to_be_bytes());
            bytes[2..4].copy_from_slice(&q_val.to_be_bytes());
            bytes
        })
        .collect();
    IqBlock::new(ts_ns, count, data)
}

/// Строит детерминированный минимальный GLOS файл (Test Vector #1).
///
/// Параметры: HackRf, Int16, None, 2 Msps, 1602 MHz, 40 dB.
/// 2 блока по 1 000 выборок.
fn build_test_vector_1() -> Vec<u8> {
    let mut raw = Vec::new();
    let mut header = deterministic_header();
    header.total_samples = 2_000;
    header.timestamp_end = 1_704_067_201;
    raw.extend_from_slice(&header.serialize().unwrap());
    raw.extend_from_slice(
        &deterministic_block(1_704_067_200_000_000_000, 1_000)
            .serialize()
            .unwrap(),
    );
    raw.extend_from_slice(
        &deterministic_block(1_704_067_200_500_000_000, 1_000)
            .serialize()
            .unwrap(),
    );
    raw
}

/// Строит файл с LZ4-сжатием (Test Vector #2).
fn build_test_vector_2() -> Vec<u8> {
    let mut raw = Vec::new();
    let mut header = deterministic_header();
    header.compression = Compression::Lz4;
    header.total_samples = 2_000;
    header.timestamp_end = 1_704_067_201;
    raw.extend_from_slice(&header.serialize().unwrap());

    for i in 0..2u64 {
        let mut block = IqBlock::new(
            1_704_067_200_000_000_000 + i * 500_000_000,
            1_000,
            vec![42u8; 4_000], // повторяющиеся данные — хорошо сжимаются
        );
        block.compress().unwrap();
        raw.extend_from_slice(&block.serialize().unwrap());
    }
    raw
}

/// Строит файл с повреждённым вторым блоком (Test Vector #3).
fn build_test_vector_3() -> Vec<u8> {
    let mut raw = Vec::new();
    let header = deterministic_header();
    raw.extend_from_slice(&header.serialize().unwrap());

    // Блок 1: валидный
    raw.extend_from_slice(&deterministic_block(1_000_000_000, 100).serialize().unwrap());

    // Блок 2: порченый CRC
    let mut b2 = deterministic_block(2_000_000_000, 100).serialize().unwrap();
    let last = b2.len() - 1;
    b2[last] ^= 0xFF;
    raw.extend_from_slice(&b2);

    // Блок 3: валидный
    raw.extend_from_slice(&deterministic_block(3_000_000_000, 100).serialize().unwrap());

    raw
}

// ===========================================================================
// Test Vector #1 — минимальный валидный файл
// ===========================================================================

#[test]
fn test_vector_1_byte_layout() {
    let bytes = build_test_vector_1();

    // Заголовок: фиксированные поля
    assert_eq!(&bytes[0..4], b"GLOS", "magic");
    assert_eq!(bytes[4], 1, "version");
    assert_eq!(bytes[5], 0, "flags = big-endian");
    assert_eq!(bytes[12], 0, "sdr_type = HackRF");
    assert_eq!(bytes[13], 1, "iq_format = Int16");
    assert_eq!(bytes[14], 0, "compression = None");
    // sample_rate = 2_000_000 = 0x001E8480
    assert_eq!(&bytes[16..20], &[0x00, 0x1E, 0x84, 0x80], "sample_rate BE");
    // Первый блок начинается сразу после 128-байтного заголовка
    // content_size = 4 + 8 + 4000 = 4012 = 0x00000FAC
    assert_eq!(
        &bytes[128..132],
        &[0x00, 0x00, 0x0F, 0xAC],
        "content_size bytes"
    );

    // content_size = 4 + 8 + 4000 = 4012 = 0x00000FAC
    let content_size = u32::from_be_bytes([bytes[128], bytes[129], bytes[130], bytes[131]]);
    assert_eq!(content_size, 4_012, "content_size первого блока");
    // sample_count = 1000
    let sample_count = u32::from_be_bytes([bytes[132], bytes[133], bytes[134], bytes[135]]);
    assert_eq!(sample_count, 1_000, "sample_count первого блока");
}

#[test]
fn test_vector_1_parse_and_validate() {
    let raw = build_test_vector_1();
    let mut reader = GlosReader::new(std::io::Cursor::new(raw)).unwrap();

    let h = reader.header();
    assert_eq!(h.sdr_type, SdrType::HackRf);
    assert_eq!(h.sample_rate, 2_000_000);
    assert_eq!(h.center_freq, 1_602_000_000);
    assert_eq!(h.gain_db, 40.0);
    assert_eq!(h.iq_format, IqFormat::Int16);
    assert_eq!(h.compression, Compression::None);
    assert_eq!(h.total_samples, 2_000);
    assert_eq!(h.timestamp_start, 1_704_067_200);

    let blocks = read_all_blocks(&mut reader).unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].sample_count, 1_000);
    assert_eq!(blocks[1].sample_count, 1_000);

    // Все блоки Int16: sample_count × 4 == data.len()
    for block in &blocks {
        block.validate_sample_count(IqFormat::Int16).unwrap();
    }

    reader.validate_totals().unwrap();
}

#[test]
fn test_vector_1_deterministic_crc() {
    // Один и тот же вход → одинаковые байты → одинаковый CRC
    let raw1 = build_test_vector_1();
    let raw2 = build_test_vector_1();
    assert_eq!(raw1, raw2, "сборка должна быть детерминированной");
    assert_eq!(raw1[72..76], raw2[72..76], "header CRC должен совпадать");
}

// ===========================================================================
// Test Vector #2 — LZ4 compression
// ===========================================================================

#[test]
fn test_vector_2_compressed_smaller() {
    let uncompressed = build_test_vector_1();
    let compressed = build_test_vector_2();
    assert!(
        compressed.len() < uncompressed.len(),
        "LZ4 должен дать файл меньше ({} < {})",
        compressed.len(),
        uncompressed.len()
    );
}

#[test]
fn test_vector_2_parse_and_decompress() {
    let raw = build_test_vector_2();
    let mut reader = GlosReader::new(std::io::Cursor::new(raw)).unwrap();
    assert_eq!(reader.header().compression, Compression::Lz4);

    let blocks = read_all_blocks(&mut reader).unwrap();
    assert_eq!(blocks.len(), 2);
    // Блоки уже распакованы GlosReader
    assert!(!blocks[0].is_compressed);
    for block in &blocks {
        assert_eq!(block.data, vec![42u8; 4_000], "данные после распаковки");
        block.validate_sample_count(IqFormat::Int16).unwrap();
    }
    reader.validate_totals().unwrap();
}

// ===========================================================================
// Test Vector #3 — corrupted block recovery
// ===========================================================================

#[test]
fn test_vector_3_partial_recovery() {
    let raw = build_test_vector_3();
    let mut reader = GlosReader::new(std::io::Cursor::new(raw)).unwrap();

    let mut ok_blocks = Vec::new();
    while let Some(res) = reader.next_block() {
        if let Ok(block) = res {
            ok_blocks.push(block);
        }
    }

    assert_eq!(ok_blocks.len(), 2, "блоки 1 и 3 должны быть восстановлены");
    assert!(
        reader.stats().blocks_corrupted > 0,
        "блок 2 помечен как повреждённый"
    );
    assert_eq!(reader.stats().samples_recovered, 200); // 100 + 100
}

// ===========================================================================
// Existing integration tests (updated to pass Compression context)
// ===========================================================================

#[test]
fn test_large_file_streaming() {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path().to_path_buf();
    let block_samples: u32 = 512 * 1024 / 4; // ~128K Int16 пар ≈ 512 KB
    let num_blocks = 10;

    {
        let file = fs::File::create(&temp_path).unwrap();
        let header = GlosHeader::new(SdrType::HackRf, 10_000_000, 2_400_000_000);
        let mut writer = GlosWriter::new(file, header).unwrap();

        for i in 0..num_blocks as u64 {
            let data = vec![42u8; block_samples as usize * 4];
            writer
                .write_block(IqBlock::new(i * 1_000_000, block_samples, data))
                .unwrap();
        }
        writer.finish().unwrap();
    }

    {
        let file = fs::File::open(&temp_path).unwrap();
        let mut reader = GlosReader::new(file).unwrap();
        let blocks = read_all_blocks(&mut reader).unwrap();
        assert_eq!(blocks.len(), num_blocks);
        reader.validate_totals().unwrap();
    }

    fs::remove_file(&temp_path).unwrap();
}

#[allow(unused_variables, unused_assignments)]
#[test]
fn test_header_all_fields() {
    let mut header = GlosHeader::new(SdrType::UsrpB200, 50_000_000, 2_000_000_000);
    header.gain_db = 70.5;
    header.timestamp_end = 1_234_567_890;
    header.total_samples = 5_000_000_000;
    header.iq_format = IqFormat::Float32;
    header.compression = Compression::Lz4;

    let serialized = header.serialize().unwrap();
    let deserialized = GlosHeader::deserialize(&serialized).unwrap();

    assert_eq!(deserialized.version, GLOS_VERSION);
    assert_eq!(deserialized.sdr_type, SdrType::UsrpB200);
    assert_eq!(deserialized.sample_rate, 50_000_000);
    assert_eq!(deserialized.center_freq, 2_000_000_000);
    assert_eq!(deserialized.gain_db, 70.5);
    assert_eq!(deserialized.timestamp_end, 1_234_567_890);
    assert_eq!(deserialized.total_samples, 5_000_000_000);
    assert_eq!(deserialized.iq_format, IqFormat::Float32);
    assert_eq!(deserialized.compression, Compression::Lz4);
}

#[test]
fn test_block_boundary_conditions() {
    // Пустой блок (0 выборок)
    let block = IqBlock::new(0, 0, vec![]);
    let s = block.serialize().unwrap();
    let (d, _) = IqBlock::deserialize(&s, Compression::None).unwrap();
    assert_eq!(d.sample_count, 0);
    d.validate_sample_count(IqFormat::Int16).unwrap(); // 0 × 4 = 0 ✓

    // Одна выборка Int16
    let block = IqBlock::new(1, 1, vec![1, 2, 3, 4]);
    let s = block.serialize().unwrap();
    let (d, _) = IqBlock::deserialize(&s, Compression::None).unwrap();
    assert_eq!(d.sample_count, 1);
    assert_eq!(d.data, vec![1, 2, 3, 4]);
    d.validate_sample_count(IqFormat::Int16).unwrap(); // 1 × 4 = 4 ✓

    // Большой блок (до лимита 1 MB)
    let large_samples = (1024 * 1024 - 20) / 4;
    let large_data = vec![42u8; large_samples * 4];
    let block = IqBlock::new(999, large_samples as u32, large_data.clone());
    let s = block.serialize().unwrap();
    assert!(s.len() <= 1024 * 1024 + 8);
    let (d, _) = IqBlock::deserialize(&s, Compression::None).unwrap();
    d.validate_sample_count(IqFormat::Int16).unwrap();
}

#[test]
fn test_crc_validation_header() {
    let header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
    let mut serialized = header.serialize().unwrap();
    serialized[72] ^= 0x01;

    let result = GlosHeader::deserialize(&serialized);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("CRC"));
}

#[test]
fn test_crc_validation_block() {
    let block = IqBlock::new(123_456, 2, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let mut serialized = block.serialize().unwrap();
    serialized[20] ^= 0xFF;

    let result = IqBlock::deserialize(&serialized, Compression::None);
    assert!(result.is_err());
}

#[test]
fn test_version_mismatch() {
    let header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
    let mut serialized = header.serialize().unwrap();
    serialized[4] = 99; // неверная версия

    let result = GlosHeader::deserialize(&serialized);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Unsupported version"));
}

#[test]
fn test_timestamp_precision() {
    let now_ns = 1_234_567_890_123_456_789u64;
    let block = IqBlock::new(now_ns, 1, vec![1, 2, 3, 4]); // Int16: 1 × 4 = 4 ✓
    let s = block.serialize().unwrap();
    let (d, _) = IqBlock::deserialize(&s, Compression::None).unwrap();
    assert_eq!(d.timestamp_ns, now_ns);
}

#[test]
fn test_lz4_compression_basic() {
    let data = vec![42u8; 10_000]; // хорошо сжимается
    let mut block = IqBlock::new(0, 2_500, data.clone());

    let original_size = block.data.len();
    block.compress().unwrap();
    assert!(block.is_compressed);
    assert!(block.data.len() < original_size);

    block.decompress().unwrap();
    assert!(!block.is_compressed);
    assert_eq!(block.data, data);
}

#[test]
fn test_lz4_compression_random_data() {
    // Случайные данные плохо сжимаются, но должны корректно roundtrip-иться
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };
    let data: Vec<u8> = (0..10_000u32)
        .map(|i| {
            let mut h = DefaultHasher::new();
            i.hash(&mut h);
            h.finish() as u8
        })
        .collect();
    let mut block = IqBlock::new(0, 2_500, data.clone());
    block.compress().unwrap();
    block.decompress().unwrap();
    assert_eq!(block.data, data);
}

#[test]
fn test_compressed_block_serialization() {
    // Int8: 4 выборки × 2 = 8 байт
    let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut block = IqBlock::new(999, 4, data.clone());
    block.compress().unwrap();

    let s = block.serialize().unwrap();
    let (d, _) = IqBlock::deserialize(&s, Compression::Lz4).unwrap();
    let uncompressed = d.get_uncompressed_data().unwrap();
    assert_eq!(uncompressed, data);
}

#[test]
fn test_little_endian_header() {
    let mut header = GlosHeader::new(SdrType::PlutoSdr, 10_000_000, 1_575_000_000);
    header.flags = 0x01;
    header.gain_db = 35.5;
    header.total_samples = 1_000_000;

    let s = header.serialize().unwrap();
    let d = GlosHeader::deserialize(&s).unwrap();

    assert!(d.is_little_endian());
    assert_eq!(d.sample_rate, 10_000_000);
    assert_eq!(d.center_freq, 1_575_000_000);
    assert_eq!(d.gain_db, 35.5);
    assert_eq!(d.total_samples, 1_000_000);
}

#[test]
fn test_double_compress_decompress_idempotent() {
    let data = vec![42u8; 1000];
    let mut block = IqBlock::new(0, 500, data.clone());

    block.compress().unwrap();
    let size1 = block.data.len();
    block.compress().unwrap(); // no-op
    assert_eq!(block.data.len(), size1);

    block.decompress().unwrap();
    block.decompress().unwrap(); // no-op
    assert_eq!(block.data, data);
}

// ===========================================================================
// Пункт 7: pre-processed тип данных — документированное решение
// ===========================================================================

/// Формат `pre-processed` из изначального issue не реализован как отдельный
/// enum вариант. Решение: pre-processed данные хранятся через
/// `IqFormat::Float32` (нормализованные комплексные выборки после
/// downconversion/filtering) или через `SdrType::Unknown` с кастомным
/// `IqFormat`. Отдельный тип не нужен т.к. блок данных сам по себе непрозрачен
/// — интерпретация зависит от метаданных заголовка. Зафиксировано в
/// спецификации для v1.0.
#[allow(unused_variables, unused_assignments)]
#[test]
fn test_pre_processed_via_float32() {
    // Pre-processed IQ: нормализованные float32 выборки
    let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
    header.iq_format = IqFormat::Float32;

    // 5 выборок × 8 байт = 40 байт
    let data: Vec<u8> = (0..5u32)
        .flat_map(|i| {
            let i_val = (i as f32 / 5.0).to_be_bytes();
            let q_val = -(i as f32 / 5.0); // используем унарный минус вместо * -1.0
            let q_val = q_val.to_be_bytes();
            [i_val, q_val].concat()
        })
        .collect();

    let block = IqBlock::new(0, 5, data.clone());
    let s = block.serialize().unwrap();
    let (d, _) = IqBlock::deserialize(&s, Compression::None).unwrap();

    d.validate_sample_count(IqFormat::Float32).unwrap(); // 5 × 8 = 40 ✓
    assert_eq!(d.data, data);
}
