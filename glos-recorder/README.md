# glos-recorder

## Установка

Добавьте в ваш `Cargo.toml`

```toml
[dependencies]
glos-recorder = "0.1"
```

## 📦 Структура

```
glos-recorder
├── docs
│   ├── план_развития.md
│   └── схема_проекта.md
├── examples
├── src
│   ├── config.rs       ← RecorderConfig, DeviceKind, parse_freq_hz()
│   ├── device.rs       ← trait SdrDevice, SimulatedDevice, create_device()
│   ├── error.rs        ← RecorderError / RecorderResult
│   ├── lib.rs          ← публичное API, реэкспорты
│   ├── main.rs         ← CLI (clap)
│   ├── metrics.rs      ← AtomicU64 метрики, MetricsSummary
│   └── pipeline.rs     ← RecordingPipeline (2 потока + writer loop)
├── tests
├── .gitignore
├── Cargo.toml
├── LICENSE
├── Makefile
└── README.md
```
