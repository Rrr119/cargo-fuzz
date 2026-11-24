# Dictionary and structured-input coverage comparison

This benchmark compares three small parser crates:

- `route_lite`: HTTP request-line and route/path/query parser.
- `kv_record`: sectioned `key=value;` configuration parser.
- `tag_lite`: HTML-like tag and attribute scanner.

Each crate has a standard libFuzzer dictionary (`dictionary.dict`) and a TOML
schema (`schema.toml`) used to generate deterministic structured corpus seeds.

## Reproduce

Install the LLVM coverage tools, then run:

```sh
rustup component add llvm-tools-preview
python3 benchmarks/coverage-compare/run.py --runs 3000
```

The script creates isolated copies under `target/coverage-compare`, runs the
same fixed libFuzzer budget for each crate/strategy, and writes:

- `coverage-comparison.md`
- `coverage-comparison.csv`

The table is aggregated with:

```sh
cargo fuzz compare-coverage \
  --expect-three-crates \
  --report route_lite:structured:route_lite-structured.json
```

## Strategies

- `baseline`: empty starting corpus.
- `dictionary`: passes the crate dictionary through the new `--dict` bridge,
  which becomes libFuzzer's native `-dict=...` argument.
- `structured`: generates schema-driven corpus files with
  `cargo fuzz generate seeds` before fuzzing.

Generated structured targets can also use `Arbitrary` directly:

```sh
cargo fuzz add --structured my_target
cargo fuzz generate target my_other_target --schema fuzz/schemas/input.toml
```

The generated code uses `libfuzzer_sys::arbitrary` and does not require a
separate schema runtime.
