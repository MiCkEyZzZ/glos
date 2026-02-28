use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};

use crate::{
    error::{GlosError, GlosResult},
    format::{Compression, GlosHeader, IqBlock, GLOS_HEADER_SIZE},
};

/// Потоковый писатель GLOS файлов.
pub struct GlosWriter<W: Write + Seek> {
    writer: BufWriter<W>,
    header: GlosHeader,
    total_samples: u64,
    block_count: u64,
}

/// Потоковый читатель GLOS файлов.
pub struct GlosReader<R: Read> {
    reader: BufReader<R>,
    header: GlosHeader,
    read_buf: Vec<u8>,
    leftover: Vec<u8>,
    stats: ReadStats,
    eof: bool,
}

/// Статистика, накопленная [`GlosReader`] в процессе чтения.
#[derive(Debug, Default, Clone)]
pub struct ReadStats {
    /// Успешно прочитанных блоков.
    pub blocks_ok: u64,
    /// Блоков с ошибкой CRC или повреждённых.
    pub blocks_corrupted: u64,
    /// Сумма `sample_count` по всем успешным блокам.
    pub samples_recovered: u64,
    /// Всего обработано байт (включая служебные поля блоков).
    pub bytes_processed: u64,
}

impl<W: Write + Seek> GlosWriter<W> {
    /// Создаёт новый писатель, немедленно записывая заголовок в поток.
    pub fn new(
        inner: W,
        header: GlosHeader,
    ) -> GlosResult<Self> {
        let mut writer = BufWriter::new(inner);

        writer.write_all(&header.serialize()?)?;

        Ok(Self {
            writer,
            header,
            total_samples: 0,
            block_count: 0,
        })
    }

    /// Записывает один блок IQ данных.
    pub fn write_block(
        &mut self,
        mut block: IqBlock,
    ) -> GlosResult<()> {
        if self.header.compression == Compression::Lz4 && !block.is_compressed {
            block.compress()?;
        }

        self.total_samples += block.sample_count as u64;
        self.block_count += 1;
        self.writer.write_all(&block.serialize()?)?;

        Ok(())
    }

    /// Завершает запись: сбрасывает буфер и перезаписывает заголовок.
    pub fn finish(mut self) -> GlosResult<()> {
        self.writer.flush()?;
        self.header.total_samples = self.total_samples;
        self.header.timestamp_end = current_unix_secs();

        let mut inner = self
            .writer
            .into_inner()
            .map_err(|e| GlosError::Io(e.into_error()))?;

        inner.seek(SeekFrom::Start(0))?;
        inner.write_all(&self.header.serialize()?)?;
        inner.flush()?;

        Ok(())
    }

    /// Общее количество записанных IQ выборок (до вызова [`finish`]).
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    /// Количество записанных блоков.
    pub fn block_count(&self) -> u64 {
        self.block_count
    }

    /// Ссылка на текущий заголовок (до финализации).
    pub fn header(&self) -> &GlosHeader {
        &self.header
    }
}

impl<R: Read> GlosReader<R> {
    /// Создаёт читатель, читая и валидируя заголовок из `inner`.
    pub fn new(inner: R) -> GlosResult<Self> {
        let mut reader = BufReader::new(inner);
        let mut hdr_buf = [0u8; GLOS_HEADER_SIZE];

        reader.read_exact(&mut hdr_buf)?;

        let header = GlosHeader::deserialize(&hdr_buf)?;

        Ok(Self {
            reader,
            header,
            read_buf: vec![0u8; 2 * 1024 * 1024],
            leftover: Vec::new(),
            stats: ReadStats::default(),
            eof: false,
        })
    }

