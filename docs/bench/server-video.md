# PointBlitz server video 측정 (P3.4) + 같은 세션 전체 비교

three.js · native · browser 세 가지(WebGPU 기본 모듈 · WebGPU 전용 모듈 · WebGL2) · server video 를 **같은 세션에서 번갈아** 쟀다(2026-10-08 10:52:26 ~ 11:10:36).
정의·유효성 규칙은 측정 전에 고정한 결정 0038, 목표는 SPEC §7.1(서버 영상 목표는 2026-10-08 소유자 결정).

## 1. 조건

| 항목 | 값 |
|---|---|
| 데이터 | `flight-01`(14 스냅샷, 마지막 2,502,015 점) |
| 장비 | RTX 4070 12 GB(driver 591.86) · i7-13700F · Windows 11 · 전원 구성표 균형 조정 · 60 Hz |
| 브라우저 | Chrome 155 헤드리스, ANGLE D3D11, vsync 켬 |
| server video | `pointblitz-server serve`(헤드리스 wgpu Vulkan 렌더 → NVENC H.264, QP 18, P1 + 초저지연, B 프레임 없음, VUI bitstream_restriction) → WebSocket(루프백) → 브라우저 WebCodecs(하드웨어) → 캔버스 2D. **서버와 브라우저가 같은 PC·같은 GPU** |
| 반복 | cold 5 · orbit 5 · replay ×60 3, 여섯 구현을 번갈아(`bench/suite.sh`) |
| 유효성 | 실행마다 GPU 기록(`bench/gpu-watch.mjs`), 오염 기준 C1(실행 직전 GPU 사용률 > 15 %)·C2(서버 렌더·인코딩 p50 > 같은 시나리오 중앙값 × 2). **78 실행 중 오염 0**(`target/bench/p34/contamination.json`) |

재현(저장소 루트에서):
```sh
bash bench/suite.sh <ply dir> target/bench/p34        # 여섯 구현, cold·orbit·×60
node bench/contamination.mjs target/bench/p34          # 오염 판정(--apply 로 빼냄)
node baseline/three/aggregate.mjs target/bench/p34
# 화질: 재생 서버 --web . → pointblitz-server serve --replay … --port P --cold --wait-for-client
node bench/web/video-capture.mjs --server http://127.0.0.1:<replay> --ws ws://127.0.0.1:<P> --out target/bench/p34/video-views
pointblitz-bench compare target/bench/pointblitz-native target/bench/p34/video-views --viewpoints bench/viewpoints/flight-01.json
```
알려진 것: 이번 suite 는 자료를 모두 쓴 뒤 종료 코드 1 로 끝났다(로그에 오류 없음, 78 실행·표 정상). 끝날 때 재생 서버를 멈추는 부분으로 보이며 따로 고친다.

## 2. SPEC §7.1 서버 영상 판정 (중앙값, 괄호는 최소–최대)

| 지표 | 값 | 목표 | 판정 |
|---|---|---|---|
| `event_latency` preview p50 (×60) | 14.9 (12.6–25.1) ms | ≤ 80 ms | 통과 |
| `event_latency` refined p50 (×60) | 116.2 (85.0–132.2) ms | ≤ 250 ms | 통과 |
| cold | 174.7 (109.6–207.9) ms | ≤ 350 ms | 통과 |
| 입력 → 표시 p50 / p95 | orbit 17.8 / 22.6, replay 20.6 / 30.1, cold 17.3 / 24.3 ms | ≤ 50 / 80 ms | 통과(세 시나리오 모두) |
| 프레임 빠짐(orbit, 첫 프레임 ~ orbit 끝) | **0.26 %** (0.26–4.86 %) | ≤ 1 % | 통과(실행 중앙값 — 기존 집계 규칙). **실행의 약 40 % 가 1 % 초과**: 이 측정 5 회 + 감독 재현 4 회(1.85, 0.13, 0.39, 15.23 %) = 9 회 중 4 회, 최대 15.23 %. 각주: 주기당 그린 프레임 1.00 장 — 손실이 아니라 서버 틱과 화면 vsync 의 위상(diagnostics.md P4.3) |
| 화질 SSIM(고정 시점 8곳, 대 native 캡처) | **0.9967**(최저 0.9934 close_dense) | ≥ 0.95 | 통과 |
| 클라이언트 `main_thread_block` | 0 / 0 / 0 회(cold·orbit·×60) | 0 | 통과 |

