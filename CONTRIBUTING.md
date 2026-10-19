# Contributing

1. `cargo test` must pass, and `cargo build --release && ./target/release/questlog . --min-score 70` must exit 0.
2. Each check lives in `audit()` in `src/main.rs`. Category points must sum to 100 (a unit test enforces this).
3. New checks need a reason a reviewer would care, and a fix message that tells the user what to do.
4. No dependencies. The tool is std-only on purpose.