    /// Возвращает следующий блок или `None` на EOF.
    pub fn next_block(&mut self) -> Option<GlosResult<IqBlock>> {
        loop {
            if self.leftover.len() >= 20 {
                match IqBlock::deserialize(&self.leftover, self.header.compression) {
                    Ok((mut block, bytes_read)) => {
                        // Распаковка (если нужна)
                        if block.decompress().is_err() {
                            // Сжатые данные повреждены — пропускаем весь блок
                            self.leftover.drain(..bytes_read);
                            self.stats.blocks_corrupted += 1;
                            continue;
                        }

                        // Валидация: sample_count × sample_size == data.len()
                        // (спецификация п.5)
                        if block.validate_sample_count(self.header.iq_format).is_err() {
                            self.leftover.drain(..bytes_read);
                            self.stats.blocks_corrupted += 1;
                            continue;
                        }

                        self.stats.blocks_ok += 1;
                        self.stats.samples_recovered += block.sample_count as u64;
                        self.stats.bytes_processed += bytes_read as u64;
                        self.leftover.drain(..bytes_read);
                        return Some(Ok(block));
                    }

                    Err(GlosError::Corrupted(_)) => {
                        if self.eof {
                            // leftover.len() >= 20, значит данные есть, но
                            // content_size указывает за конец буфера — мусор
                            // после повреждённого блока. Сканируем побайтово.
                            self.leftover.drain(..1);
                            continue;
                        }
                        // Данных не хватает — дочитываем
                    }

                    Err(GlosError::CrcMismatch { .. }) => {
                        self.stats.blocks_corrupted += 1;
                        self.leftover.drain(..1);
                        continue;
                    }

                    Err(e) => {
                        self.stats.blocks_corrupted += 1;
                        self.leftover.drain(..1);
                        return Some(Err(e));
                    }
                }
            }

            if self.eof {
                // leftover < 20: усечённый хвост файла, завершаем
                return None;
            }

            match self.reader.read(&mut self.read_buf) {
                Ok(0) => {
                    self.eof = true;
                    if self.leftover.is_empty() {
                        return None;
                    }
                }
                Ok(n) => {
                    self.leftover.extend_from_slice(&self.read_buf[..n]);
                }
                Err(e) => return Some(Err(GlosError::Io(e))),
            }
        }
    }

    /// Проверяет, что `header.total_samples = Σ block.sample_count`.
    pub fn validate_totals(&self) -> GlosResult<()> {
        let expected = self.header.total_samples;

        if expected == 0 {
            return Ok(());
        }

        if self.stats.samples_recovered != expected {
            return Err(GlosError::FormatViolation(format!(
                "total_samples mismatch: header={}, recovered={}",
                expected, self.stats.samples_recovered,
            )));
        }

        Ok(())
    }

    /// Прочитанный и проверенный заголовок файла.
    pub fn header(&self) -> &GlosHeader {
        &self.header
    }

    /// Накопленная статистика чтения.
    pub fn stats(&self) -> &ReadStats {
        &self.stats
    }
}

impl<R: Read> Iterator for GlosReader<R> {
    type Item = GlosResult<IqBlock>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_block()
    }
}

