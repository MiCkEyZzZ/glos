use std::net::UdpSocket;

use glos_core::{GlosHeaderExt, IqBlockExt, UdpPacket};
use glos_replayer::{ReplayConfiq, ReplaySession};
use glos_types::{GlosHeader, IqBlock, IqFormat, SdrType};
use tempfile::NamedTempFile;

#[test]
fn test_integration_record_then_replay() {
    use std::time::Duration;

    use glos_core::serialization::GlosWriter;

    // --- Запись ---
    let tmp = NamedTempFile::new().unwrap();
    let n_blocks: u64 = 10;
    let samples: u32 = 200;

    {
        let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
        header.iq_format = IqFormat::Int16;
        let file = std::fs::File::create(tmp.path()).unwrap();
        let mut writer = GlosWriter::new(file, header).unwrap();
        let period_ns = 500u64; // 1/2Msps
        for i in 0..n_blocks {
            let ts = 1_000_000_000u64 + i * samples as u64 * period_ns;
            let data = vec![(i as u8).wrapping_mul(7); samples as usize * 4];
            writer.write_block(IqBlock::new(ts, samples, data)).unwrap();
        }
        writer.finish().unwrap();
    }

    // --- Воспроизведение ---
    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    listener
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();

    let config = ReplayConfiq {
        input_path: tmp.path().to_path_buf(),
        target_addr: addr,
        speed: 100.0,
        loop_playback: false,
        stats_interval_secs: 60,
        bind_addr: "0.0.0.0:0".to_string(),
    };
    let session = ReplaySession::new(config).unwrap();
    session.run().unwrap();

    // --- Проверка ---
    let mut received_ts: Vec<u64> = Vec::new();
    let mut buf = vec![0u8; 65536];
    while let Ok(n) = listener.recv(&mut buf) {
        let (ts, count, data) = UdpPacket::decode(&buf[..n]).unwrap();
        assert_eq!(count, samples as u16);
        assert_eq!(data.len(), samples as usize * 4);
        received_ts.push(ts);
    }

    assert_eq!(
        received_ts.len(),
        n_blocks as usize,
        "все пакеты должны дойти"
    );

    // Timestamp'ы монотонно возрастают
    for w in received_ts.windows(2) {
        assert!(
            w[1] > w[0],
            "timestamps должны быть монотонными: {} > {}",
            w[1],
            w[0]
        );
    }
}
