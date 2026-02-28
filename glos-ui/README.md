# glos-ui

Графический интерфейс для системы анализа GNSS/SDR сигналов GLOS.

**GLOS-UI** — это инженерный инструмент визуализации, предоставляющий:

- Dashboard: общая сводка состояния системы, метрик и спутников;
- Signal View: спектральный анализ, FFT, waterfall plot;
- Satellites Panel: таблица спутников с метриками (CN0, допплер, elevation) + sky plot;
- Logs: системные логи с временными метками и подсветкой;

## Использование

### Запуск

```zsh
cargo build --release
cargo run --release
```
