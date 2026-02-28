//! Пример: чтение GLOS-файла через GlosReader
//!
//! Демонстрирует:
//! - открытие файла и валидацию заголовка через GlosReader
//! - итерацию блоков (повреждённые автоматически пропускаются)
//! - финальную валидацию total_samples

use std::fs::File;

use glos_core::serialization::{read_all_blocks, GlosReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = "glos-core/test_output.glos";

    // --- GlosReader валидирует заголовок при открытии ---
    let file = File::open(input_path)?;
    let mut reader = match GlosReader::new(file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("✗ Header validation failed: {e}");
            return Err(Box::new(e));
        }
    };

    let h = reader.header();
    println!("✓ Header validated");
    println!("  SDR Type      : {:?}", h.sdr_type);
    println!("  Sample Rate   : {} Hz", h.sample_rate);
    println!("  Center Freq   : {} Hz", h.center_freq);
    println!("  Gain          : {} dB", h.gain_db);
    println!("  IQ Format     : {:?}", h.iq_format);
    println!("  Compression   : {:?}", h.compression);
    println!("  Total Samples : {}", h.total_samples);
    println!("  Timestamp End : {}", h.timestamp_end);

    // --- Читаем все блоки (повреждённые пропускаются) ---
    let blocks = read_all_blocks(&mut reader)?;

    println!("\n✓ Read complete");
    println!("  Blocks ok        : {}", reader.stats().blocks_ok);
    println!("  Blocks corrupted : {}", reader.stats().blocks_corrupted);
    println!("  Samples recovered: {}", reader.stats().samples_recovered);

    // --- Валидация total_samples == Σ sample_count ---
    match reader.validate_totals() {
        Ok(()) => println!("  Total samples    : ✓ match"),
        Err(e) => println!("  Total samples    : ✗ {e}"),
    }

    // --- Показываем первые 3 блока ---
    println!("\nFirst blocks:");
    for (i, block) in blocks.iter().take(3).enumerate() {
        println!(
            "  [{i}] {} samples @ {}ns (compressed={})",
            block.sample_count, block.timestamp_ns, block.is_compressed
        );
    }

    Ok(())
}