- 진단: 점 영역 SSIM 0.9846(점이 있는 창만), 인코딩 전 서버 프레임 대비도 같은 값(서버 렌더 = native 렌더). 참고로 native 대 three.js 는 전체 0.9919, 점 영역 0.9554.
- 응답 없는 입력: replay·cold 에서 1 개씩 — 서버가 끝날 때 아직 오가던 마지막 입력(측정 끝 처리). orbit 은 0.
- 서버: 예산(16.7 ms) 넘은 틱 0~0.38 %. 렌더 p50 3.8 ms(cold) ~ 6.1 ms(×60), 인코딩 p50 2.8 ~ 5.7 ms — 브라우저 디코딩·다른 실행과 GPU 를 나누는 구간에서 커진다.
- 프레임 빠짐 4.86 % 실행은 서버 늦은 틱이 0.13 % 로 낮았다 — 빠짐은 서버가 아니라 클라이언트 쪽(디코딩 출력 시점과 화면 주기의 맞물림)에서 생긴다. replay ×60 의 빠짐 2.5~5.1 %(보고만)도 같은 모양이다. 원인 분리는 다음 작업 후보.
- 입력 → 표시 최대 512.9 ms(감독 실행, PR #27)는 이번 측정에서 재현되지 않았다(최대 37.7 ms).

### 2.1 고친 뒤(B: 클라이언트 한 프레임 버퍼, 결정 0039) — 위 판정 행은 그대로 둔다

같은 세션(2026-10-08 13:15–13:21), buffer / immediate 번갈아 orbit 5 회씩 + replay ×60 2 회씩, `gpu-watch` 오염 0, 실패·재시도 0. immediate = 위 판정과 같은 동작. 값은 중앙값(괄호는 최소–최대).

| 지표 | immediate | **buffer(B)** | 목표 |
|---|---|---|---|
| 프레임 빠짐 orbit | 5.50 % (0.97–8.03) — 5 회 중 4 회 > 1 % | **0.28 % (0.26–0.65)** — 0 회 > 1 % | ≤ 1 % |
| 두 장 주기 orbit(실행별) | 34–61 | **0** | — |
| 입력 → 표시 p50 orbit | 17.5 (16.4–22.3) ms | **33.3 (33.3–41.3) ms** | ≤ 50 |
| 입력 → 표시 p95 orbit | 24.4 (17.6–27.6) ms | **39.2 (34.6–53.7) ms** | ≤ 80 |
| 프레임 빠짐 replay ×60(보고만) | 2.60 % | 1.23 % (1.20–1.25) | — |
| 입력 → 표시 p50 replay ×60 | 20.0 ms | **45.1 (43.7–46.5) ms** | ≤ 50 |
| 입력 → 표시 p95 replay ×60 | 27.9 (26.9–29.0) ms | **54.4 (53.3–55.5) ms** | ≤ 80 |
| `event_latency` preview p50 (×60) | 13.1 ms | 39.9 (36.8–42.9) ms | ≤ 80 |
| `event_latency` refined p50 (×60) | 110.5 ms | 113.6 ms | ≤ 250 |

- **끊김은 위상과 무관하게 사라졌다**(orbit 두 장 주기 0, 남은 빈 주기 2–5 개는 시작과 버퍼가 빈 순간). immediate 는 이 세션에서 5 회 중 4 회가 1 % 를 넘었다(위상 운).
- **대가는 orbit 에서 약 한 화면 주기(+15.8 ms p50), replay 에서 약 1.5 주기(+25 ms p50)**. replay 는 서버가 스냅샷을 바꾸는 동안 도착이 더 흔들려 큐가 두 장으로 머무는 때가 많다. replay 입력 → 표시 p50 45.1 ms 는 목표(50) 안이지만 여유가 작다.
- orbit 5 번째 쌍은 두 방식 모두 화면 주기가 적게 셌다(720–723 대 757–764) — 이 PC 의 화면 쪽 일시 현상으로 보고, buffer 는 이때 쌓인 프레임 42 장을 버렸다(`pacing_dropped`).

## 3. 같은 세션 전체 비교 (중앙값)

| 지표 | three.js | native | browser WebGPU | browser WebGPU 전용 | browser WebGL2 | server video |
|---|---:|---:|---:|---:|---:|---:|
| preview p50 (×60) | 456.2 ms | 20.1 ms | 26.5 ms | 26.5 ms | 35.7 ms | **14.9 ms** |
| refined p50 (×60) | 641.2 ms | 141.8 ms | 141.8 ms | 127.9 ms | 128.5 ms | **116.2 ms** |
| cold | 909.1 ms | 186.8 ms | 167.1 ms | **102.5 ms** | 164.9 ms | 174.7 ms |
| `frame_time_total` p50 / p95 / p99 (orbit) | 1.70 / 1.90 / 2.10 | **1.54 / 1.82 / 1.96** | 3.30 / 4.40 / 5.00 | 3.20 / 4.40 / 5.00 | 3.50 / 6.00 / 13.34 | (서버 렌더 p50 5.8 ms) |
| `main_thread_block` (×60) | 14 회 | — | 0 | 0 | 1 | 0 |
| 받은 바이트 (×60 전체) | 530.8 MB | 180.4 MB | 180.4 MB | 180.4 MB | 180.4 MB | **10.3 MB** |
| 받은 바이트 (orbit 12 초 회전 포함) | 67.6 MB | 40.0 MB | 40.0 MB | 40.0 MB | 40.0 MB | 113.0 MB |
| 클라이언트 메모리 `mem_cpu` (×60, 기록) | 964.5 MB | 361.6 MB | 178.1 MB | 164.4 MB | 213.4 MB | **45.0 MB** |
| GPU 프로세스(브라우저) / 서버 프로세스 (×60) | 476.4 MB | — | 286.6 MB | 277.5 MB | 339.7 MB | 238.7 MB / 서버 746.0 MB |

- server video 의 바이트는 화면 변화량에 따른다: 화면이 멈춰 있으면 거의 0, 회전하면 QP 18 에서 약 75 Mbps(P3.1). 대역폭은 제약이 아니다(INTENT 원칙 2).
- 클라이언트 쪽 일(받기·디코딩)이 가장 가볍다 — 렌더 비용은 서버로 간다.

## 4. 세션 간 흔들림 (three.js)

| 지표 | P0.6 | P1.5b | P2.5 | P3.4 |
|---|---:|---:|---:|---:|
| cold | 882.2 | 888.2 | 950.4 | 909.1 |
| orbit frame_time p50 / p95 / p99 | 2.8 / 3.8 / 6.0 | 2.7 / 3.5 / 6.5 | 2.7 / 3.6 / 6.0 | **1.70 / 1.90 / 2.10** |
| replay ×60 preview / refined p50 | 466.5 / 609.1 | 470.2 / 621.0 | 446.9 / 621.7 | 456.2 / 641.2 |

이번 세션의 three.js 프레임 시간이 이전 세션보다 크게 짧다(p99 6.0 → 2.1 ms). 같은 세션의 native(1.54 ms)는 이전과 같아, GPU 가 아니라 Chrome·ANGLE 쪽 상태로 보인다. 원인은 찾지 못했다. **목표 판정은 SPEC 대로 P0.6 고정값(2.8 / 3.8 / 6.0)으로** 하므로 browser 의 frame_time 판정(P2.5 미달)은 그대로다.
