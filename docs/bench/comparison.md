# PointBlitz 대 기준 방식(three.js) — 같은 세션 비교 (P4.6 초안)

- **상태: 초안.** 공개(README·발표·패키지)는 소유자 결정(P4.8)이다. 이 문서의 수치는 아래 조건의 한 PC 측정값이며, 다른 하드웨어 값은 비용 모델(docs/bench/cost-model.md, V100 보정 대기)로 추정한다.
- 측정: 2026-10-08 15:38–15:59, `bash bench/suite.sh <ply> target/bench/p46`(main `6d3ce87`), 계획 [comparison-plan.md](comparison-plan.md) 그대로.
- 대상 6 — three.js(기준 방식, 밉맵 끔), native(wgpu Vulkan, 창), web(Chrome WebGPU, 기본 모듈), webs(WebGPU 전용 모듈), webgl(WebGL2), video(서버 영상: NVENC H.264 + WebCodecs, 쓰기 스레드 송신, 위상 잠금 끔). 시나리오 cold 5 회·orbit 5 회·replay ×60 3 회, 대상끼리 실행마다 번갈아.
- 장비: Windows 11, i7-13700F, RTX 4070 12 GB(드라이버 591.86), 60 Hz 화면, 서버와 클라이언트 같은 PC(루프백).
- 데이터: skyrecon `flight-01` 14 스냅샷(preview 7·refined 7), 마지막 2,502,015 점, PLY 합계 530.8 MB.

## 측정 유효성 (C1–C4, 결정 0038·0040·0041)

모든 실행은 `bench/gpu-watch.mjs` → `bench/measure-env.ps1`(측정 동안만 화면을 켜 두고, 측정 프로세스에만 Windows 전원 조절을 끔 — 시스템 설정 변경 없음)로 돌렸다. 실패·무효 실행은 같은 조건으로 1 회만 다시 돌리고 첫 시도 파일을 남긴다.

| 대상 | 유효 실행 | 다시 돌린 실행 | 화면 시계(C3) | CPU 보정(C4, 회/s) | 전원 조절 끈 프로세스(실행마다) |
|---|---:|---:|---|---|---|
| native | 13 | 1 | 59.57–60.34 Hz | 12633–12832 | 7–8 (실패 1) |
| three.js | 13 | 0 | 59.88–60.24 Hz | 12629–12831 | 14–15 (실패 1) |
| video | 13 | 0 | 59.88–59.88 Hz | 12625–12833 | 16–30 |
| web-webgl2 | 13 | 0 | 59.88–60.24 Hz | 12604–12844 | 14–19 (실패 2) |
| web-webgpu | 13 | 0 | 59.88–60.24 Hz | 12564–12806 | 14–15 (실패 10) |
| web-webgpu-only | 13 | 0 | 59.70–60.24 Hz | 12628–12833 | 14–15 (실패 6) |

C2 로 뺀 실행(contaminated/): 0 회.

- 다시 돌린 실행 1 회: native cold 4 — 첫 시도 화면 시계 59.41 Hz(C3 밖), 다시 59.57 Hz. native 의 화면 시계는 실행 끝의 vsync present 측정(120 장)이라 브라우저(rAF, 59.88)보다 조금 낮게 읽히는 경향이 있다(59.57–60.34).
- "전원 조절 끈 프로세스" 의 실패는 실행 끝(종료 중에 뜨고 바로 끝나는 Chrome 보조 프로세스)이나 실행 중 잠깐 뜬 보조 프로세스 1 개다. 각 대상의 주 프로세스(렌더러·GPU·서버·native)는 모든 실행에서 꺼졌다.

## 판정 (SPEC §7.1, 값 = 실행별 값의 중앙값)

오른쪽 열은 P3.4(2026-10-08 오전, **C4 미상**)를 같은 목표로 다시 판정한 것이다. 이전 판정은 지우지 않는다.

