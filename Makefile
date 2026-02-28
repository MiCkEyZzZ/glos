# The Glos Makefile
#
# Набор удобных команд для разработки Glos.
# Основные возможности:
#  - Сборка debug/release:   make build / make build-release
#  - Запуск:                 make run / make run-release
#  - Тесты и property тесты: make test / make proptest / make stress-test
#  - Форматирование/линты:   make fmt / make clippy / make check-all
#  - Fuzz и benchmark:       make fuzz / make bench
#  - Управление git/релизом: make git-tag / make git-release
#
# Переменные:
#  BUILD_TARGET  - читается из .cargo/config.toml (target triple)
#  TARGET_ARG    - автоматически формируется для cargo (--target ...)
#  TARGET_DIR    - путь в target/ для выбранного target'а
#  VERSION       - автоматически берётся из Cargo.toml (используется в release-auto)
#  CRATE         - для тестов конкретного крейта: make test CRATE=glos-core
#
# Как пользоваться:
#  make build            # debug-сборка
#  make build-release    # optimized release-сборка
#  make test             # запуск unit-тестов
#  make test CRATE=glos-core    # запуск тестов конкретного крейта
#  make test-all         # запуск всех тестов (root + все подкрейты)
#  make proptest         # property tests
#  make run              # запустить локально в debug режиме
#  make fuzz             # запуск фаззера (cargo-fuzz)
#
# Пометки:
#  - make help выведет удобную сводку доступных команд.
#  - Для CI полезно вызывать make check-all и make test-all.

BUILD_TARGET := $(shell test -f .cargo/config.toml && grep -E '^\s*target\s*=' .cargo/config.toml | head -1 | cut -d'"' -f2)
TARGET_ARG   := $(if $(BUILD_TARGET),--target $(BUILD_TARGET),)
TARGET_DIR   := target/$(if $(BUILD_TARGET),$(BUILD_TARGET)/,)

##@ Build
.PHONY: build build-release
build: ## Сборка debug
	cargo build $(TARGET_ARG)

build-release: ## Сборка релизной версии
	cargo build --release $(TARGET_ARG)

##@ Test
.PHONY: check clippy clippy-ci nextest test miri miri-test test-all nextest-all

check: ## Cargo проверка
	cargo check

clippy: ## Clippy (рассматривать предупреждения как ошибки)
	cargo clippy -- -D warnings

clippy-ci: ## Clippy как в CI: все таргеты и все фичи, warnings -> error
	cargo clippy --all-targets --all-features -- -D warnings

# Параметризованный тест - запускает тесты корневого проекта или конкретного крейта
test: ## Запуск тестов. Использование: make test [CRATE=glos-core]
ifdef CRATE
	@echo "Running tests for crate: $(CRATE)"
	cargo test -p $(CRATE)
else
	@echo "Running tests for root project"
	cargo test
endif

# Параметризованный nextest
nextest: ## Nextest. Использование: make nextest [CRATE=glos-core]
ifdef CRATE
	@echo "Running nextest for crate: $(CRATE)"
	cargo nextest run -p $(CRATE)
else
	@echo "Running nextest for root project"
	cargo nextest run
endif

test-all: ## Полный набор тестов (root + все подкрейты)
	@echo "Running all tests (root + crates)..."
	cargo test
	@for c in $(CRATES); do \
		echo ""; \
		echo "Running tests for crate: $$c"; \
		cargo test -p $$c; \
	done

# Запуск nextest везде
nextest-all: ## Nextest везде (root + все подкрейты)
	@echo "Running nextest everywhere (root + crates)..."
	cargo nextest run
	@for c in $(CRATES); do \
		echo ""; \
		echo "Running nextest for crate: $$c"; \
		cargo nextest run -p $$c; \
	done

miri: ## Запустите все тесты в Miri
	cargo miri test

miri-test: ## Запустите определенный тест в Miri. Использование: make miri-test TEST="модуль::имя_теста"
	cargo miri test $(TEST)

