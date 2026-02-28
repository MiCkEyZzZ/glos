# The Glos CLI Makefile

.PHONY: build build-release test clippy fmt clean

# Сборка
build:
	cargo build

build-release:
	cargo build --release

# Тесты
test:
	cargo test

# Clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Форматирование
fmt:
	cargo fmt

# Очистка
clean:
	cargo clean