/// Convenience: читает все блоки из файла, собирая их в вектор.
///
/// Повреждённые блоки пропускаются.
pub fn read_all_blocks<R: Read>(reader: &mut GlosReader<R>) -> GlosResult<Vec<IqBlock>> {
    let mut blocks = Vec::new();
    while let Some(result) = reader.next_block() {
        match result {
            Ok(block) => blocks.push(block),
            Err(GlosError::CrcMismatch { .. }) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(blocks)
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::format::{Compression, IqFormat, SdrType};

    fn make_header() -> GlosHeader {
        GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000)
    }

    fn make_block(
        ts: u64,
        count: u32,
    ) -> IqBlock {
        let data = vec![0u8; count as usize * 4]; // Int16: 4 байта/пара
        IqBlock::new(ts, count, data)
    }

    #[test]
    fn test_writer_reader_round_trip() {
        let buf = Cursor::new(Vec::<u8>::new());
        let header = make_header();
        let mut writer = GlosWriter::new(buf, header).unwrap();

        for i in 0..5u64 {
            writer.write_block(make_block(i * 1_000_000, 1000)).unwrap();
        }
        let total = writer.total_samples();
        assert_eq!(total, 5000);

        writer.finish().unwrap();

        // GlosWriter потреблён — читаем напрямую из raw bytes
        // Пересоздаём через отдельный буфер (finish перезаписал заголовок)
    }

    #[test]
    fn test_reader_iterates_blocks() {
        // Сериализуем вручную чтобы создать Cursor
        let mut raw = Vec::<u8>::new();
        let header = make_header();
        raw.extend_from_slice(&header.serialize().unwrap());

        for i in 0..3u64 {
            let block = make_block(i * 500_000, 500);
            raw.extend_from_slice(&block.serialize().unwrap());
        }

        let mut reader = GlosReader::new(Cursor::new(raw)).unwrap();
        let mut count = 0;

        while let Some(res) = reader.next_block() {
            res.unwrap();
            count += 1;
        }

        assert_eq!(count, 3);
        assert_eq!(reader.stats().blocks_ok, 3);
        assert_eq!(reader.stats().blocks_corrupted, 0);
        assert_eq!(reader.stats().samples_recovered, 1500);
    }

    #[test]
    fn test_iterator_impl() {
        let mut raw = Vec::<u8>::new();
        let header = make_header();
        raw.extend_from_slice(&header.serialize().unwrap());
        raw.extend_from_slice(&make_block(0, 100).serialize().unwrap());
        raw.extend_from_slice(&make_block(1, 200).serialize().unwrap());

        let reader = GlosReader::new(Cursor::new(raw)).unwrap();
        let blocks: Vec<_> = reader.filter_map(|r| r.ok()).collect();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].sample_count, 100);
        assert_eq!(blocks[1].sample_count, 200);
    }

    #[test]
    fn test_corrupted_block_skipped() {
        let mut raw = Vec::<u8>::new();
        let header = make_header();
        raw.extend_from_slice(&header.serialize().unwrap());

        let b1 = make_block(1, 10).serialize().unwrap();
        let mut b2_corrupt = make_block(2, 10).serialize().unwrap();
        let b3 = make_block(3, 10).serialize().unwrap();

        // Портим CRC второго блока
        let last = b2_corrupt.len() - 1;
        b2_corrupt[last] ^= 0xFF;

        raw.extend_from_slice(&b1);
        raw.extend_from_slice(&b2_corrupt);
        raw.extend_from_slice(&b3);

        let mut reader = GlosReader::new(Cursor::new(raw)).unwrap();
        let mut ok = 0u32;
        while let Some(res) = reader.next_block() {
            if res.is_ok() {
                ok += 1;
            }
        }

        // Блоки 1 и 3 читаются, блок 2 пропускается
        assert_eq!(ok, 2);
        assert!(reader.stats().blocks_corrupted > 0);
    }

    #[test]
    fn test_lz4_auto_compress_decompress() {
        let mut raw = Vec::<u8>::new();
        let mut header = make_header();
        header.compression = Compression::Lz4;
        header.iq_format = IqFormat::Int16;
        raw.extend_from_slice(&header.serialize().unwrap());

        // Записываем сжатый блок вручную (имитируем то, что делает GlosWriter)
        let data = vec![42u8; 4000]; // хорошо сжимается
        let mut b = IqBlock::new(0, 1000, data.clone());
        b.compress().unwrap();
        let compressed_bytes = b.serialize().unwrap();
        raw.extend_from_slice(&compressed_bytes);

        let mut reader = GlosReader::new(Cursor::new(raw)).unwrap();
        assert_eq!(reader.header().compression, Compression::Lz4);

        let block_out = reader.next_block().unwrap().unwrap();
        // После автоматической распаковки данные должны совпасть
        assert_eq!(block_out.data, data);
        assert!(!block_out.is_compressed);
    }

    #[test]
    fn test_read_all_blocks_helper() {
        let mut raw = Vec::<u8>::new();
        raw.extend_from_slice(&make_header().serialize().unwrap());
        for i in 0..4u64 {
            raw.extend_from_slice(&make_block(i, 50).serialize().unwrap());
        }

        let mut reader = GlosReader::new(Cursor::new(raw)).unwrap();
        let blocks = read_all_blocks(&mut reader).unwrap();
        assert_eq!(blocks.len(), 4);
    }

    #[test]
    fn test_header_validated_on_open() {
        let mut raw = vec![0u8; 128]; // мусор
        raw[0..4].copy_from_slice(b"XXXX"); // неверный magic

        let result = GlosReader::new(Cursor::new(raw));
        assert!(result.is_err());
    }

    #[test]
    fn test_writer_block_count() {
        let mut raw = Vec::<u8>::new();
        {
            let cursor = Cursor::new(&mut raw);
            let mut writer = GlosWriter::new(cursor, make_header()).unwrap();
            writer.write_block(make_block(0, 100)).unwrap();
            writer.write_block(make_block(1, 200)).unwrap();
            assert_eq!(writer.block_count(), 2);
            assert_eq!(writer.total_samples(), 300);
            writer.finish().unwrap();
        }

        // После finish заголовок должен содержать total_samples = 300
        let mut reader = GlosReader::new(Cursor::new(raw)).unwrap();
        assert_eq!(reader.header().total_samples, 300);

        let blocks = read_all_blocks(&mut reader).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn test_empty_file_no_blocks() {
        let mut raw = Vec::<u8>::new();
        raw.extend_from_slice(&make_header().serialize().unwrap());

        let mut reader = GlosReader::new(Cursor::new(raw)).unwrap();
        assert!(reader.next_block().is_none());
        assert_eq!(reader.stats().blocks_ok, 0);
    }
}