##@ Internal crates
CRATES := glos-core glos-ui glos-recorder glos-replayer glos-analyzer glos-cli

.PHONY: build-core build-ui test-core test-ui clippy-core clippy-ui fmt-core fmt-ui clean-core clean-ui

# Сборка всех внутренних крейтов
build-core:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c build; \
	done

build-ui:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c build; \
	done

# Тесты всех внутренних крейтов
test-core:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c test; \
	done

test-ui:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c test; \
	done

# Clippy для всех внутренних крейтов
clippy-core:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c clippy; \
	done

clippy-ui:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c clippy; \
	done

# Форматирование всех внутренних крейтов
fmt-core:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c fmt; \
	done

fmt-ui:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c fmt; \
	done

# Очистка всех внутренних крейтов
clean-core:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c clean; \
	done

clean-ui:
	@for c in $(CRATES); do \
		$(MAKE) -C $$c clean; \
	done

##@ Format & Lints
.PHONY: fmt fmt-toml fmt-all check-toml check-all

fmt: ## Rust fmt
	cargo fmt --all

fmt-toml: ## TOML fmt
	taplo format

fmt-all: ## Форматирование всего
	$(MAKE) fmt
	$(MAKE) fmt-toml

check-toml: ## Проверка TOML-формата
	taplo format --check

check-all: ## Полная проверка (check + clippy + формат + toml)
	$(MAKE) check
	$(MAKE) clippy
	$(MAKE) fmt
	$(MAKE) check-toml

##@ Bench & Fuzz
.PHONY: bench fuzz

bench: ## Бенчмарки
	cargo bench

fuzz: ## Fuzz tests
	cargo fuzz run

##@ Misc
.PHONY: clean

clean: ## Очистка артефактов
	cargo clean

##@ Git
.PHONY: git-add git-commit git-push git-status

git-add: ## Добавить все изменения в индекс
	git add .

git-commit: ## Закоммитить изменения. Использование: make git-commit MSG="Your message"
ifndef MSG
	$(error MSG is not set. Use make git-commit MSG="your message")
endif
	git commit -m "$(MSG)"

git-push: ## Отправить коммиты на удалённый репозиторий
	git push

git-status: ## Показать статус репозитория
	git status

##@ Git Release
.PHONY: git-tag git-push-tag git-release release-auto bump-version release-all

git-tag: ## Создание git-тега. Пример: make git-tag VERSION=v0.2.0
ifndef VERSION
	$(error VERSION is not set. Use make git-tag VERSION=v0.2.0)
endif
	git tag $(VERSION)

git-push-tag: ## Отправить тег в origin. Пример: make git-push-tag VERSION=v0.2.0
ifndef VERSION
	$(error VERSION is not set. Use make git-push-tag VERSION=v0.2.0)
endif
	git push origin $(VERSION)

git-release: ## Полный релиз: tag + push + GitHub Release. Пример: make git-release VERSION=v0.2.0
ifndef VERSION
	$(error VERSION is not set. Use make git-release VERSION=v0.1.0)
endif
	@# Проверка наличия gh
	@if ! command -v gh >/dev/null 2>&1; then \
	  echo "Error: GitHub CLI (gh) not found. Please install and authenticate."; \
	  exit 1; \
	fi
	$(MAKE) git-tag VERSION=$(VERSION)
	$(MAKE) git-push-tag VERSION=$(VERSION)
	gh release create $(VERSION) --generate-notes --allow-dirty

