#!/usr/bin/env python3
"""Run baseline/dictionary/structured fuzzing coverage for three crates.

The script intentionally uses only the public `cargo fuzz` CLI so that the
comparison also exercises libFuzzer compatibility.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path


BENCHMARK_DIR = Path(__file__).resolve().parent
REPO_ROOT = BENCHMARK_DIR.parents[1]
FIXTURE_CRATES = BENCHMARK_DIR / "crates"
CRATES = ("route_lite", "kv_record", "tag_lite")
STRATEGIES = ("baseline", "dictionary", "structured")


def run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True)


def output(command: list[str], *, env: dict[str, str] | None = None) -> str:
    return subprocess.check_output(command, env=env, text=True).strip()


def rust_host_triple(env: dict[str, str]) -> str:
    verbose = output(["rustc", "-vV"], env=env)
    for line in verbose.splitlines():
        if line.startswith("host: "):
            return line.split(": ", 1)[1]
    raise RuntimeError("could not determine Rust host triple")


def rustlib_bin(env: dict[str, str]) -> Path:
    sysroot = Path(output(["rustc", "--print", "sysroot"], env=env))
    host = rust_host_triple(env)
    path = sysroot / "lib" / "rustlib" / host / "bin"
    if not (path / "llvm-profdata").exists():
        raise RuntimeError(
            f"llvm-profdata was not found in {path}; run "
            "`rustup component add llvm-tools-preview`"
        )
    return path


def cargo_fuzz_binary() -> Path:
    binary = REPO_ROOT / "target" / "debug" / "cargo-fuzz"
    if not binary.exists():
        run(["cargo", "build", "--manifest-path", str(REPO_ROOT / "Cargo.toml")])
    return binary


def prepare_crate(name: str, work_dir: Path, cargo_fuzz: Path, env: dict[str, str]) -> Path:
    source = FIXTURE_CRATES / name
    destination = work_dir / "crates" / name
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(source, destination)

    run([str(cargo_fuzz), "init", "--fuzzing-workspace=true"], cwd=destination, env=env)
    target = f"{name}_fuzz"
    run([str(cargo_fuzz), "add", target], cwd=destination, env=env)
    harness = destination / "fuzz" / "fuzz_targets" / f"{target}.rs"
    harness.write_text(
        "#![no_main]\n"
        "use libfuzzer_sys::fuzz_target;\n\n"
        f"fuzz_target!(|data: &[u8]| {{ let _ = {name}::check(data); }});\n",
        encoding="utf-8",
    )
    return destination


def run_strategy(
    strategy: str,
    name: str,
    crate_dir: Path,
    work_dir: Path,
    cargo_fuzz: Path,
    runs: int,
    fuzz_target_dir: Path,
    env: dict[str, str],
) -> Path:
    target = f"{name}_fuzz"
    corpus = work_dir / "corpora" / name / strategy
    corpus.mkdir(parents=True, exist_ok=True)

    command = [
        str(cargo_fuzz),
        "run",
        "--sanitizer=none",
        f"--target-dir={fuzz_target_dir}",
    ]

    if strategy == "dictionary":
        dictionary = FIXTURE_CRATES / name / "dictionary.dict"
        command.append(f"--dict={dictionary}")
    else:
        command.append("--no-default-dict")

    if strategy == "structured":
        schema = FIXTURE_CRATES / name / "schema.toml"
        run(
            [
                str(cargo_fuzz),
                "generate",
                "seeds",
                target,
                "--schema",
                str(schema),
                "--out",
                str(corpus),
                "--count=24",
                "--force",
            ],
            cwd=crate_dir,
            env=env,
        )

    command.extend(
        [
            target,
            str(corpus),
            "--",
            f"-runs={runs}",
            "-seed=1",
            "-max_len=1024",
            "-print_final_stats=1",
        ]
    )
    run(command, cwd=crate_dir, env=env)
    return corpus


def collect_coverage(
    name: str,
    strategy: str,
    crate_dir: Path,
    corpus: Path,
    work_dir: Path,
    cargo_fuzz: Path,
    coverage_target_dir: Path,
    llvm_bin: Path,
    triple: str,
    env: dict[str, str],
) -> Path:
    target = f"{name}_fuzz"
    run(
        [
            str(cargo_fuzz),
            "coverage",
            "--sanitizer=none",
            f"--target-dir={coverage_target_dir}",
            "--llvm-path",
            str(llvm_bin),
            target,
            str(corpus),
        ],
        cwd=crate_dir,
        env=env,
    )

    profdata = crate_dir / "fuzz" / "coverage" / target / "coverage.profdata"
    binary = coverage_target_dir / triple / "release" / target
    if not binary.exists():
        raise FileNotFoundError(f"coverage binary not found: {binary}")

    report_path = work_dir / "reports" / f"{name}-{strategy}.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    with report_path.open("w", encoding="utf-8") as report:
        subprocess.run(
            [
                str(llvm_bin / "llvm-cov"),
                "export",
                "--summary-only",
                f"--instr-profile={profdata}",
                str(binary),
            ],
            check=True,
            env=env,
            stdout=report,
        )
    return report_path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=3000)
    parser.add_argument("--work-dir", type=Path, default=REPO_ROOT / "target" / "coverage-compare")
    parser.add_argument("--output-md", type=Path, default=BENCHMARK_DIR / "coverage-comparison.md")
    parser.add_argument("--output-csv", type=Path, default=BENCHMARK_DIR / "coverage-comparison.csv")
    args = parser.parse_args()

    env = os.environ.copy()
    env.setdefault("CARGO_TERM_COLOR", "never")
    cargo_fuzz = cargo_fuzz_binary()
    llvm_bin = rustlib_bin(env)
    triple = rust_host_triple(env)

    if args.work_dir.exists():
        shutil.rmtree(args.work_dir)
    args.work_dir.mkdir(parents=True)

    fuzz_target_dir = args.work_dir / "fuzz-target"
    coverage_target_dir = args.work_dir / "coverage-target"
    reports: list[Path] = []

    for name in CRATES:
        crate_dir = prepare_crate(name, args.work_dir, cargo_fuzz, env)
        for strategy in STRATEGIES:
            corpus = run_strategy(
                strategy,
                name,
                crate_dir,
                args.work_dir,
                cargo_fuzz,
                args.runs,
                fuzz_target_dir,
                env,
            )
            report = collect_coverage(
                name,
                strategy,
                crate_dir,
                corpus,
                args.work_dir,
                cargo_fuzz,
                coverage_target_dir,
                llvm_bin,
                triple,
                env,
            )
            reports.append(report)

    markdown_command = [
        str(cargo_fuzz),
        "compare-coverage",
        "--expect-three-crates",
        "--format=markdown",
    ]
    csv_command = [str(cargo_fuzz), "compare-coverage", "--format=csv"]
    for name, strategy, report in zip(
        [name for name in CRATES for _ in STRATEGIES],
        STRATEGIES * len(CRATES),
        reports,
        strict=True,
    ):
        spec = f"{name}:{strategy}:{report}"
        markdown_command.extend(["--report", spec])
        csv_command.extend(["--report", spec])

    markdown = subprocess.check_output(markdown_command, env=env, text=True)
    csv_text = subprocess.check_output(csv_command, env=env, text=True)
    args.output_md.write_text(
        "# Three-crate coverage comparison\n\n"
        f"Command: `python3 benchmarks/coverage-compare/run.py --runs {args.runs}`\n\n"
        "Strategies use identical libFuzzer run budgets and a fixed seed. "
        "`dictionary` uses the crate's standard libFuzzer dictionary; "
        "`structured` uses deterministic schema-generated corpus seeds.\n\n"
        + markdown,
        encoding="utf-8",
    )
    args.output_csv.write_text(csv_text, encoding="utf-8")
    print(f"\nWrote {args.output_md}")
    print(f"Wrote {args.output_csv}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
