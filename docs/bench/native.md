# PointBlitz native 측정 (P1.5)

기준 방식(three.js)과 PointBlitz native 를 **같은 세션에서 번갈아** 쟀다(PR #10 검토). 측정 방법·지표 정의는 결정 0020, 목표는 SPEC §7.1, 고친 것은 결정 0027.

## 1. 조건

| 항목 | 값 |
|---|---|
| 데이터 | `flight-01`(14 스냅샷, 마지막 2,502,015 점) |
| 장비 | RTX 4070 12 GB(driver 591.86) · i7-13700F · Windows 11(10.0.26200) · 전원 구성표 균형 조정(`381b4222-…`) · 모니터 60 Hz |
| three.js | Chrome 155 headless, ANGLE D3D11, 60 Hz, vsync 켬, 밉맵 끔(결정 0025) |
| native | wgpu 30.0.1 Vulkan, 창 1920×1080, vsync(`AutoVsync`), 요청 때 그리기, `MemoryHints::MemoryUsage`(결정 0027) |
| 서버 | 루프백 `pointblitz-bench replay`. three.js 는 `/data`(PLY), native 는 `/chunks`(결정 0026) |
| 반복 | cold 5 · orbit 5 · replay ×60 3 · replay ×1 1, 구현을 번갈아 |
| 측정 시작 | 2026-10-08 00:26, GPU 대기 상태 P8 · 210 MHz · 40 °C(측정 전 `nvidia-smi`) |

재현:
```sh
bash bench/suite.sh <ply dir> target/bench/p15b                         # cold·orbit·×60, 번갈아
COLD=0 ORBIT=0 X60=0 X1=1 bash bench/suite.sh <ply dir> target/bench/p15b-x1
node baseline/three/aggregate.mjs target/bench/p15b
```

## 2. 결과와 목표 판정 (중앙값, 괄호는 최소–최대)

| 지표 | three.js (이 세션) | native | 목표(SPEC §7.1) | 판정 |
|---|---:|---:|---|---|
| replay ×60 `event_latency` preview p50 | 470.2 (458.9–481.3) ms | **25.8** (17.4–27.5) ms | ≤ 50 ms | 통과 |
| replay ×60 `event_latency` refined p50 | 621.0 (617.2–629.7) ms | **145.1** (135.0–147.9) ms | ≤ 200 ms | 통과 |
| cold 마지막 스냅샷 `event_latency` | 888.2 (876.7–903.2) ms | **168.6** (146.1–175.4) ms | ≤ 300 ms | 통과 |
| `frame_time_total` p50 / p95 / p99 (orbit) | 2.7 / 3.5 / 6.5 ms | **1.53 / 1.82 / 1.98** ms | 각각 ≤ 2.8 / 3.8 / 6.0 ms | 통과 |
| `mem_cpu` 최대 (replay ×60) | 957.6 MB (렌더러) | **348.6** (348.3–357.2) MB (프로세스) | ≤ 300 MB | **미달** |
| `main_thread_block` (replay ×60) | 14 회 | — | browser 만 | 해당 없음 |
| `bytes_total` (replay ×60) | 530,796,804 B | 180,433,296 B | 기록만 | |
| `bytes_total` (cold) | 67,554,652 B | 40,033,040 B | 기록만 | |
| GPU 메모리 | 90.1 MB(추정, 36 B/점) | 40.0 MB(실제 정점 버퍼, 16 B/점) | 기록만 | |
| `first_frame` (cold) | 1,015 ms(탐색 시작부터) | 559 ms(프로세스 시작부터, 창·장치 생성 포함) | 보고만 | |

replay ×60 의 preview·refined 최댓값: native 36.1 / 214.7 ms, three.js 898.4 / 921.0 ms.

**mem_cpu 미달**에 대해(결정 0027 "남은 것"):
- 데이터가 오기 전 74~102 MB, 첫 프레임 직후 약 236 MB 다(데이터 2.3 MB). 드라이버·런타임 고정 비용이 크다.
- 나머지는 refined 교체 동안 두 세대를 함께 들고 있는 비용이다.
- 같은 세션 A/B(3 회씩)에서 DX12 백엔드는 293 MB 로 목표 안이지만, 다른 지표가 조금씩 나빠진다. 그래서 백엔드는 바꾸지 않았다.
- three.js 의 수치는 렌더러 프로세스만이고 GPU 프로세스(307~518 MB)는 따로다. native 는 한 프로세스에 드라이버까지 들어 있다.

| 백엔드 A/B (replay ×60 · orbit, 3 회씩) | Vulkan(기본) | DX12 |
|---|---:|---:|
| preview p50 | 23.0 ms | 27.9 ms |
| refined p50 | 165.6 ms | 172.0 ms |
| frame_time p50 / p95 / p99 | 1.52 / 1.81 / 1.93 ms | 1.63 / 2.11 / 2.40 ms |
| mem_cpu (replay ×60) | 328.7 MB | 293.1 MB |

## 3. 시간은 어디에 쓰이나 (native, 중앙값)

클라이언트가 각 경계를 직접 기록한다(PR #12 검토 중간 1).

| 구간 | 뜻 | preview (×60) | refined (×60) | cold |
|---|---|---:|---:|---:|
| queue | 이벤트 받음 → 요청 보냄 | 0.08 | 0.07 | 0.04 |
| server_read | 요청 → 응답 헤더(서버가 스냅샷 파일을 읽음) | 2.93 | 17.13 | 19.17 |
| server_stream | 헤더 → 마지막 청크 받음(서버 인코딩·전송, 앞 청크는 그동안 업로드) | 4.17 | 89.39 | 144.98 |
| upload_tail | 마지막 청크 받음 → 장면에 들어감 | 0.64 | 2.76 | 0.50 |
| present | 장면에 들어감 → 표시(다음 프레임 + GPU 끝) | 10.68 | 22.54 | 3.44 |
| (upload_total) | 청크 검증·GPU 버퍼 생성에 쓴 시간 합(겹침) | 0.55 | 15.04 | 15.11 |

- **서버**(read + stream)가 refined 의 대부분이다. 인코딩은 병렬로 7 ms 이고, 나머지는 40 MB 전송이다.
  이 PC 루프백은 curl·정적 파일로도 350~500 MB/s 라 코드보다 전송이 바닥이다(결정 0027).
- **클라이언트**(upload_tail + present)는 preview 11 ms, refined 25 ms 다. present 에는 vsync 와 맞물리는 기다림이 들어간다.

### 차분 판정 (PR #12 검토 요청 ①②③)
- ① 전제: "이 preview 는 앞 스냅샷을 그대로 품는다" 는 **만드는 쪽(skyrecon)이 아는 메타데이터로 가정**한다.
  벤치 서버는 이를 서버 시작 때 바이트 비교로 한 번 구한다(`is_prefix`, 결정 0026).
- ② 요청마다 판정하면 드는 비용: 65 MB 스냅샷 둘을 읽어 비교하는 데 **43 ms**(P1.4 실측, seq 13). 지금의 preview p50(25.8 ms)보다 크다.
- ③ skyrecon 이 이 정보를 직접 내게 할지는 별도 기록으로 남긴다: [docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md).

## 4. 세션 간 흔들림 (three.js 기준 방식)

같은 코드·같은 PC 인데 세션마다 이만큼 달랐다(PR #10 검토 중간 1). 목표 판정은 P0.6 고정값으로 한다.

| 지표 | P0.6 (10-07) | PR #10 A/B (10-07) | P1.5a (10-08 00:0x) | P1.5b (10-08 00:2x, 이 표) |
|---|---:|---:|---:|---:|
| cold event_latency | 882.2 | 955.6 | 964.7 | 888.2 |
| orbit frame_time p50 / p95 / p99 | 2.8 / 3.8 / 6.0 | 2.8 / 4.6 / 6.0 | 3.0 / 4.5 / 6.5 | 2.7 / 3.5 / 6.5 |
| replay ×60 preview p50 | 466.5 | — | 960.1 | 470.2 |
| replay ×60 refined p50 | 609.1 | — | 1,370.4 | 621.0 |

P1.5a 의 replay ×60 은 다른 세션의 약 2 배였다. 같은 날 30 분 뒤(P1.5b)에는 P0.6 과 비슷했다. 원인은 찾지 못했다(GPU 클럭·전원 구성표는 같음).
흔들림이 이 정도이므로 **구현 간 비교는 같은 세션 번갈아 측정한 값으로만** 한다.

## 5. 만든 그대로(P1.5a) → 고친 뒤(P1.5b), native

| 지표 | P1.5a | P1.5b | 고친 것(결정 0027) |
|---|---:|---:|---|
| replay ×60 preview p50 | 56.8 ms | 25.8 ms | 요청 때 그리기, 차분 꼬리 읽기 |
| replay ×60 refined p50 | 153.4 ms | 145.1 ms | 병렬 인코딩 |
| cold event_latency | 217.8 ms | 168.6 ms | 병렬 인코딩, 메모리 우선 할당 |
| orbit frame_time p50 / p95 / p99 | 1.51 / 1.81 / 2.01 ms | 1.53 / 1.82 / 1.98 ms | — |
| mem_cpu (replay ×60) | 399.9 MB | 348.6 MB | 메모리 우선 할당 |

## 6. replay ×1

같은 세션, 구현마다 1 회(three.js 00:30, native 01:03 시작). 측정 전 GPU P8 · 210 MHz · 40 °C, 전원 구성표 균형 조정.

| 지표 | three.js ×1 | native ×1 | (참고) 같은 세션 ×60 three.js / native | (참고) P0.6 three.js ×1 |
|---|---:|---:|---:|---:|
| preview p50 / 최대 | 455.1 / 864.3 ms | **24.7 / 33.7 ms** | 470.2 / 25.8 ms | 873.9 / 1,658.9 ms |
| refined p50 / 최대 | 606.6 / 898.0 ms | **169.5 / 249.7 ms** | 621.0 / 145.1 ms | 881.2 / 1,610.8 ms |
| mem_cpu | 699.1 MB | 348.3 MB | 957.6 / 348.6 MB | 701.0 MB |
| main_thread_block | 14 회 | — | 14 회 | 16 회 |

- native 는 ×1 과 ×60 이 같은 수준이고 목표(preview ≤ 50, refined ≤ 200 ms) 안이다.
- **P0.6 의 "×1 이 ×60 보다 1.4~1.9 배 느리다" 는 이번에 재현되지 않았다.** 이 세션의 three.js ×1 은 ×60 과 같은 수준이다.
  P0.6 ×1 의 느림은 ×1 자체가 아니라 그 세션의 흔들림이었던 것으로 본다(§4 와 같은 크기). PR #7 검토의 미해결 항목을 이것으로 닫는다.
