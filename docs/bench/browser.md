# PointBlitz browser 측정 (P2.5)

기준 방식(three.js), native, browser 세 가지(WebGPU 기본 모듈 · WebGPU 전용 모듈 · WebGL2)를 **같은 세션에서 번갈아** 쟀다.
지표 정의는 결정 0020, 메모리는 결정 0032, 브라우저 측정 방법은 결정 0033, 목표는 SPEC §7.1.

## 0. 이번에 바뀐 것 (이전 기록과 비교할 때 주의)

| 바뀐 것 | 내용 | 결정 |
|---|---|---|
| 메모리 표본 시작 | 페이지로 가기 전에 같은 출처의 다른 주소를 열어 렌더러 프로세스를 만들고 표본을 시작한다(기준 방식도) | 0030 |
| `mem_cpu` 정의 | 250 ms 표본의 최대 → **OS 가 기록한 프로세스 최대 commit**(`PeakPagefileUsage`). 표본 최대는 `mem_cpu_sampled_max` 로 남김 | 0032 |
| `mem_cpu` 목표 | ≤ 300 MB → **기록만**(2026-10-08 소유자: "메모리는 무조건 많이 써도 된다"). P1.5 native 미달·0032 재측정이 알려진 뒤의 변경 | SPEC §7.1·§9 |
| 브라우저 동기화 프레임 | 화면 밖 텍스처에 그리고, WebGPU 는 1 픽셀 복사 + `mapAsync`, WebGL2 는 1 픽셀 `readPixels`(기준 방식과 같은 방법) | 0033 |

P0.6 · P1.5 문서의 `mem_cpu` 와 판정은 그때의 방법·목표로 남긴 기록이다(고치지 않음). 이 문서의 `mem_cpu` 는 모두 새 방법이다.

## 1. 조건

| 항목 | 값 |
|---|---|
| 데이터 | `flight-01`(14 스냅샷, 마지막 2,502,015 점) |
| 장비 | RTX 4070 12 GB(driver 591.86) · i7-13700F · Windows 11 · 전원 구성표 균형 조정 · 60 Hz |
| 브라우저 | Chrome 155 헤드리스, ANGLE D3D11, vsync 켬. WebGL2 는 `--disable-features=WebGPUService` |
| native | wgpu 30.0.1 Vulkan, 1920×1080, 요청 때 그리기 |
| 모듈 | 기본(WebGPU + WebGL2 대체) wasm 4,253,077 B · WebGPU 전용 wasm 385,107 B(`?pkg=webgpu`) |
| 반복 | cold 5 · orbit 5 · replay ×60 3, 다섯 구현을 번갈아(`bench/suite.sh`), 2026-10-08 02:21~02:33 |

재현: `bash bench/suite.sh <ply dir> target/bench/p25` → `node baseline/three/aggregate.mjs target/bench/p25`

## 2. SPEC §7.1 판정

목표: preview p50 ≤ 50 ms, refined p50 ≤ 200 ms(replay ×60), cold ≤ 300 ms, `main_thread_block` 0 회(replay ×60·cold, browser), `frame_time_total` p50/p95/p99 각각 ≤ 2.8 / 3.8 / 6.0 ms(P0.6 기준 방식). 메모리는 기록만.

### browser — WebGPU (기본 모듈)
| 지표 | 값(중앙값, 최소–최대) | 목표 | 판정 |
|---|---|---|---|
| preview p50 | 27.2 (26.5–33.4) ms | ≤ 50 | 통과 |
| refined p50 | 131.3 (109.4–173.9) ms | ≤ 200 | 통과 |
| cold | 118.7 (106.1–217.6) ms | ≤ 300 | 통과 |
| `main_thread_block` replay ×60 / cold | 0 / 0 회 | 0 | 통과 |
| `frame_time_total` p50 / p95 / p99 | 5.10 / 7.00 / 7.54 ms | ≤ 2.8 / 3.8 / 6.0 | **미달**(셋 다) |
| `mem_cpu` 렌더러 (replay ×60) | 183.8 MB, GPU 프로세스 285.9 MB | 기록만 | |

### browser — WebGPU 전용 모듈
| 지표 | 값 | 목표 | 판정 |
|---|---|---|---|
| preview p50 | 27.9 (25.9–32.8) ms | ≤ 50 | 통과 |
| refined p50 | 130.2 (107.4–157.3) ms | ≤ 200 | 통과 |
| cold | 153.1 (136.0–189.9) ms | ≤ 300 | 통과 |
| `main_thread_block` replay ×60 / cold | 0 / 0 회 | 0 | 통과 |
| `frame_time_total` p50 / p95 / p99 | 5.00 / 7.00 / 7.40 ms | ≤ 2.8 / 3.8 / 6.0 | **미달**(셋 다) |
| `mem_cpu` 렌더러 (replay ×60) | 169.7 MB, GPU 프로세스 303.2 MB | 기록만 | |

### browser — WebGL2 (WebGPU 끔)
| 지표 | 값 | 목표 | 판정 |
|---|---|---|---|
| preview p50 | 34.7 (34.2–35.0) ms | ≤ 50 | 통과 |
| refined p50 | 110.8 (93.7–119.7) ms | ≤ 200 | 통과 |
| cold | 148.1 (131.3–215.1) ms | ≤ 300 | 통과 |
| `main_thread_block` replay ×60 / cold | **1 / 1 회**(70~84 ms, 시작 때) | 0 | **미달** |
| `frame_time_total` p50 / p95 / p99 | 3.50 / 13.50 / 14.04 ms | ≤ 2.8 / 3.8 / 6.0 | **미달**(셋 다) |
| `mem_cpu` 렌더러 (replay ×60) | 222.7 MB, GPU 프로세스 345.4 MB | 기록만 | |