# Автоматический релиз по версии из Cargo.toml
VERSION := v$(shell awk -F\" '/^version/ {print $$2}' Cargo.toml)
release-auto: ## Автоматический релиз (tag + push) по версии из Cargo.toml
	$(MAKE) git-release VERSION=$(VERSION)

bump-version: ## Бампит патч-версию в Cargo.toml (cargo-edit)
	cargo set-version --bump patch
	git add Cargo.toml
	git commit -m "chore: bump version to $(shell awk -F\" '/^version/ {print $$2}' Cargo.toml)"

release-all: ## Полный цикл релиза: bump-version + release-auto
	$(MAKE) bump-version
	$(MAKE) release-auto

##@ Property testing команды
.PHONY: proptest proptest-quick proptest-long proptest-verbose proptest-coverage proptest-continuous proptest-timing \
        stress-test stress-test-quick endurance-test find-bugs-fast

# Быстрые property tests (100 случаев на тест)
proptest-quick: ## Быстрые property tests (100 случаев)
	PROPTEST_CASES=100 cargo test --test property_tests

# Обычные property tests (по умолчанию 1000 случаев)
proptest: ## Обычные property tests (по умолчанию)
	cargo test --test property_tests

# Длительное тестирование (10000 случаев)
proptest-long: ## Длительное property testing
	PROPTEST_CASES=10000 cargo test --test property_tests

# Подробный вывод для отладки
proptest-verbose: ## Подробный вывод для property tests
	PROPTEST_CASES=1000 RUST_LOG=debug cargo test --test property_tests -- --nocapture

# Запуск property tests с генерацией отчета о покрытии
proptest-coverage: ## Генерация покрытия для property tests (tarpaulin, HTML)
	cargo tarpaulin --tests --out Html --output-dir coverage/ --test property_tests

# Continuous property testing - запускать в фоне
proptest-continuous: ## Бесконечный цикл property tests (оставлять с осторожностью)
	while true; do \
		echo "Running property tests iteration $$(date)"; \
		PROPTEST_CASES=1000 cargo test --test property_tests || break; \
		sleep 60; \
	done

# Проверить что property tests проходят быстро (не более 30 сек как в Success Criteria)
proptest-timing: ## Измерение времени выполнения property tests
	time PROPTEST_CASES=1000 cargo test --test property_tests

# Запуск стресс-тестов (медленные, с большим количеством итераций)
stress-test: ## Запуск стресс-тестов (медленные, много итераций)
	PROPTEST_CASES=10000 cargo test --test stress_tests

# Быстрые стресс-тесты для CI
stress-test-quick: ## Быстрые стресс-тесты (короткие, для CI)
	PROPTEST_CASES=1000 cargo test --test stress_tests

# Эндуранс тест - найти memory leaks (очень медленный, только локально)
endurance-test: ## Эндуранс тест для поиска утечек памяти (медленный)
	cargo test --test stress_tests test_endurance_many_iterations --release -- --ignored --nocapture

# Найти баги быстро - краткий набор тестов с разными типами
find-bugs-fast: ## Минимальный набор тестов, чтобы быстро найти баги
	PROPTEST_CASES=500 cargo test --test property_tests roundtrip_all_values
	PROPTEST_CASES=500 cargo test --test property_tests numeric_edge_cases
	cargo test --test stress_tests test_compression_pathological_cases

##@ Run
.PHONY: run run-full run-compact run-release

run: ## Запуск Glos в режиме по умолчанию (debug → full)
	cargo run

run-full: ## Запуск Glos с полным баннером (force)
	GLOS_BANNER=full cargo run

run-compact: ## Запуск Glos с коротким баннером (force)
	GLOS_BANNER=compact cargo run

run-release: ## Запуск Glos в релизной версии
	cargo build --release $(TARGET_ARG) && ./$(TARGET_DIR)release/glos

##@ Help
help: ## Показать это сообщение
	@echo
	@echo "Glos Makefile (version $(shell awk -F\" '/^version/ {print $$2}' Cargo.toml))"
	@echo "Usage: make [target]"
	@echo
	@awk 'BEGIN {FS = ":.*##"; \
	  printf "%-20s %s\n", "Target", " Description"; \
	  printf "--------------------  -----------------------------\n"} \
	/^[a-zA-Z0-9_-]+:.*?##/ { printf " \033[36m%-20s\033[0m %s\n", $$1, $$2 } \
	/^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) }' $(MAKEFILE_LIST)
