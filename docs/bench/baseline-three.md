# 기준 방식(three.js) 측정 — P0.6

- 날짜: 2026-10-07
- 대상: 기준 방식(결정 0005·0019) — three.js 0.185.1 `PLYLoader` + `Points`, 이벤트마다 전체 다운로드·메인 스레드 해석·전체 교체
- 데이터: `flight-01`(14 스냅샷, 마지막 2,502,015 점 · 67,554,652 B)
- 측정 방법: 결정 0020(동기화 프레임, OS 프로세스 메모리, longtask), 결정 0021(vsync 켬)
- 장비: RTX 4070 12 GB(드라이버 591.86) · ANGLE D3D11 · Chrome 155.0.8059.40 · Intel i7-13700F · Windows 11(10.0.26200) · 헤드리스 60 Hz · 전원 구성표 균형 조정(`381b4222-f694-41f0-9685-ff5bb260df2e`, 측정 뒤 확인 — 이후 측정은 `device.power_plan` 에 기록)
- 측정 커밋: `78ed45c` · 서버는 루프백(네트워크 시간 없음 — 결정 0011 로 따로 더한다)

> **C3 감사(2026-10-08, 결정 0040, [validity-audit.md](validity-audit.md))**: 이 측정의 14 회 중 11 회(cold 5/5, orbit 5/5, replay ×60 1/4)가 **화면 시계 범위 밖(56.2–58.1 Hz)** 이었다 — 밤 측정 중 디스플레이가 꺼져 있었던 것으로 본다. 아래 숫자는 지우지 않고 그대로 두며, keep-display 로 다시 잰 값을 별도 행으로 싣는다. SPEC §7.1 frame_time 목표의 정정은 감독이 정한다(소유자 위임).

## 요약 — 비교표 three.js 열 (중앙값)

| 지표 | 값 | 시나리오 · 실행 수 | 비고 |
|---|---:|---|---|
| bytes_total | 530,796,804 B | replay ×60 · 3 | 스냅샷 14개 전체를 매번 받는다 |
| event_latency (preview, p50) — ×60 | 466.5 ms | replay ×60 · 3 | 최대 875 ms |
| event_latency (refined, p50) — ×60 | 609.1 ms | replay ×60 · 3 | 최대 920 ms |
| event_latency (preview / refined, p50) — ×1 | 873.9 / 881.2 ms | replay ×1 · **1** | ×60 의 1.9 / 1.4 배, 원인 미확인(아래) |
| first_frame (cold) | 990.1 ms | cold · 5 | 마지막 스냅샷 하나 |
| frame_time_total p50 / p95 / p99 (2.5 M) | 2.8 / 3.8 / 6.0 ms | orbit · 5 | 동기화 프레임, 시점 8곳 |
| frame_time_cpu p50 | < 0.1 ms | orbit · 5 | `performance.now()` 해상도 0.1 ms |
| main_thread_block | 16 회 · 6,305 ms | replay ×60 · 3 | 50 ms 넘는 작업 합 |
| main_thread_block (cold) | 3 회 · 837 ms | cold · 5 | |
| frame_interval_max | 816.7 ms | replay ×60 · 3 | 가장 긴 멈춤(해석 중) |
| mem_cpu (렌더러 private) | 860.7 MB | replay ×60 · 3 | cold 597.2 MB |
| mem_gpu (추정) | 90.1 MB | 모두 | 2.5 M 점 × 36 B(위치·법선·색 float32) |

### 재측정(2026-10-08 13:52–13:57, C3 준수) — 위 표는 그대로 둔다

> **C4 미상(결정 0041)**: 이 재측정에는 CPU 상태 기록이 없고, replay ×60 의 해석이 절반 속도였던 것은 Windows 전원 조절(EcoQoS)로 확인됐다. C1–C4 를 지켜 다시 잰 값은 아래 "재측정 2" 다.

