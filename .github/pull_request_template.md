# Pull Request

Please include:

- **Scope**: what area of the codebase this touches (e.g. `glos-core`, `glos-replayer`, `glos-analyzer`, `glos-ui`).
- **Summary**: brief description of the change.
- **Testing**: how did you verify it works? (unit tests, manual steps).
- **Checklist**:
  - [ ] `cargo fmt --check`
  - [ ] `cargo clippy -- -D warnings`
  - [ ] New tests added / existing tests updated
  - [ ] Added the necessary rustdoc comments.
  - [ ] Changelog updated if applicable