| 지표 | 시나리오 | 대상 | 목표 | 값(중앙값, 최소–최대, 유효 실행) | 판정 | P3.4(C4 미상): 값 / 판정(같은 목표) |
|---|---|---|---|---|---|---|
| event_latency preview p50 | replay×60 | native | ≤ 50 ms | 20.7 (15.8–20.7, 3 회) | 통과 | 20.1 (16.4–26.0, 3 회) / 통과 |
| event_latency preview p50 | replay×60 | web-webgpu | ≤ 50 ms | 26.8 (25.9–28.1, 3 회) | 통과 | 26.5 (26.3–27.5, 3 회) / 통과 |
| event_latency preview p50 | replay×60 | web-webgpu-only | ≤ 50 ms | 26.8 (26.3–31.9, 3 회) | 통과 | 26.5 (24.3–35.4, 3 회) / 통과 |
| event_latency preview p50 | replay×60 | web-webgl2 | ≤ 50 ms | 35.5 (35.1–46.9, 3 회) | 통과 | 35.7 (34.4–48.4, 3 회) / 통과 |
| event_latency refined p50 | replay×60 | native | ≤ 200 ms | 130.6 (125.4–163.3, 3 회) | 통과 | 141.8 (115.6–155, 3 회) / 통과 |
| event_latency refined p50 | replay×60 | web-webgpu | ≤ 200 ms | 121 (106.5–134.7, 3 회) | 통과 | 141.8 (125.4–171.8, 3 회) / 통과 |
| event_latency refined p50 | replay×60 | web-webgpu-only | ≤ 200 ms | 143.6 (123.9–147.2, 3 회) | 통과 | 127.9 (95.5–137, 3 회) / 통과 |
| event_latency refined p50 | replay×60 | web-webgl2 | ≤ 200 ms | 122.6 (110.1–180.5, 3 회) | 통과 | 128.5 (93.6–153.7, 3 회) / 통과 |
| cold 마지막 스냅샷 event_latency | cold | native | ≤ 300 ms | 185.7 (166.2–200.0, 5 회) | 통과 | 186.8 (172.7–204.7, 5 회) / 통과 |
| cold 마지막 스냅샷 event_latency | cold | web-webgpu | ≤ 300 ms | 133.1 (105.4–198.7, 5 회) | 통과 | 167.1 (87.3–218.1, 5 회) / 통과 |
| cold 마지막 스냅샷 event_latency | cold | web-webgpu-only | ≤ 300 ms | 165.5 (149.9–202.1, 5 회) | 통과 | 102.5 (100.4–151.3, 5 회) / 통과 |
| cold 마지막 스냅샷 event_latency | cold | web-webgl2 | ≤ 300 ms | 131.3 (114.8–198.2, 5 회) | 통과 | 164.9 (115–181.6, 5 회) / 통과 |
| main_thread_block 횟수 | replay×60 | web-webgpu | 0 회 | 0 (0–0, 3 회) | 통과 | 0 (0–0, 3 회) / 통과 |
| main_thread_block 횟수 | replay×60 | web-webgpu-only | 0 회 | 0 (0–0, 3 회) | 통과 | 0 (0–0, 3 회) / 통과 |
| main_thread_block 횟수 | replay×60 | web-webgl2 | 0 회 | 0 (0–1, 3 회) | 통과 | 1 (1–1, 3 회) / **미달** |
| main_thread_block 횟수 | cold | web-webgpu | 0 회 | 0 (0–0, 5 회) | 통과 | 0 (0–0, 5 회) / 통과 |
| main_thread_block 횟수 | cold | web-webgpu-only | 0 회 | 0 (0–0, 5 회) | 통과 | 0 (0–0, 5 회) / 통과 |
| main_thread_block 횟수 | cold | web-webgl2 | 0 회 | 1 (0–1, 5 회) | **미달** | 1 (0–1, 5 회) / **미달** |
| frame_time_total p50 | orbit | native | ≤ 1.7 ms | 1.55 (1.53–1.66, 5 회) | 통과 | 1.54 (1.52–1.55, 5 회) / 통과 |
| frame_time_total p50 | orbit | web-webgpu | ≤ 1.7 ms | 3.10 (3.10–3.20, 5 회) | **미달** | 3.30 (3–3.30, 5 회) / **미달** |
| frame_time_total p50 | orbit | web-webgpu-only | ≤ 1.7 ms | 3.10 (3.10–3.10, 5 회) | **미달** | 3.20 (3.10–3.20, 5 회) / **미달** |
| frame_time_total p50 | orbit | web-webgl2 | ≤ 1.7 ms | 3.50 (3.40–3.50, 5 회) | **미달** | 3.50 (3.50–3.60, 5 회) / **미달** |
| frame_time_total p95 | orbit | native | ≤ 1.9 ms | 1.81 (1.78–1.86, 5 회) | 통과 | 1.82 (1.75–1.84, 5 회) / 통과 |
| frame_time_total p95 | orbit | web-webgpu | ≤ 1.9 ms | 4.30 (4.20–4.40, 5 회) | **미달** | 4.40 (4.10–4.60, 5 회) / **미달** |
| frame_time_total p95 | orbit | web-webgpu-only | ≤ 1.9 ms | 4.20 (4.20–4.30, 5 회) | **미달** | 4.40 (4.30–4.50, 5 회) / **미달** |
| frame_time_total p95 | orbit | web-webgl2 | ≤ 1.9 ms | 6 (5.90–6.10, 5 회) | **미달** | 6 (5.90–6.10, 5 회) / **미달** |
| frame_time_total p99 | orbit | native | ≤ 2 ms | 2 (1.93–2.03, 5 회) | 통과 | 1.96 (1.89–2.04, 5 회) / 통과 |
| frame_time_total p99 | orbit | web-webgpu | ≤ 2 ms | 5 (4.80–5.04, 5 회) | **미달** | 5 (4.60–5, 5 회) / **미달** |
| frame_time_total p99 | orbit | web-webgpu-only | ≤ 2 ms | 4.90 (4.84–5, 5 회) | **미달** | 5 (4.90–5.10, 5 회) / **미달** |
| frame_time_total p99 | orbit | web-webgl2 | ≤ 2 ms | 13.5 (13.4–13.5, 5 회) | **미달** | 13.3 (12.3–13.5, 5 회) / **미달** |
| mem_cpu 최대 | replay×60 | native | 기록만 | 355.8 (352.0–364.5, 3 회) | 기록만 | 361.6 (357.8–371.0, 3 회) / 기록만 |
| mem_cpu 최대 | replay×60 | web-webgpu | 기록만 | 186.0 (172.9–186.6, 3 회) | 기록만 | 178.1 (170.4–187.5, 3 회) / 기록만 |
| mem_cpu 최대 | replay×60 | web-webgpu-only | 기록만 | 166.5 (166.1–174.1, 3 회) | 기록만 | 164.4 (160.0–167.0, 3 회) / 기록만 |
| mem_cpu 최대 | replay×60 | web-webgl2 | 기록만 | 215.1 (211.7–226.5, 3 회) | 기록만 | 213.4 (211.4–218.8, 3 회) / 기록만 |
| event_latency preview p50 (screen) | replay×60 | video | ≤ 80 ms | 22.7 (18.7–25, 3 회) | 통과 | 14.9 (12.6–25.1, 3 회) (event_latency_preview_p50, 그린 시각) / 통과 |
| event_latency refined p50 (screen) | replay×60 | video | ≤ 250 ms | 118.3 (105.9–125.5, 3 회) | 통과 | 116.2 (85–132.2, 3 회) (event_latency_refined_p50, 그린 시각) / 통과 |
| cold (screen) | cold | video | ≤ 350 ms | 155.7 (132.2–179.3, 5 회) | 통과 | 174.7 (109.6–207.9, 5 회) (event_latency_refined_p50, 그린 시각) / 통과 |
| 입력 → 표시 p50 (screen) | orbit | video | ≤ 50 ms | 22.8 (18.7–33, 5 회) | 통과 | 17.8 (16.7–19.4, 5 회) (input_latency_p50, 그린 시각) / 통과 |
| 입력 → 표시 p95 (screen) | orbit | video | ≤ 80 ms | 33.2 (22.6–36.4, 5 회) | 통과 | 22.6 (19.1–31.9, 5 회) (input_latency_p95, 그린 시각) / 통과 |
| 입력 → 표시 p50 (screen) | replay×60 | video | ≤ 50 ms | 26.8 (23.8–27.9, 3 회) | 통과 | 20.6 (20–21.7, 3 회) (input_latency_p50, 그린 시각) / 통과 |
| 입력 → 표시 p95 (screen) | replay×60 | video | ≤ 80 ms | 37.4 (33.4–41.0, 3 회) | 통과 | 30.1 (28.1–30.8, 3 회) (input_latency_p95, 그린 시각) / 통과 |
| 영상 프레임 빠짐 | orbit | video | ≤ 1 % | 0.40 (0.13–6.32, 5 회); 1 % 초과 2/5 회 | 통과 | 0.26 (0.26–4.86, 5 회); 1 % 초과 2/5 회 / 통과 |
| 클라이언트 main_thread_block 횟수 | replay×60 | video | 0 회 | 0 (0–0, 3 회) | 통과 | 0 (0–0, 3 회) / 통과 |
| 클라이언트 main_thread_block 횟수 | cold | video | 0 회 | 0 (0–0, 5 회) | 통과 | 0 (0–0, 5 회) / 통과 |
| event_latency preview p50 | replay×60 | three.js | — | 470.7 (456.5–472.9, 3 회) | 기준 방식 | 456.2 (449.2–475.6, 3 회) / 기준 방식 |
| event_latency refined p50 | replay×60 | three.js | — | 632.3 (631.9–640.7, 3 회) | 기준 방식 | 641.2 (625.8–710.4, 3 회) / 기준 방식 |
| cold 마지막 스냅샷 event_latency | cold | three.js | — | 904.2 (876.8–947.5, 5 회) | 기준 방식 | 909.1 (873.4–966.1, 5 회) / 기준 방식 |
| frame_time_total p50 / p95 / p99 | orbit | three.js | — | 1.70 / 1.90 / 2 (5 회) | 기준 방식 | 1.70 / 1.90 / 2.10 (5 회) / 기준 방식 |
| main_thread_block 횟수 | replay×60 | three.js | — | 14 (14–15, 3 회) | 기준 방식 | 14 (13–14, 3 회) / 기준 방식 |
| mem_cpu 최대 | replay×60 | three.js | — | 965.0 (964.6–965.8, 3 회) | 기준 방식 | 964.5 (964.0–968.2, 3 회) / 기준 방식 |

