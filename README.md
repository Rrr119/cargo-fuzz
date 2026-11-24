<div align="center">
  <h1><code>cargo fuzz</code></h1>

  <p><b>A <code>cargo</code> subcommand for fuzzing with <code>libFuzzer</code>! Easy to use!</b></p>
</div>

## Installation

```sh
$ cargo install cargo-fuzz
```

Note: `libFuzzer` needs LLVM sanitizer support, so this only works on x86-64 and Aarch64,
and only on Unix-like operating systems (not Windows). This also needs a nightly compiler since it uses some
 unstable command-line flags. You'll also need a C++ compiler with C++11 support.

## Usage

### `cargo fuzz init`

Initialize a `cargo fuzz` project for your crate!

### If your crate uses cargo workspaces, add `fuzz` directory to `workspace.members` in root `Cargo.toml`

`fuzz` directory can be either a part of an existing workspace (default)
or use an independent workspace. If latter is desired, you can use
`cargo fuzz init --fuzzing-workspace=true`.

### `cargo fuzz add <target>`

Create a new fuzzing target!

Use `cargo fuzz add <target> --structured` to generate an `Arbitrary`-based
structured input target.

### Dictionaries

Import an AFL/libFuzzer dictionary, word list, or directory:

```sh
cargo fuzz dict import ./path/to/sql.dict my_target
cargo fuzz dict list
```

The imported dictionary is stored at
`fuzz/dictionaries/<target>.dict` and automatically passed to libFuzzer as
`-dict=<absolute path>` by `cargo fuzz run my_target`. Use `--dict <path-or-name>`
for a one-off dictionary and `--no-default-dict` to disable automatic loading.
The old workflow remains available through passthrough arguments:
`cargo fuzz run my_target -- -dict=./my.dict`.

### Structured input generation

Generate deterministic corpus seeds from a small TOML schema:

```sh
cargo fuzz generate seeds my_target --schema fuzz/schemas/input.toml --count 32
cargo fuzz generate target my_target --schema fuzz/schemas/input.toml
```

Schemas describe fields (`bool`, `integer`, `float`, `string`, `choices`,
`tokens`, or `constant`) and an optional `template`. Without a template, seeds
are rendered as JSON objects. `generate target` creates a hand-written
`Arbitrary` implementation backed by `libfuzzer-sys`'s re-exported `arbitrary`
crate, so existing libFuzzer targets and corpora remain compatible.

### Coverage comparison

`cargo fuzz compare-coverage` converts multiple `llvm-cov export
--summary-only` reports into one Markdown or CSV table. A reproducible
three-crate baseline/dictionary/structured comparison lives in
[`benchmarks/coverage-compare`](benchmarks/coverage-compare/README.md).

### `cargo fuzz run <target>`

Run a fuzzing target and find bugs!

### `cargo fuzz fmt <target> <input>`

Print the `std::fmt::Debug` output for a test case. Useful when your fuzz target
takes an `Arbitrary` input!

### `cargo fuzz tmin <target> <input>`

Found a failing input? Minify it to the smallest input that causes that failure
for easier debugging!

### `cargo fuzz cmin <target>`

Minify your corpus of input files!

### `cargo fuzz coverage <target>`

Generate coverage information on the fuzzed program!

## Documentation

Documentation can be found in the [Rust Fuzz
Book](https://rust-fuzz.github.io/book/cargo-fuzz.html).

You can also always find the full command-line options that are available with
`--help`:

```sh
$ cargo fuzz --help
```

## Trophy case

[The trophy case](https://github.com/rust-fuzz/trophy-case) has a list of bugs
found by `cargo fuzz` (and others). Did `cargo fuzz` and libFuzzer find a bug
for you? Add it to the trophy case!

## License

`cargo-fuzz` is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](./LICENSE-APACHE) and [LICENSE-MIT](./LICENSE-MIT) for
details.
