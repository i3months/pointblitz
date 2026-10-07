# baseline/three

The current three.js approach, reproduced on PointBlitz's reference dataset (decisions 0005, 0019).
Benchmark only — not part of any PointBlitz package.

```
npm ci
# from the repository root, serve the dataset and the repository as web root:
cargo run --release -p pointblitz-bench -- replay --data <ply dir> --web . --port 8700
# then, in baseline/three:
node capture.mjs --server http://127.0.0.1:8700 --out ../../target/bench/baseline-three
node run.mjs --server http://127.0.0.1:8700 --scenario replay --speed 60 --metrics ../../target/bench/three-replay.jsonl
node run.mjs --server http://127.0.0.1:8700 --scenario cold --metrics ../../target/bench/three-cold.jsonl
node run.mjs --server http://127.0.0.1:8700 --scenario orbit --metrics ../../target/bench/three-orbit.jsonl
```

`orbit` measures synchronised render time per frame at the 8 fixed viewpoints (decision 0020).
Stop the replay server by its PID, never by image name — other sessions may run the same binary.
