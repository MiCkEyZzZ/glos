# Changelog

All notable changes to **GLOS** are documented in this file.

---

## [Unreleased] — 00-00-0000

### Added

- Code quality tooling configured:
  - `rustfmt`
  - `clippy`
  - `taplo`
  - `cargo-deny`

- Added `Makefile` for workspace management and common developer workflows.
- CI/CD pipelines and GitHub templates for Issues and Pull Requests.

- **glos**
  - Настроены линтеры и инструменты качества кода: `taplo`, `rustfmt`, `clippy`, `cargo-deny`;
  - Добавлен `Makefile` для управления сборкой и внутренними крейтами;

* **glos-core**
  - Introduced the `.glos` binary container format for IQ data and metadata storage.
  - Implemented versioned file header describing receiver and session parameters.
  - Added CRC32 validation for headers and data blocks.
  - Support for multiple IQ sample representations:
    - `i8`
    - `i16`
    - compressed payload variants.

  - Implemented serialization and deserialization layers:
    - `format.rs`
    - `serialization.rs`

  - Added validation tests and format verification vectors.
  - Initial documentation and binary layout examples.

- **glos-recorder**
  - Initial recording pipeline capable of capturing IQ streams and writing `.glos` sessions.
  - HackRF device integration (feature-gated backend).
  - Buffered streaming writer with continuous disk output.
  - Ring-buffer based ingestion pipeline for stable streaming workloads.
  - Graceful shutdown with proper block finalization and file integrity guarantees.
  - Command-line interface:

    ```bash
    glos-recorder \
      --device hackrf \
      --freq 1602MHz \
      --gain 40 \
      --output file.glos \
      --duration 60
    ```

  - Runtime error handling:
    - sample loss detection
    - I/O failure handling
    - recorder state validation
  - Recording metrics:
    - total samples recorded
    - dropped frames
    - write throughput statistics
  - Unit tests covering read/write recording pipeline.

### Performance Characteristics

Current MVP targets:

- Supported SDR class: HackRF / PlutoSDR
- Stable recording range: **0.5–2 Msps**
- Data rate example:
  - 2 Msps (`i8 + i8`) ≈ 4 MB/s
  - ≈ 14.4 GB/hour (uncompressed)

- Capture → disk latency target: **< 100 ms**
  (typically < 200 ms on HDD systems)
- CPU usage target: **< 40% of a single core**

Future stretch goal:

- 20–50 Msps aggregate throughput with professional SDR or FPGA-based frontends.

### Changed

- **changelog**
  - Test execution workflow clarified:
    tests can now be executed either from the workspace root or per-crate level.
