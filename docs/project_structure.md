# Схема проекта Glos

```
glos
├── .cargo
│   └── config.toml
├── .config
│   ├── config.toml
│   └── nextest.toml
├── .github
│   ├── ISSUE_TEMPLATE
│   │   ├── dependency-check.yml
│   │   ├── config.yml
│   │   ├── crash_report.yml
│   │   ├── enhancement.yml
│   │   ├── feature.yml
│   │   ├── other_stuff.yml
│   │   └── question.yml
│   ├── workflows
│   │   ├── dependency-check.yml
│   │   ├── dev-build.yml
│   │   ├── property-tests.yml
│   │   ├── release.yml
│   │   └── semantic-pull-request.yml
│   ├── cargo-blacklist.txt
│   ├── CODEOWNERS
│   └── pull_request_template.md
├── benches
│   ├── benches
│   │   └── recorder_benchmark
│   │       └── recorder_bench.rs
│   ├── Cargo.toml
│   └── README.md
├── docs
│   ├── ARCHITECTURE.md
│   ├── glos_file_format_spec_v1_0.md
│   ├── project_structure.md
│   ├── QUICK_START.md
│   ├── ROADMAP.md
│   └── TESTING.md
├── glos-analyzer
│   ├── docs
│   │   ├── roadmap.md
│   │   └── project_structure.md
│   ├── src
│   │   └── lib.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── glos-cli
│   ├── docs
│   │   ├── roadmap.md
│   │   └── project_structure.md
│   ├── src
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── glos-core
│   ├── docs
│   │   ├── roadmap.md
│   │   └── project_structure.md
│   ├── examples
│   │   ├── read_glos_file.rs
│   │   └── write_glos_file.rs
│   ├── src
│   │   ├── binary
│   │   │   ├── mod.rs
│   │   │   ├── read.rs
│   │   │   └── write.rs
│   │   ├── format.rs
│   │   ├── lib.rs
│   │   └── serialization.rs
│   ├── tests
│   │   └── integration_tests.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── glos-recorder
│   ├── docs
│   │   ├── roadmap.md
│   │   └── project_structure.md
│   ├── src
│   │   ├── config.rs
│   │   ├── device.rs
│   │   ├── error.rs
│   │   ├── lib.rs
│   │   ├── metrics.rs
│   │   └── pipeline.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── glos-replayer
│   ├── docs
│   │   ├── roadmap.md
│   │   └── project_structure.md
│   ├── src
│   │   ├── config.rs
│   │   ├── error.rs
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── replayer.rs
│   │   └── session.rs
│   ├── tests
│   │   └── integration_tests.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── glos-types
│   ├── docs
│   │   └── project_structure.md
│   ├── src
│   │   ├── compression.rs
│   │   ├── error.rs
│   │   ├── header.rs
│   │   ├── iq_block.rs
│   │   ├── iq_format.rs
│   │   ├── lib.rs
│   │   └── sdr.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── glos-ui
│   ├── docs
│   │   ├── roadmap.md
│   │   └── project_structure.md
│   ├── src
│   │   ├── data
│   │   │   ├── export.rs
│   │   │   ├── mock.rs
│   │   │   └── mod.rs
│   │   ├── panels
│   │   │   ├── dashboard.rs
│   │   │   ├── logs.rs
│   │   │   ├── mod.rs
│   │   │   ├── satellites.rs
│   │   │   ├── settings.rs
│   │   │   └── signals.rs
│   │   ├── app.rs
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   └── theme.rs
│   ├── .gitignore
│   ├── Cargo.toml
│   ├── LICENSE
│   ├── Makefile
│   └── README.md
├── .editorconfig
├── .gitignore
├── AUTHOR.md
├── BUGS
├── Cargo.lock
├── Cargo.toml
├── CHANGELOG.md
├── clippy.toml
├── CODE_OF_CONDUCT.md
├── CONTRIBUTING.md
├── deny.md
├── INSTALL
├── LICENSE
├── Makefile
├── README.md
├── rust-toolchain.toml
├── rustfmt.toml
├── SECURITY.md
└── taplo.toml
```