## 요약

| 대상 | 판정 |
|---|---|
| **native** | **모든 목표 통과**(frame_time p99 2.00 ms 는 목표 2.0 ms 와 같음 — 여유 없음) |
| **browser** WebGPU(두 모듈) | 지연·`main_thread_block` 통과, **frame_time 미달**(p50 3.1 ms 대 목표 1.7 ms) |
| **browser** WebGL2 | 지연 통과, **frame_time 미달**(p50 3.5, p99 13.5 ms), **cold `main_thread_block` 1 회 미달** |
| **server video** | **모든 목표 통과** — 단 프레임 빠짐은 중앙값 0.40 % 통과이나 **5 회 중 2 회가 1 % 초과(최대 6.32 %)** |
| three.js(기준 방식) | 판정 없음: preview / refined 471 / 632 ms, cold 904 ms, frame_time 1.70 / 1.90 / 2.0 ms, `main_thread_block` 14 회, `mem_cpu` 965 MB |

## 그 밖의 기록(목표 아님)

| 대상 | 받은 바이트 cold | 받은 바이트 replay ×60 | first_frame cold |
|---|---:|---:|---:|
| three.js | 67.6 MB(PLY) | 530.8 MB | 946.6 ms |
| native | 40.0 MB(청크) | 180.4 MB(preview 는 차분) | 594.0 ms |
| web-webgpu / webs / webgl | 40.0 MB | 180.4 MB | 309.4 / 328.7 / 268.3 ms |
| video | 1.26 MB(영상) | 10.3 MB | 180.3 ms |