### native — 새 방법으로 다시 (참고, 같은 세션)
| 지표 | 값 | 목표 | 판정 |
|---|---|---|---|
| preview p50 / refined p50 / cold | 23.1 / 152.7 / 183.9 ms | ≤ 50 / 200 / 300 | 통과 |
| `frame_time_total` p50 / p95 / p99 | 1.55 / 1.83 / 1.99 ms | ≤ 2.8 / 3.8 / 6.0 | 통과 |
| `mem_cpu` (replay ×60) | 360.5 MB(새 방법; P1.5 이전 방법 348.6 MB) | 기록만 | |

### 같은 세션 three.js (참고)
preview 446.9 / refined 621.7 / cold 950.4 ms, `main_thread_block` replay ×60 14 회 · 6,091 ms, frame 2.7 / 3.6 / 6.0 ms, `mem_cpu` 968.7 MB(새 방법) · GPU 프로세스 492.7 MB, 받은 바이트 530,796,804 B(PointBlitz 180,433,296 B).

## 3. 미달 항목 분석

### `frame_time_total` (browser 세 가지 모두)
- 같은 장면을 같은 core 가 그린다. native 는 1.55 ms 이고 GPU 는 같으므로 차이는 **브라우저에서 "GPU 가 끝났다" 를 아는 비용**이 크다.
  - WebGPU 에는 막는 읽기가 없어 1 픽셀 복사 + `mapAsync` 를 기다린다. GPU 프로세스와의 왕복과 Promise 처리가 매 프레임 들어간다(cpu 쪽 호출은 p50 0.1 ms).
  - 기준 방식의 `readPixels` 도 왕복이지만 동기라 더 짧다.
- WebGL2 는 `render_offscreen` 호출 자체가 p50 3.5 ms 다(wgpu GL 백엔드가 CPU 에서 GL 호출로 옮기는 비용). p95·p99 의 13~14 ms 는 매 실행 같은 모양이며 원인은 아직 모른다.
- 판정은 목표대로 **미달**로 둔다. 원인 분리(WebGPU 타임스탬프 쿼리로 GPU 시간만 재기, WebGL2 의 꼬리)는 다음 작업 후보다.

### WebGL2 `main_thread_block` 1 회
- 시작 마크로 나눴다(cold 3 회): 긴 작업 75~80 ms 는 `wasm_init` 끝(전체 15~23 ms)과 **`Viewer.create` 의 대부분(71~76 ms, GL 에서는 동기)**에 걸친다. wasm 컴파일이 아니라 뷰어 생성(컨텍스트·장치·셰이더)이 원인이다.
- WebGPU 에서는 같은 단계가 65~100 ms 지만 비동기라 블록이 없다.
- 판정 전에 고치지 않았다(PR #18 검토). 고친다면 후보는 Worker + OffscreenCanvas.

## 4. 모듈 크기 A/B (같은 세션)

| | 기본 모듈 | WebGPU 전용 |
|---|---:|---:|
| wasm | 4,253,077 B | 385,107 B |
| `wasm_init`(받기·컴파일·인스턴스화, 루프백, cold 3 회) | 18.3~25.1 ms | 5.5~9.7 ms |
| `first_frame` cold (5 회 중앙값) | 299.8 ms | 306.5 ms |
| refined p50 / preview p50 (replay ×60) | 131.3 / 27.2 ms | 130.2 / 27.9 ms |

루프백에서는 크기 차이가 `wasm_init` 13 ms 정도이고 `first_frame` 은 흔들림 안이다. 실제 네트워크에서는 3.9 MB 를 더 받는다(100 Mbps 에서 약 0.3 초).
WebGPU 가 있으면 작은 모듈, 없으면 기본 모듈을 받는 선택 로더가 후보다(결정 0033 다시 볼 조건).

## 5. `navigator.gpu` 가 없을 때
초기화 스크립트로 `navigator.gpu` 를 지우고 확인했다. 기본 모듈의 `auto` 는 WebGL2(ANGLE)로 가서 cold 를 끝까지 그렸다.
WebGPU 전용 모듈은 "no WebGPU, and this module was built without WebGL2" 오류를 낸다.

## 6. 메모리 할당 설정 A/B (PR #20 검토 5)

메모리가 제약이 아니게 되어 `MemoryHints` 를 속도로 다시 골랐다. 같은 세션 A/B 에서 `memory` 와 `performance` 의 지연·프레임 차이는 실행 간 범위 안이고 방향도 엇갈려, 설정은 그대로 둔다(결정 0034).

## 7. 세션 간 흔들림 (three.js)

| 지표 | P0.6 | P1.5b | P2.5 |
|---|---:|---:|---:|
| cold event_latency | 882.2 | 888.2 | 950.4 |
| orbit frame_time p50 / p95 / p99 | 2.8 / 3.8 / 6.0 | 2.7 / 3.5 / 6.5 | 2.7 / 3.6 / 6.0 |
| replay ×60 preview p50 | 466.5 | 470.2 | 446.9 |
| replay ×60 refined p50 | 609.1 | 621.0 | 621.7 |
| `mem_cpu` replay ×60 | 860.7(표본) | 957.6(표본) | 968.7(OS 최대) |

cold 는 이번 세션이 약 7 % 높다. 구현 간 비교는 같은 세션 값으로만 한다.