| | P0.6(2026-10-07, 위 표) | **재측정** |
|---|---|---|
| 기준 방식 | 원판 텍스처 **밉맵 켬** | **밉맵 끔**(결정 0025, PR #10 이후의 기준 방식 — SSIM·화면 동등성의 기준) |
| 화면 시계(C3) | 14 회 중 11 회 범위 밖(56.2–58.1 Hz) | 15 회 모두 범위 안(59.88–60.24 Hz), keep-display |
| GPU 클럭(orbit 중앙) | 기록 없음 | 2,490–2,775 MHz(P8/P5 ↔ P0) |
| **frame_time_total p50 / p95 / p99**(orbit · 5) | 2.8 / 3.8 / 6.0 ms | **1.9 / 3.2 / 3.6 ms**(실행별 p50 1.8–2.0, p99 3.4–5.4) |
| first_frame(cold · 5) | 990.1 ms | 1,036 (1,013–1,136) ms |
| event_latency refined p50(cold · 5) | 882.2 ms | 982.1 (960.3–1,010) ms |
| event_latency preview / refined p50(×60) | 466.5 / 609.1 ms(3 회) | **쓰지 않음**(아래) — 1,048.6 / 1,358.8 ms(5 회) |

- 같은 코드 경로: 재측정은 `main`(결정 0025 이후) — P0.6 이후 바뀐 기준 방식의 기본 경로는 밉맵 끔 하나이고, 나머지(batch·memspike)는 선택 옵션이다.
- frame_time 은 P3.4(1.70 / 2.00 / 2.20, 낮 세션)·P4.4(1.70 / 2.00 / 2.20)와 가깝다. P4.4 에서 본 대로 three.js 의 frame_time 은 GPU 클럭 상태에 따라 흔들린다(이번은 높은 클럭).
- **replay ×60 의 event_latency 는 이번 세션 값으로 쓰지 않는다.** 같은 14 개 스냅샷·같은 530.8 MB 인데, 해석 속도가 셋째 스냅샷부터 약 43 MB/s 로 P3.4(85 MB/s)의 절반이었고 받기도 느렸다(합계 3.5 대 0.8 s). cold 의 해석 속도(93 MB/s)는 그대로다. gpu-watch·keep-display 를 빼고 돌려도 같았고(11.8 대 11.6 s), 다른 프로세스의 CPU 사용은 거의 없었다. 단일 스레드 10 초 부하에서 3 초 뒤 22 % 느려지는 것(터보 종료)만 보였고, 절반은 설명하지 못한다. 원인 미확인 — 기준 방식에 불리한 쪽(three.js 가 느려 보임)이라 공개 비교에 쓰면 안 된다. 다음 같은 세션 비교에서 해석 속도를 함께 기록해 다시 본다.
- 판정·기준값 정정은 감독이 정한다(소유자 위임, 결정 0040). ~~SPEC §7.1 frame_time 목표의 근거로는 위 frame_time 행(1.9 / 3.2 / 3.6 ms)을 제안한다.~~ → 재측정 2 로 바꾼다.

### 재측정 2(2026-10-08 14:23–14:28, C1–C4 준수) — 기준값 정정의 근거

| | 재측정 2 |
|---|---|
| 기준 방식 | 밉맵 끔(결정 0025 이후 기준 방식) |
| 유효성 | 15 회 모두 C1–C4 통과(59.88 Hz, CPU 보정 기준의 99–101 %), 실패·재실행 0, 전원 조절 끈 프로세스 실행마다 14–15 개 |
| **frame_time_total p50 / p95 / p99**(orbit · 5) | **1.7 / 1.9 / 2.0 ms**(실행별 p50 1.7, p99 2.0–2.1) |
| first_frame(cold · 5) | 989.5 (924.1–1,007.8) ms |
| event_latency refined p50(cold · 5) | 943.0 (879.5–953.7) ms |
| event_latency preview / refined p50(×60 · 5) | **478.4 / 625.5 ms**(해석 합계 6.17 s — P3.4 6.26 s 와 같음) |

- 13:52 재측정의 p95·p99(3.2 / 3.6 ms)는 전원 조절 상태의 영향이었다(동기화 프레임에 CPU 몫이 있음). C1–C4 를 지키면 꼬리가 P3.4·P4.4 낮 세션(1.70 / 2.00 / 2.20)보다도 좁다.
- **SPEC §7.1 frame_time 목표 정정의 근거로 이 행(1.7 / 1.9 / 2.0 ms)을 제안한다.** 정하는 것은 감독(소유자 위임).
- replay ×60 은 이번에는 정상(478 / 626 ms, P2.5·P3.4 와 같음) — 13:52 의 두 배는 전원 조절 때문이었다.

## 어디에 시간이 드는가

마지막 스냅샷(cold, 5회 중앙값):

| 단계 | 시간 | 비율(받음 → 반영 882 ms 기준) |
|---|---:|---:|
| 다운로드(루프백) | 124 ms | 14 % |
| 메인 스레드 해석(`PLYLoader.parse`) | 711 ms | 81 % |
| 교체 프레임 GPU 대기(정점 업로드 포함) | 12.5 ms | 1 % |
| 그리기 한 프레임 | 2.8 ms | — |

- **비용의 대부분은 해석이다.** 67.5 MB 를 해석하는 711 ms 동안 메인 스레드가 멈춘다(입력·화면 갱신 없음, `frame_interval_max` 744 ms).
- 그리기는 이 GPU 에서 250만 점에 2.8 ms 로 이미 빠르다.
- 교체 프레임 GPU 대기는 점 수에 비례해 커진다(replay ×60: 이벤트 1 의 1.9 ms → 이벤트 14 의 29.1 ms).
- 메모리는 원본 파일 버퍼(67.5 MB) + 해석 중간 배열 + float32 속성(90 MB)이 겹쳐 렌더러 private 이 600 MB 대다. replay 에서는 이전 스냅샷이 해제되기 전에 다음을 받으므로 861 MB 까지 오른다.
- **replay ×1(실시간 33 분, 1 회)은 ×60 보다 느리다.** p50 preview 873.9 ms(×60 의 1.9 배), refined 881.2 ms(1.4 배), 최대 1.66 s. 해석 합 9,080 ms(×60 6,111 ms).
  이벤트별(×1 / ×60 중앙값):

  | # | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
  |---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
  | ×1 (ms) | 74 | 167 | 319 | 608 | 668 | 929 | 970 | 1277 | 1303 | 1611 | 1659 | 839 | 874 | 881 |
  | ×60 (ms) | 75 | 142 | 161 | 295 | 308 | 452 | 467 | 609 | 621 | 751 | 755 | 885 | 875 | 920 |
  | 배 | 0.98 | 1.17 | 1.98 | 2.06 | 2.17 | 2.05 | 2.08 | 2.10 | 2.10 | 2.14 | 2.20 | 0.95 | 1.00 | 0.96 |

  #3~#11 은 약 2 배, #1·#12~#14 는 같고 #2 는 1.17 배다. 원인은 **확인 안 됨**. 이벤트 사이 긴 유휴(수 분) 뒤 CPU 클럭이 내려간 상태에서 해석이 시작된다는 가설이 있으나,
  감독이 잰 ×10(이벤트 간격 3~30 s)에서는 나타나지 않았고(preview / refined p50 503.8 / 606.7 ms), ×1 에서도 #12~#14 는 정상이라 가설과 다 맞지는 않는다.
  P1.5 전에 ×1 을 한 번 더 재며 전원 구성표·CPU 클럭을 함께 기록한다(PR #7 검토 중간).

## PointBlitz 가 이겨야 할 곳

| 기준 방식 비용 | PointBlitz 설계 | 근거 결정 |
|---|---|---|
| 해석 711 ms(메인 스레드) | GPU 레이아웃 청크 → 해석 0, 메인 스레드 밖 | 0008 |
| 매번 전체 다운로드 530.8 MB | preview 는 새 점만(합 약 6.5 MB), refined 만 전체 | 0009 |
| GPU 속성 36 B/점 | 16 B/점(법선 기본 제외) | 0008 |
| 교체 중 이전·새 데이터 공존(861 MB) | 세대 교체(GPU 약 80 MB, CPU 버퍼 최소) | 0009 |
| 그리기 2.8 ms | 이 GPU 에서는 차이가 작을 것 — 저사양 GPU 는 비용 모델로 | 0011 |

## 해석 주의

- **GPU 프로세스 private bytes**(`mem_gpu_process_private_max`, cold 308 MB · replay 511 MB)는 GPU 메모리(VRAM)가 아니다. 드라이버·ANGLE 의 스테이징 버퍼와 공유 메모리가 들어 있다.
  vsync 를 끈 첫 측정에서는 큐가 쌓여 2.47 GB 까지 올랐다(결정 0021). VRAM 비교는 `mem_gpu`(추정) 로 한다.
- `mem_js_heap_max` 는 typed array·ArrayBuffer 를 세지 않는 보조 지표다(결정 0020). 비교에는 `mem_cpu` 를 쓴다.
- first_frame 은 탐색 시작 기준이라 페이지·모듈 로드(three.js 약 1.2 MB)가 섞여 실행마다 흔들린다(cold 961~1,041 ms). 데이터 비용 비교는 event_latency 로 한다.
- 루프백이라 다운로드에 네트워크 지연·대역폭이 없다. 실제 망에서는 바이트 ÷ 대역폭을 더한다(100 Mbps 면 마지막 스냅샷만 5.4 s).

## 재현

```
cd baseline/three && npm ci && cd ../..
bash baseline/three/suite.sh <ply 폴더> target/bench/p06        # 약 45 분, 끝에 표를 출력한다
SKIP_X1=1 bash baseline/three/suite.sh <ply 폴더> target/bench/p06   # ×1(33 분) 생략
```
스크립트는 이름을 바꾼 서버 복사본을 띄우고 PID 로 종료한다.

## 전체 표 (중앙값 · 최소 · 최대)

`node aggregate.mjs` 출력 그대로.

장비: ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 (0x00002786) Direct3D11 vs_5_0 ps_5_0, D3D11) · Chrome 155.0.8059.40 · 13th Gen Intel(R) Core(TM) i7-13700F · Windows_NT 10.0.26200 · three.js 0.185.1 · commit 78ed45c

| 시나리오 | 지표 | 세부 | 실행 수 | 중앙값 | 최소 | 최대 | 단위 |
|---|---|---|---:|---:|---:|---:|---|
| cold | bytes_total |  | 5 | 67,554,652 | 67,554,652 | 67,554,652 | B |
| cold | event_latency | #14 refined | 5 | 882.20 | 858.10 | 907.50 | ms |
| cold | event_latency_refined_p50 |  | 5 | 882.20 | 858.10 | 907.50 | ms |
| cold | event_latency_refined_max |  | 5 | 882.20 | 858.10 | 907.50 | ms |
| cold | first_frame |  | 5 | 990.10 | 960.80 | 1041.40 | ms |
| cold | frame_interval_p50 |  | 5 | 17.60 | 17.60 | 17.75 | ms |
| cold | frame_interval_p95 |  | 5 | 19.91 | 18.30 | 29.88 | ms |
| cold | frame_interval_p99 |  | 5 | 496.02 | 488.58 | 498.60 | ms |
| cold | frame_interval_max |  | 5 | 744 | 741.70 | 762.50 | ms |
| cold | swap_frame_gpu_wait | #14 | 5 | 12.50 | 12.40 | 13.10 | ms |
| cold | main_thread_block_count |  | 5 | 3 | 2 | 3 | count |
| cold | main_thread_block_ms |  | 5 | 837 | 758 | 861 | ms |
| cold | mem_cpu |  | 5 | 597,151,744 | 596,336,640 | 599,568,384 | B |
| cold | mem_renderer_working_set_max |  | 5 | 660,934,656 | 660,361,216 | 663,601,152 | B |
| cold | mem_gpu_process_private_max |  | 5 | 307,855,360 | 306,618,368 | 309,751,808 | B |
| cold | mem_js_heap_max |  | 5 | 358,259,504 | 358,193,580 | 385,720,592 | B |
| cold | mem_gpu_estimate |  | 5 | 90,072,540 | 90,072,540 | 90,072,540 | B |
| cold | stage_fetch_total |  | 5 | 124 | 99.40 | 124.30 | ms |
| cold | stage_parse_total |  | 5 | 710.90 | 705.80 | 731.40 | ms |
| orbit | bytes_total |  | 5 | 67,554,652 | 67,554,652 | 67,554,652 | B |
| orbit | event_latency | #14 refined | 5 | 870.90 | 864.10 | 884.90 | ms |
| orbit | event_latency_refined_p50 |  | 5 | 870.90 | 864.10 | 884.90 | ms |
| orbit | event_latency_refined_max |  | 5 | 870.90 | 864.10 | 884.90 | ms |
| orbit | first_frame |  | 5 | 990.10 | 919.30 | 1006.70 | ms |
| orbit | frame_interval_p50 |  | 5 | 17.70 | 17.50 | 17.80 | ms |
| orbit | frame_interval_p95 |  | 5 | 18.54 | 18.16 | 39.38 | ms |
| orbit | frame_interval_p99 |  | 5 | 493.22 | 481.32 | 510.57 | ms |
| orbit | frame_interval_max |  | 5 | 757.20 | 728.20 | 760.30 | ms |
| orbit | frame_time_total_p50 |  | 5 | 2.80 | 2.80 | 2.80 | ms |
| orbit | frame_time_total_p95 |  | 5 | 3.80 | 3.30 | 4.50 | ms |
| orbit | frame_time_total_p99 |  | 5 | 6 | 5.94 | 6.10 | ms |
| orbit | frame_time_cpu_p50 |  | 5 | 0 | 0 | 0 | ms |
| orbit | frame_time_cpu_p95 |  | 5 | 0.10 | 0.10 | 0.10 | ms |
| orbit | frame_time_cpu_p99 |  | 5 | 0.10 | 0.10 | 0.10 | ms |
| orbit | frame_time_gpu_estimate_p50 |  | 5 | 2.80 | 2.80 | 2.80 | ms |
| orbit | frame_time_gpu_estimate_p95 |  | 5 | 3.70 | 3.30 | 4.50 | ms |
| orbit | frame_time_gpu_estimate_p99 |  | 5 | 5.90 | 5.84 | 6.04 | ms |
| orbit | frame_time_total_p50_view | overview_sw | 5 | 3.60 | 2.90 | 4.40 | ms |
| orbit | frame_time_total_p50_view | north | 5 | 2.80 | 2.80 | 2.90 | ms |
| orbit | frame_time_total_p50_view | east | 5 | 2.80 | 2.80 | 2.80 | ms |
| orbit | frame_time_total_p50_view | south | 5 | 2.80 | 2.80 | 2.80 | ms |
| orbit | frame_time_total_p50_view | west | 5 | 2.80 | 2.80 | 2.80 | ms |
| orbit | frame_time_total_p50_view | top_down | 5 | 2.80 | 2.70 | 2.80 | ms |
| orbit | frame_time_total_p50_view | low_south | 5 | 2.80 | 2.70 | 2.80 | ms |
| orbit | frame_time_total_p50_view | close_dense | 5 | 2.10 | 2.10 | 2.10 | ms |
| orbit | swap_frame_gpu_wait | #14 | 5 | 12.80 | 12.60 | 13.40 | ms |
| orbit | main_thread_block_count |  | 5 | 3 | 2 | 3 | count |
| orbit | main_thread_block_ms |  | 5 | 848 | 758 | 854 | ms |
| orbit | mem_cpu |  | 5 | 598,405,120 | 598,147,072 | 598,650,880 | B |
| orbit | mem_renderer_working_set_max |  | 5 | 662,839,296 | 662,409,216 | 662,990,848 | B |
| orbit | mem_gpu_process_private_max |  | 5 | 313,053,184 | 307,326,976 | 321,785,856 | B |
| orbit | mem_js_heap_max |  | 5 | 363,472,992 | 363,450,532 | 363,537,504 | B |
| orbit | mem_gpu_estimate |  | 5 | 90,072,540 | 90,072,540 | 90,072,540 | B |
| orbit | stage_fetch_total |  | 5 | 110.20 | 101.90 | 117.20 | ms |
| orbit | stage_parse_total |  | 5 | 708.70 | 707.70 | 715.60 | ms |
| replay×1 | bytes_total |  | 1 | 530,796,804 | 530,796,804 | 530,796,804 | B |
| replay×1 | event_latency | #1 preview | 1 | 73.60 | 73.60 | 73.60 | ms |
| replay×1 | event_latency | #2 refined | 1 | 166.60 | 166.60 | 166.60 | ms |
| replay×1 | event_latency | #3 preview | 1 | 318.80 | 318.80 | 318.80 | ms |
| replay×1 | event_latency | #4 refined | 1 | 607.50 | 607.50 | 607.50 | ms |
| replay×1 | event_latency | #5 preview | 1 | 668.20 | 668.20 | 668.20 | ms |
| replay×1 | event_latency | #6 refined | 1 | 928.70 | 928.70 | 928.70 | ms |
| replay×1 | event_latency | #7 preview | 1 | 970 | 970 | 970 | ms |
| replay×1 | event_latency | #8 refined | 1 | 1276.90 | 1276.90 | 1276.90 | ms |
| replay×1 | event_latency | #9 preview | 1 | 1303.20 | 1303.20 | 1303.20 | ms |
| replay×1 | event_latency | #10 refined | 1 | 1610.80 | 1610.80 | 1610.80 | ms |
| replay×1 | event_latency | #11 preview | 1 | 1658.90 | 1658.90 | 1658.90 | ms |
| replay×1 | event_latency | #12 refined | 1 | 838.50 | 838.50 | 838.50 | ms |
| replay×1 | event_latency | #13 preview | 1 | 873.90 | 873.90 | 873.90 | ms |
| replay×1 | event_latency | #14 refined | 1 | 881.20 | 881.20 | 881.20 | ms |
| replay×1 | event_latency_preview_p50 |  | 1 | 873.90 | 873.90 | 873.90 | ms |
| replay×1 | event_latency_preview_max |  | 1 | 1658.90 | 1658.90 | 1658.90 | ms |
| replay×1 | event_latency_refined_p50 |  | 1 | 881.20 | 881.20 | 881.20 | ms |
| replay×1 | event_latency_refined_max |  | 1 | 1610.80 | 1610.80 | 1610.80 | ms |
| replay×1 | first_frame |  | 1 | 207.90 | 207.90 | 207.90 | ms |
| replay×1 | frame_interval_p50 |  | 1 | 16.70 | 16.70 | 16.70 | ms |
| replay×1 | frame_interval_p95 |  | 1 | 16.90 | 16.90 | 16.90 | ms |
| replay×1 | frame_interval_p99 |  | 1 | 17.10 | 17.10 | 17.10 | ms |
| replay×1 | frame_interval_max |  | 1 | 1299.80 | 1299.80 | 1299.80 | ms |
| replay×1 | swap_frame_gpu_wait | #1 | 1 | 2.10 | 2.10 | 2.10 | ms |
| replay×1 | swap_frame_gpu_wait | #2 | 1 | 8.70 | 8.70 | 8.70 | ms |
| replay×1 | swap_frame_gpu_wait | #3 | 1 | 10.90 | 10.90 | 10.90 | ms |
| replay×1 | swap_frame_gpu_wait | #4 | 1 | 17 | 17 | 17 | ms |
| replay×1 | swap_frame_gpu_wait | #5 | 1 | 17.50 | 17.50 | 17.50 | ms |
| replay×1 | swap_frame_gpu_wait | #6 | 1 | 17.20 | 17.20 | 17.20 | ms |
| replay×1 | swap_frame_gpu_wait | #7 | 1 | 18 | 18 | 18 | ms |
| replay×1 | swap_frame_gpu_wait | #8 | 1 | 21.50 | 21.50 | 21.50 | ms |
| replay×1 | swap_frame_gpu_wait | #9 | 1 | 21.70 | 21.70 | 21.70 | ms |
| replay×1 | swap_frame_gpu_wait | #10 | 1 | 37.60 | 37.60 | 37.60 | ms |
| replay×1 | swap_frame_gpu_wait | #11 | 1 | 31.60 | 31.60 | 31.60 | ms |
| replay×1 | swap_frame_gpu_wait | #12 | 1 | 28.30 | 28.30 | 28.30 | ms |
| replay×1 | swap_frame_gpu_wait | #13 | 1 | 17.10 | 17.10 | 17.10 | ms |
| replay×1 | swap_frame_gpu_wait | #14 | 1 | 19.40 | 19.40 | 19.40 | ms |
| replay×1 | main_thread_block_count |  | 1 | 16 | 16 | 16 | count |
| replay×1 | main_thread_block_ms |  | 1 | 9,239 | 9,239 | 9,239 | ms |
| replay×1 | mem_cpu |  | 1 | 701,030,400 | 701,030,400 | 701,030,400 | B |
| replay×1 | mem_renderer_working_set_max |  | 1 | 768,356,352 | 768,356,352 | 768,356,352 | B |
| replay×1 | mem_gpu_process_private_max |  | 1 | 399,192,064 | 399,192,064 | 399,192,064 | B |
| replay×1 | mem_js_heap_max |  | 1 | 374,339,548 | 374,339,548 | 374,339,548 | B |
| replay×1 | mem_gpu_estimate |  | 1 | 90,072,540 | 90,072,540 | 90,072,540 | B |
| replay×1 | stage_fetch_total |  | 1 | 2585.90 | 2585.90 | 2585.90 | ms |
| replay×1 | stage_parse_total |  | 1 | 9079.90 | 9079.90 | 9079.90 | ms |
| replay×60 | bytes_total |  | 3 | 530,796,804 | 530,796,804 | 530,796,804 | B |
| replay×60 | event_latency | #1 preview | 3 | 75.20 | 73 | 80.50 | ms |
| replay×60 | event_latency | #2 refined | 3 | 142 | 137.40 | 149.20 | ms |
| replay×60 | event_latency | #3 preview | 3 | 160.70 | 158.70 | 193.10 | ms |
| replay×60 | event_latency | #4 refined | 3 | 295.20 | 291.90 | 295.70 | ms |
| replay×60 | event_latency | #5 preview | 3 | 308.10 | 306.30 | 309.70 | ms |
| replay×60 | event_latency | #6 refined | 3 | 452.10 | 436.30 | 467.60 | ms |
| replay×60 | event_latency | #7 preview | 3 | 466.50 | 464 | 481.20 | ms |
| replay×60 | event_latency | #8 refined | 3 | 609.10 | 598.90 | 610.20 | ms |
| replay×60 | event_latency | #9 preview | 3 | 621.30 | 618.90 | 647.60 | ms |
| replay×60 | event_latency | #10 refined | 3 | 751.30 | 716.60 | 756 | ms |
| replay×60 | event_latency | #11 preview | 3 | 754.90 | 751.10 | 776.60 | ms |
| replay×60 | event_latency | #12 refined | 3 | 885.40 | 883.70 | 897.80 | ms |
| replay×60 | event_latency | #13 preview | 3 | 875.40 | 872 | 912.80 | ms |
| replay×60 | event_latency | #14 refined | 3 | 920.40 | 910.20 | 960.10 | ms |
| replay×60 | event_latency_preview_p50 |  | 3 | 466.50 | 464 | 481.20 | ms |
| replay×60 | event_latency_preview_max |  | 3 | 875.40 | 872 | 912.80 | ms |
| replay×60 | event_latency_refined_p50 |  | 3 | 609.10 | 598.90 | 610.20 | ms |
| replay×60 | event_latency_refined_max |  | 3 | 920.40 | 910.20 | 960.10 | ms |
| replay×60 | first_frame |  | 3 | 205.40 | 204.60 | 228.90 | ms |
| replay×60 | frame_interval_p50 |  | 3 | 16.70 | 16.70 | 17.20 | ms |
| replay×60 | frame_interval_p95 |  | 3 | 16.90 | 16.80 | 18.20 | ms |
| replay×60 | frame_interval_p99 |  | 3 | 17.02 | 17 | 36.80 | ms |
| replay×60 | frame_interval_max |  | 3 | 816.70 | 800 | 850.50 | ms |
| replay×60 | swap_frame_gpu_wait | #1 | 3 | 1.90 | 1.90 | 2.10 | ms |
| replay×60 | swap_frame_gpu_wait | #2 | 3 | 5.40 | 5.40 | 5.60 | ms |
| replay×60 | swap_frame_gpu_wait | #3 | 3 | 8.80 | 8.60 | 10.30 | ms |
| replay×60 | swap_frame_gpu_wait | #4 | 3 | 12.50 | 11.90 | 12.50 | ms |
| replay×60 | swap_frame_gpu_wait | #5 | 3 | 13.10 | 11.50 | 14.80 | ms |
| replay×60 | swap_frame_gpu_wait | #6 | 3 | 15.90 | 15.30 | 18.80 | ms |
| replay×60 | swap_frame_gpu_wait | #7 | 3 | 15.40 | 9.90 | 17.30 | ms |
| replay×60 | swap_frame_gpu_wait | #8 | 3 | 20.90 | 18.80 | 21.40 | ms |
| replay×60 | swap_frame_gpu_wait | #9 | 3 | 21.40 | 13.70 | 24 | ms |
| replay×60 | swap_frame_gpu_wait | #10 | 3 | 16.30 | 15.10 | 23.40 | ms |
| replay×60 | swap_frame_gpu_wait | #11 | 3 | 16.70 | 15.80 | 24.50 | ms |
| replay×60 | swap_frame_gpu_wait | #12 | 3 | 28.20 | 28.10 | 31.60 | ms |
| replay×60 | swap_frame_gpu_wait | #13 | 3 | 18.10 | 15.60 | 18.50 | ms |
| replay×60 | swap_frame_gpu_wait | #14 | 3 | 29.10 | 21.60 | 29.30 | ms |
| replay×60 | main_thread_block_count |  | 3 | 16 | 15 | 17 | count |
| replay×60 | main_thread_block_ms |  | 3 | 6,305 | 6,185 | 6,321 | ms |
| replay×60 | mem_cpu |  | 3 | 860,733,440 | 812,425,216 | 911,822,848 | B |
| replay×60 | mem_renderer_working_set_max |  | 3 | 921,550,848 | 874,975,232 | 968,040,448 | B |
| replay×60 | mem_gpu_process_private_max |  | 3 | 511,238,144 | 474,685,440 | 511,926,272 | B |
| replay×60 | mem_js_heap_max |  | 3 | 511,453,304 | 484,134,544 | 549,809,004 | B |
| replay×60 | mem_gpu_estimate |  | 3 | 90,072,540 | 90,072,540 | 90,072,540 | B |
| replay×60 | stage_fetch_total |  | 3 | 817.20 | 807.70 | 845.80 | ms |
| replay×60 | stage_parse_total |  | 3 | 6110.60 | 6096.60 | 6161.30 | ms |