## 각주

1. **browser frame_time 은 동기화 비용이다**: 판정 값은 결정 0020 의 "프레임마다 동기화"(batch=1)이다. 30 프레임에 한 번 동기화하면(batch=30, 진단 값) WebGPU 는 native 와 거의 같다 — 아래 "진단: 30 프레임마다 동기화" (C1–C4 로 다시 잼; P4.1 의 같은 진단은 C4 미상). WebGL2 는 비용이 CPU 쪽 GL 제출에 있다(P4.2). 공개 시에는 판정 값과 진단 값을 함께 싣는다(PR #30 검토).
2. **frame_time 기준값**은 SPEC §7.1 정정값(1.7 / 1.9 / 2.0 ms, 결정 0040·0041 의 재측정 2)이다. 같은 세션 three.js 는 1.70 / 1.90 / 2.0 ms 로 같았다. three.js 의 frame_time 은 세션의 GPU 클럭 상태에 따라 흔들린다(P4.4).
3. **서버 영상 프레임 빠짐**: 프레임은 사라지지 않는다(주기당 그린 프레임 1.00). 서버 틱(60 Hz)과 화면(약 59.94 Hz)의 속도 차로 도착 위상이 실행 중에 흘러 화면 갱신 경계를 지날 때 "빈 주기 + 두 장 주기" 가 생긴다(diagnostics.md P4.3, 결정 0042). 위상 잠금(`--phase-lock on`)은 이를 줄이지만 수용 기준을 못 맞춰 끔으로 확정했다.
4. **서버 영상 지연은 화면 기준**(결정 0040): 렌더링 갱신 시각까지. P3.4 열의 서버 영상 값은 "그린 시각" 기준이라 몇 ms 작다.
5. **요청 때 그리기**: native·browser 는 바뀐 것이 있을 때만 그린다(P1.5). orbit 의 frame_time 은 동기화 프레임으로 잰 것이다.
6. **native `first_frame`** 은 프로세스 시작부터(창·장치 만들기 포함), 브라우저는 탐색 시작부터다(PR #13) — 대상 사이에 직접 비교하지 않는다.
7. **화질**: 서버 영상 고정 시점 SSIM 0.9967(대 native 캡처, 점 영역 0.9846), native 대 three.js 점 영역 0.9554 — P3.4(C4 미상, 화질은 타이밍과 무관). 공개 wasm 크기는 CI 빌드 값(아래 "브라우저 모듈 크기", PR #21).
8. 메모리·대역폭은 목표가 아니라 기록이다(소유자 결정, INTENT 원칙 2).

## 브라우저 모듈 크기 (CI 빌드, PR #21)

GitHub Actions `wasm` 작업(ubuntu-latest, wasm-bindgen 0.2.129, 실행 37742566372, PR #47 커밋)의 값. gzip 은 `gzip -9`(전송 크기의 기준).

| 모듈 | wasm raw | wasm gzip | JS raw | JS gzip | 합계 gzip |
|---|---:|---:|---:|---:|---:|
| 기본(WebGPU + WebGL2 대체, `pointblitz-web`) | 4,277,741 B | 1,206,591 B | 122,246 B | 20,032 B | **1.23 MB** |
| WebGPU 전용(`pointblitz-web/webgpu`, 결정 0033) | 388,044 B | 96,958 B | 71,204 B | 13,974 B | **0.11 MB** |

- 비교를 위해: three.js 기준 방식 페이지가 받는 모듈(three 0.185.1 `three.module.js` + `three.core.js` + `PLYLoader.js`)은 2,115,078 B, gzip 417,594 B(**0.42 MB**). (baseline-three.md 의 "약 1.2 MB" 는 core 를 빼고 어림한 값이었다.)
- npm 이름 `pointblitz-web`·`pointblitz` 는 2026-10-08 기준 npm 레지스트리에 없다(배포는 P4.8).
- 모듈 크기는 한 번만 받는 비용이고, 위 판정의 `first_frame`·지연에는 루프백에서 받은 모듈 로드가 들어 있다.

## 진단: 30 프레임마다 동기화 (batch=30, C1–C4) — 판정 값 아님

2026-10-08 16:30–16:33, orbit 3 회씩, 같은 측정 규칙(실패·무효 1 회 다시 돌림). browser 세 대상은 번갈아 16:30–16:32 에, native 는 화면 시계 probe 를 고친 뒤 16:32–16:33 에 이어서 쟀다(아래).

| 대상 | frame_time_total p50 / p95 / p99 (batch=30) | 판정 값(batch=1, 위 표) |
|---|---|---|
| native | **1.37 / 1.56 / 1.61 ms** | 1.55 / 1.81 / 2.00 ms |
| web-webgpu | **1.43 / 1.61 / 1.65 ms** | 3.1 / 4.3 / 5.0 ms |
| web-webgpu-only | **1.43 / 1.57 / 1.64 ms** | 3.1 / 4.2 / 4.9 ms |
| web-webgl2 | 3.80 / 5.22 / 6.38 ms | 3.5 / 6.0 / 13.5 ms |

- WebGPU 의 그리기 자체는 native 와 거의 같고(p50 1.43 대 1.37 ms), 판정 값과의 차이는 매 프레임 동기화(1 픽셀 복사 + `mapAsync` 왕복) 비용이다. WebGL2 는 동기화를 나눠도 줄지 않는다 — 비용이 CPU 쪽 GL 제출에 있다(P4.2).
- native 의 화면 시계: 처음 시도 6 회가 C3 무효(62.8–63.8 Hz)였다. 원인은 측정 도구 쪽 — 화면 시계 probe 의 처음 몇 장은 비어 있는 스왑체인 이미지를 받아 화면을 기다리지 않는데, 오래 쉬던 orbit 뒤에는 첫 구간 전체가 약 63 Hz 로 읽혔다. probe 의 처음 10 장을 빼도록 고친 뒤(같은 PR) 다시 잰 native 3 회는 59.95–60.02 Hz(구간 59.95–60.03)였다. 무효 시도 파일은 남겼다.

## 보충 값 — P4.6 원 기록에서 집계한 값

판정표 밖의 값이며, 같은 측정(2026-10-08 15:38–15:59)의 원 기록을 `node baseline/three/aggregate.mjs <suite dir>` 로 집계했다(실행별 값의 중앙값; 원 기록은 저장소에 넣지 않는다). 성능 보고서([report.md](report.md))가 인용한다.

**스냅샷별 화면 반영 시간**(재생 ×60, 3 회 중앙값, ms)

| 스냅샷 | 종류 | 점 | three.js | web-webgpu | native | video |
|---:|---|---:|---:|---:|---:|---:|
| 1 | preview | 151,510 | 78.2 | 31.7 | 14.6 | 44.7 |
| 2 | refined | 336,118 | 165.3 | 33.0 | 45.9 | 24.8 |
| 3 | preview | 371,585 | 162.8 | 16.7 | 4.3 | 23.6 |
| 4 | refined | 749,216 | 304.7 | 61.7 | 68.5 | 42.5 |
| 5 | preview | 798,744 | 313.1 | 26.8 | 12.6 | 27.6 |
| 6 | refined | 1,192,254 | 445.9 | 106.5 | 84.0 | 99.5 |
| 7 | preview | 1,236,878 | 470.7 | 25.9 | 20.7 | 12.8 |
| 8 | refined | 1,640,611 | 632.3 | 112.8 | 130.6 | 103.6 |
| 9 | preview | 1,676,080 | 612.3 | 26.2 | 21.3 | 12.4 |
| 10 | refined | 2,059,581 | 740.0 | 125.3 | 171.5 | 167.0 |
| 11 | preview | 2,113,986 | 773.2 | 32.7 | 27.7 | 12.5 |
| 12 | refined | 2,404,412 | 883.9 | 153.9 | 210.5 | 164.9 |
| 13 | preview | 2,426,023 | 913.7 | 34.8 | 20.3 | 14.3 |
| 14 | refined | 2,502,015 | 931.4 | 162.5 | 222.8 | 175.1 |

(video 는 그린 시각 기준.) native 가 받은 바이트: 미리보기 차분 345,856–870,560 B, 정밀 전체 2,424,240–40,033,040 B.

**cold 단계별 시간**(5 회 중앙값): three.js 받기 114.7 ms, 해석 739.6 ms(메인 스레드 멈춤 739 ms) · native 서버 읽기 19.31 ms, 서버 전송 163.74 ms, 업로드 합계 10.35 ms(마지막 청크 뒤 0.48 ms), 다음 프레임 1.85 ms.

**재생 ×60 메모리**(3 회 중앙값): 렌더러/프로세스 최대 commit — three.js 964,980,736 B, native 355,827,712, web-webgpu 185,974,784, web-webgpu-only 166,469,632, web-webgl2 215,093,248, video 브라우저 46,260,224, video 서버 741,126,144 · JavaScript 힙 최대 — three.js 478,394,160(실행별 462,446,384 / 478,394,160 / 486,558,096), PointBlitz 브라우저 2.4–3.1 MB · three.js 재생 해석 합계 6,200.4 ms, 메인 스레드 멈춤 14 회 6,201 ms.

