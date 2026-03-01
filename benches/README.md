# Collection of CPU benchmarks for `Glos`

These benchmarks are designed as a first line of defense against performance
regressions and generally provide an approximate estimate of performance for users.

## Usage

```zsh
# Run all benchmarks
cargo bench -p glos-benchmark

# Run a specific benchmark containing the word "filter" in its name
cargo bench -p glos-benchmark -- "filter"
```
