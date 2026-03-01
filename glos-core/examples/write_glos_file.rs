//! Пример: запись GLOS-файла с синтетическими IQ-данными

//! Пример: запись GLOS-файла через GlosWriter
//!
//! Демонстрирует:
//! - создание заголовка и GlosWriter
//! - генерацию синтетической IQ-синусоиды
//! - автоматическое обновление заголовка при finish()

use std::fs::File;

use glos_core::{GlosHeaderExt, GlosWriter, IqBlockExt};
use glos_types::{GlosHeader, IqBlock, IqFormat, SdrType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = "glos-core/test_output.glos";

    // --- Заголовок ---
    let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
    header.gain_db = 40.0;
    header.iq_format = IqFormat::Int16;

    // --- GlosWriter ---
    let file = File::create(output_path)?;
    let mut writer = GlosWriter::new(file, header)?;

    // --- Синтетические IQ-данные: комплексная синусоида 1 kHz ---
    let num_blocks = 10;
    let samples_per_block: u32 = 50_000;

    for block_idx in 0..num_blocks {
        let mut iq_data = Vec::with_capacity(samples_per_block as usize * 4);

        for i in 0..samples_per_block {
            let t = (block_idx * samples_per_block + i) as f32 / 2_000_000_f32;
            let freq = 1_000.0_f32;
            let i_val = (32_767.0 * (2.0 * std::f32::consts::PI * freq * t).sin()) as i16;
            let q_val = (32_767.0 * (2.0 * std::f32::consts::PI * freq * t).cos()) as i16;
            iq_data.extend_from_slice(&i_val.to_be_bytes());
            iq_data.extend_from_slice(&q_val.to_be_bytes());
        }

        // timestamp_ns: каждый блок = 25 мс при 2 Msps
        let timestamp_ns = block_idx as u64 * samples_per_block as u64 * 500;
        let block = IqBlock::new(timestamp_ns, samples_per_block, iq_data);
        writer.write_block(block)?;

        println!("Block {block_idx}: {samples_per_block} samples written");
    }

    // finish() перезаписывает заголовок с total_samples и timestamp_end
    let total = writer.total_samples();
    writer.finish()?;

    println!("\n✓ Записано: {output_path}");
    println!("  Blocks   : {num_blocks}");
    println!("  Samples  : {total}");

    Ok(())
}
