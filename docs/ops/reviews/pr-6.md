# PR #6 검토 노트 — P0.5 측정기

- 검토일: 2026-10-07
- 대상 커밋: `4cafd3c`
- 결과: **반려(changes)** — 높음 2건. 둘 다 비교표에 들어갈 지표의 정의 문제다.
  작업자가 이 측정기로 P0.6 을 돌리고 있어서, 숫자가 쌓이기 전에 고친다.
- 재현 환경: 이 PC, rustc 1.99.0, Node v22.23.3, Chrome 155.0.8059.40

## 재현한 것

| 확인 | 결과 |
|---|---|
| fmt·clippy·`cargo test`(17, `safe_join` 숨김 경로 시험 포함) | 통과 |
| `npm ci` | 의존성 변화 없음(three 0.185.1, playwright-core 1.63.0) |
| 코드 검토 | `main.js`·`metrics.mjs`·`run.mjs`·`args.mjs`·`replay.rs`, 결정 0020 |
| JS 힙 지표가 무엇을 세는가 | 아래 H1 실험 |
| GPU 측정 재현(cold·orbit·replay) | **미실행.** 작업자의 P0.6 측정이 같은 GPU 에서 돌고 있어서(작업자 요청) 서로 수치를 흐리지 않도록 재검토 때 돌린다 |

## 반려 사유 (높음)

### H1. `mem_js_heap_max` 는 ArrayBuffer·typed array·wasm 메모리를 세지 않는다. 비교가 PointBlitz 쪽으로 기운다 (#3, INTENT "공정한 비교")

CDP `Performance.getMetrics` 의 `JSHeapUsedSize` 를 감독이 직접 실험했다. headless Chrome 155, `about:blank`:

| 할당 | JSHeapUsedSize |
|---|---:|
| 시작 | 1.0 MB |
| + `Float32Array` 200 MB | 0.6 MB |
| + `WebAssembly.Memory` 100 MB | 0.6 MB |
| + 숫자 1천만 개 JS 배열 | 325.9 MB |

- 기준 방식에서 이 값에 잡히는 것은 PLYLoader 의 중간 JS 배열뿐이다. 받은 67.6 MB `ArrayBuffer` 와 90 MB `Float32Array` 속성은 빠진다.
- 더 큰 문제: PointBlitz browser(wasm)는 데이터 전부가 wasm 선형 메모리와 typed array 에 있어 거의 0 으로 보인다.
  SPEC §6.2 `mem_cpu`("프로세스(또는 JS 힙) 최대")를 이 값으로 채우면 비교표의 메모리 행이 구조적으로 PointBlitz 에 유리해진다.
- 고칠 것:
  - `mem_cpu` 로 쓸 값은 ArrayBuffer·typed array·wasm 메모리를 포함해야 한다. 방법은 작업자가 정하고 결정 0020 에 적는다.
    예: 렌더러 프로세스의 private bytes / working set 을 OS 에서 표본 추출하거나, 재생 서버가 COOP/COEP 헤더를 보내 `performance.measureUserAgentSpecificMemory()` 를 쓴다.
  - 위 실험 같은 확인(큰 typed array 를 잡았을 때 지표가 그만큼 오르는지)을 시험이나 측정 기록으로 남긴다.
  - `JSHeapUsedSize` 는 이름을 그대로(`mem_js_heap_max`) 두고 보조 지표로만 쓴다.

### H2. `frame_time_p*` 이름으로 SPEC 과 다른 것을 기록한다 (#2·#10 예방)

- SPEC §6.2 `frame_time` 의 정의는 "p50 / p95 / p99, **CPU·GPU 따로**" 이고, 비교표 §6.4 에 "frame_time p95 (2.5 M pts)" 행이 있다.
- 이 PR 의 `frame_time_p*` 는 vsync 를 끈 rAF 간격이다. 결정 0020 스스로 "그리기 비용이 아니다"(약 0.1 ms) 라고 적었다.
  SPEC 정의에 맞는 것은 `render_time` / `render_cpu` / `render_gpu_estimate` 다.
- 이대로 P0.6 표를 만들면 SPEC 지표 이름 아래 다른 양이 들어간다.
- 고칠 것:
  - rAF 간격은 `frame_interval_p*` 같은 다른 이름으로 바꾼다.
  - SPEC `frame_time` 에는 동기화 프레임 값(총·CPU·GPU 추정)을 쓴다. 지표 이름 대응을 0020 "무엇을 재는가" 표에 적는다.
  - SPEC 수치·범위를 바꾸는 것은 아니다. SPEC 정의를 따르게 하는 것이다.

## 기록만 (중간·낮음)

| 수준 | 내용 |
|---|---|
| 중간 | `orbit` 의 동기화 루프는 시점마다 120 프레임을 한 JS 작업 안에서 돌린다(약 0.3 s 씩). 이것이 longtask 로 잡혀 `orbit` 의 `main_thread_block_*` 이 측정 인공물이 된다. `main_thread_block` 은 replay·cold 에서만 보고하거나 동기화 구간을 빼고 센다. |
| 중간 | `swap_frame_gpu_wait` 로 업로드가 보이는 것은 좋다. 하지만 `presented` 직전 readPixels 가 그 프레임의 파이프라이닝을 막는 효과도 함께 들어간다. 0020 대가 절에 "event_latency 는 동기화 1 회만큼 보수적" 이라고 한 줄 적는다. |
| 낮음 | `commit` 은 `git rev-parse --short HEAD` 만 본다. 작업 트리가 더러우면 `-dirty` 를 붙인다(`git describe --always --dirty`). |
| 낮음 | `device` 에 GPU 드라이버 버전이 없다. 비용 모델 계수에 영향이 있으므로 함께 기록한다. |
| 낮음 | 콘솔 오류를 거를 때 `'404'` 문자열을 포함한 모든 메시지를 버린다. favicon 은 고쳤으니 거르는 조건을 없애거나 좁힌다. |

## 잘 된 것

- PR #5 지적을 모두 반영했다. 교체 프레임만 동기화하고, 법선 36 B 를 반영하고, 숨김 경로를 거부하고, favicon 과 인자 해석을 정리했다.
- 결정 0020 의 "무엇을 재는가" 표와 PointBlitz 적용법이 명확하다.
