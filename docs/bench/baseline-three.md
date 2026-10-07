# 기준 방식(three.js) 측정 — P0.6

- 날짜: 2026-10-07
- 대상: 기준 방식(결정 0005·0019) — three.js 0.185.1 `PLYLoader` + `Points`, 이벤트마다 전체 다운로드·메인 스레드 해석·전체 교체
- 데이터: `flight-01`(14 스냅샷, 마지막 2,502,015 점 · 67,554,652 B)
- 측정 방법: 결정 0020(동기화 프레임, OS 프로세스 메모리, longtask), 결정 0021(vsync 켬)
- 장비: RTX 4070 12 GB(드라이버 591.86) · ANGLE D3D11 · Chrome 155.0.8059.40 · Intel i7-13700F · Windows 11(10.0.26200) · 헤드리스 60 Hz
- 측정 커밋: `78ed45c` · 서버는 루프백(네트워크 시간 없음 — 결정 0011 로 따로 더한다)

## 요약 — 비교표 three.js 열 (중앙값)

| 지표 | 값 | 시나리오 · 실행 수 | 비고 |
|---|---:|---|---|
| bytes_total | 530,796,804 B | replay ×60 · 3 | 스냅샷 14개 전체를 매번 받는다 |
| event_latency (preview, p50) | 466.5 ms | replay ×60 · 3 | 최대 875 ms |
| event_latency (refined, p50) | 609.1 ms | replay ×60 · 3 | 최대 920 ms |
| first_frame (cold) | 990.1 ms | cold · 5 | 마지막 스냅샷 하나 |
| frame_time_total p50 / p95 / p99 (2.5 M) | 2.8 / 3.8 / 6.0 ms | orbit · 5 | 동기화 프레임, 시점 8곳 |
| frame_time_cpu p50 | < 0.1 ms | orbit · 5 | `performance.now()` 해상도 0.1 ms |
| main_thread_block | 16 회 · 6,305 ms | replay ×60 · 3 | 50 ms 넘는 작업 합 |
| main_thread_block (cold) | 3 회 · 837 ms | cold · 5 | |
| frame_interval_max | 816.7 ms | replay ×60 · 3 | 가장 긴 멈춤(해석 중) |
| mem_cpu (렌더러 private) | 860.7 MB | replay ×60 · 3 | cold 597.2 MB |
| mem_gpu (추정) | 90.1 MB | 모두 | 2.5 M 점 × 36 B(위치·법선·색 float32) |

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
- replay ×1(실시간 33 분, 1 회): 이벤트 반영 p50 은 ×60 과 비슷하지만(preview 874 ms, refined 881 ms) 최대가 1.6 s 로 길다.
  해석 합이 9,080 ms 로 ×60(6,111 ms)보다 크다 — 이벤트 사이 긴 유휴 뒤 CPU 클럭이 내려간 상태에서 해석이 시작되는 것으로 보인다(확인 안 됨, 1 회 측정).

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
- 루프백이라 다운로드에 네트워크 지연·대역폭이 없다. 실제 망에서는 바이트 ÷ 대역폭을 더한다(100 Mbps 면 마지막 스냅샷만 5.4 s).

## 재현

```
cargo build --release -p pointblitz-bench
copy target\release\pointblitz-bench.exe target\release\pb-replay.exe   # 다른 세션과 이름이 겹치지 않게
target/release/pb-replay.exe replay --data <ply 폴더> --web . --port 8782
cd baseline/three && npm ci
for i in 1..5: node run.mjs --server http://127.0.0.1:8782 --scenario cold  --metrics ../../target/bench/p06/cold-$i.jsonl
for i in 1..5: node run.mjs --server http://127.0.0.1:8782 --scenario orbit --metrics ../../target/bench/p06/orbit-$i.jsonl
for i in 1..3: node run.mjs --server http://127.0.0.1:8782 --scenario replay --speed 60 --metrics ../../target/bench/p06/replay60-$i.jsonl
node run.mjs --server http://127.0.0.1:8782 --scenario replay --speed 1 --metrics ../../target/bench/p06/replay1-1.jsonl
node aggregate.mjs ../../target/bench/p06 --md <표>
```
서버는 PID 로 종료한다.

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
