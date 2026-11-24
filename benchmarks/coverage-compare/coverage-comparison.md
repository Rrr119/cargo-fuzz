# Three-crate coverage comparison

Command: `python3 benchmarks/coverage-compare/run.py --runs 3000`

Strategies use identical libFuzzer run budgets and a fixed seed. `dictionary` uses the crate's standard libFuzzer dictionary; `structured` uses deterministic schema-generated corpus seeds.

| Crate | Strategy | Lines covered | Line coverage | Regions covered | Region coverage |
|---|---:|---:|---:|---:|---:|
| route_lite | baseline | 86/194 | 44.3% | 96/224 | 42.9% |
| route_lite | dictionary | 92/194 | 47.4% | 103/224 | 46.0% |
| route_lite | structured | 166/194 | 85.6% | 188/224 | 83.9% |
| kv_record | baseline | 72/159 | 45.3% | 83/223 | 37.2% |
| kv_record | dictionary | 88/159 | 55.3% | 105/223 | 47.1% |
| kv_record | structured | 123/159 | 77.4% | 159/223 | 71.3% |
| tag_lite | baseline | 53/140 | 37.9% | 59/177 | 33.3% |
| tag_lite | dictionary | 101/140 | 72.1% | 115/177 | 65.0% |
| tag_lite | structured | 119/140 | 85.0% | 150/177 | 84.7% |
