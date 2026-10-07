# 0020 측정 방법: 무엇을 어떻게 재는가

- 상태: 승인
- 날짜: 2026-10-07
- 결정한 사람: 작업자 제안 → 감독 승인(PR #6 재검토, 2026-10-07)

## 맥락
P0.5. SPEC §6.2 지표를 브라우저(기준 방식)에서 뽑는다. 같은 방법을 PointBlitz(native·browser·server)에도 써야 비교가 공정하다.
첫 시도에서 두 가지 함정이 드러났다.

1. `requestAnimationFrame` 간격은 화면 주사율에 묶인다(60 Hz 화면에서 p50 16.7 ms). 그리기 비용이 아니라 vsync 를 잰다.
2. 주사율 제한을 풀면(`--disable-gpu-vsync --disable-frame-rate-limit`) 간격이 0.1 ms 로 떨어진다. rAF 는 GPU 작업 완료를 기다리지 않으므로
   CPU 가 명령을 넘기는 시간만 잰다.

## 선택지
| 항목 | 선택지 | 고른 것 | 이유 |
|---|---|---|---|
| 그리기 비용 | rAF 간격 / GPU 타이머 쿼리(`EXT_disjoint_timer_query_webgl2`) / **동기화 프레임** | 동기화 프레임 | 프레임마다 그린 뒤 1 픽셀을 읽어(`readPixels`) GPU 완료를 기다리고 그 시간을 잰다. 타이머 쿼리는 브라우저에서 보안상 꺼져 있는 경우가 많고 구현마다 다르다. 동기화 프레임은 WebGL·WebGPU·native 어디서나 같은 방식(native 는 `device.poll(Wait)`)으로 잴 수 있다 |
| 반응성 | — / **rAF 간격 유지(`frame_interval_*`)** | 유지 | 메인 스레드가 멈추면 rAF 가 밀린다. 사용자가 느끼는 끊김을 보여 준다. 그리기 비용과 이름을 나눠 기록한다 |
| vsync | 켬 / **끔(기본)** | 끔 | 켜면 모든 구현이 16.7 ms 로 같아 보인다. 사용자 체감 비교가 필요하면 `--vsync` 로 따로 잰다 |
| 메인 스레드 블록 | DevTools 트레이스 / **`PerformanceObserver('longtask')`** | longtask | 표준 API, 50 ms 넘는 작업을 시작·길이로 준다. 트레이스는 파일이 크고 해석이 무겁다 |
| 메모리(`mem_cpu`) | `performance.memory` / CDP `JSHeapUsedSize` / `measureUserAgentSpecificMemory`(COOP/COEP 필요) / **OS 의 렌더러 프로세스 private bytes** | OS private bytes(250 ms 표본, 최대) | JS 힙 지표는 ArrayBuffer·typed array·wasm 메모리를 세지 않는다(PR #6 검토 H1). three.js 의 67 MB 파일 버퍼·90 MB 속성과 PointBlitz wasm 의 거의 전부가 빠져 PointBlitz 에 구조적으로 유리해진다. 프로세스 private bytes 는 모두 포함하고, COOP/COEP 처럼 페이지 조건을 바꾸지 않는다. 렌더러·GPU 프로세스 id 는 CDP `SystemInfo.getProcessInfo`, 표본은 PowerShell 상주 루프 하나(`procmem.mjs`). JS 힙은 `mem_js_heap_max` 로 보조 기록 |
| GPU 메모리 | 측정 / **추정(점 수 × 속성 바이트)** | 추정, `method` 필드에 표시 | 페이지에서 GPU 메모리를 잴 방법이 없다. `PLYLoader` 는 파일의 법선까지 float32 속성으로 만들어 위치·법선·색 점당 **36 B** 를 올린다(PR #5 검토). 속성 이름을 `parse_end` 표식에 기록해 계산한다. PointBlitz 는 법선을 기본으로 보내지 않으므로(결정 0008) 이 차이는 비교표에 원인으로 적는다. PointBlitz native 는 실제 버퍼 크기를 기록한다 |
| 이벤트 반영 시간 | 다운로드 끝까지 / render() 반환까지 / **알림 받음 → 그 점을 담은 첫 프레임의 GPU 완료** | 마지막 | 사용자가 체감하는 지연. 새 스냅샷을 담은 첫 프레임에서만 한 번 동기화(1 px 읽기)해 `submitted`(render 반환)와 `presented`(GPU 완료)를 나눈다(PR #5 검토 — render() 직후 표식은 GPU 완료가 아니다). 비행당 14 회뿐인 측정 인공물이고 모든 구현에 같게 적용한다. 차이는 `swap_frame_gpu_wait` 로 남는다 — three.js 는 이 프레임에서 정점 버퍼를 GPU 로 올리므로 업로드 비용이 여기 보인다 |
| 첫 화면 | 서버 시작부터 / **페이지 탐색 시작(performance.now 0)부터 첫 점 프레임** | 후자 | 브라우저 표준 기준점 |
| 커밋·장비 | `git rev-parse` / **`git describe --always --dirty`**, 드라이버 미기록 / **`nvidia-smi` 드라이버 버전 기록** | 후자 둘 | 더러운 작업 트리에서 잰 수치를 구분하고, 드라이버는 비용 모델 계수에 영향을 준다(PR #6 검토) |
| 지표 기록 | 사람용 표 / **JSON Lines `{metric, value, unit, target, device, scenario, commit, samples}`** | JSONL | 결정 0011 비용 모델과 비교표 생성이 같은 레코드를 읽는다. `device` 에 렌더러 문자열·브라우저·CPU·OS 를 넣는다 |

## 무엇을 재는가 (PR #5 검토 질문에 대한 답)

| 지표 | SPEC §6.2 대응 | 재는 것 | 사용자가 겪는 것인가 |
|---|---|---|---|
| `frame_time_total_p*` | `frame_time` | 동기화 프레임: CPU 명령 기록·제출 + GPU 실행 + 동기화 왕복 | 아니오 — 그리기 비용의 측정용 값(파이프라이닝 없음) |
| `frame_time_cpu_p*` | `frame_time`(CPU) | render() 호출 시간(명령 기록·제출) | 메인 스레드 몫 |
| `frame_time_gpu_estimate_p*` | `frame_time`(GPU) | `total − cpu`. GPU 실행 + 동기화 왕복 | GPU 몫의 상한 추정 |
| `frame_interval_p*`, `frame_interval_max` | (보조) | rAF 간격(vsync 끔). 메인 스레드가 프레임을 얼마나 자주 돌리는가 | 예 — 끊김·멈춤. max 는 가장 긴 멈춤 |
| `mem_cpu` | `mem_cpu` | 렌더러 프로세스 private bytes 최대 | 예 |
| `mem_js_heap_max` | (보조) | JS 힙만 | — |
| `mem_gpu_estimate` | `mem_gpu` | 점 수 × 올린 속성 바이트(추정) | — |
| `event_latency` | `event_latency` | 알림 → 그 점을 담은 첫 프레임 GPU 완료 | 예 |
| `main_thread_block_*` | `main_thread_block` | 50 ms 넘는 메인 스레드 작업. `orbit` 동기화 루프 구간(`sync_start`~`sync_end`) 안의 작업은 측정 인공물이라 뺀다 | 예 — 입력이 먹히지 않는 시간 |

**CPU·GPU 나누기(SPEC §6.2):** GPU 타이머 쿼리를 브라우저에서 쓸 수 없어(보안상 비활성) GPU 시간을 직접 재지 못한다.
대신 같은 프레임에서 `render()` 반환 시각과 동기화 완료 시각을 재서 CPU 몫과 나머지(GPU + 동기화 왕복, 상한)로 나눈다.
`performance.now()` 해상도가 0.1 ms(교차 출처 격리 꺼짐)라 CPU 몫이 0.1 ms 미만이면 0 으로 보인다.

**PointBlitz 에 같은 방법을 적용하는 법:**
- native(wgpu): CPU 몫 = 명령 인코딩 + `queue.submit` 반환까지, 총 = `device.poll(Wait)` 반환까지. wgpu 타임스탬프 쿼리를 쓸 수 있으면 GPU 시간을 직접 함께 기록한다.
- browser(WebGPU): CPU 몫 = 인코딩 + `submit`, 총 = `queue.onSubmittedWorkDone()` 해결까지. WebGL2 대체 경로는 three.js 와 같은 1 px `readPixels`.
- server: native 와 같고, 인코딩(NVENC) 시간을 따로 더한다.

## 메모리 지표 자가 점검 (`memtest` 시나리오)

페이지가 200 MB `Float32Array`(209,715,200 B)를 잡기 직전과 직후의 렌더러 private bytes 차이:

| 실행 | `memtest_cpu_delta` | `memtest_js_heap_max` |
|---|---:|---:|
| 1 | 222,859,264 B | 5,555,492 B |
| 2 | 223,498,240 B | 5,517,388 B |
| 3 | 220,745,728 B | 5,592,188 B |

`mem_cpu` 는 할당한 만큼(+5~7 %) 오르고 JS 힙은 그대로다 — typed array 메모리가 `mem_cpu` 에 잡힌다.
재현: `node run.mjs --server … --scenario memtest`.

## 결과 예 (이 PC, RTX 4070, Chrome 155, 한 번 실행 — 정식 반복 측정은 P0.6)

- `cold`: bytes_total 67,554,652 B, event_latency 902 ms(다운로드 127 ms + 해석 736 ms), 메인 스레드 블록 2 회 826 ms, JS 힙 최대 386 MB.
- `orbit` 동기화 프레임: 250만 점 frame_time_total p50 2.8 ms(시점별 2.1~2.9 ms), frame_time_cpu p50 < 0.1 ms.
- `cold` 메모리: mem_cpu 611 MB(렌더러 private), mem_js_heap_max 370 MB, GPU 프로세스 private 640 MB. frame_interval_max 788 ms(해석 중 멈춤).
- `cold` 교체 프레임: `swap_frame_gpu_wait` 12.3 ms(정점 버퍼 90 MB 업로드 포함), mem_gpu_estimate 90,072,540 B(250만 점 × 36 B).

해석: 이 GPU 에서 기준 방식의 **그리기는 이미 빠르고, 비용은 받기·해석·업로드(약 0.9 s, 그동안 메인 스레드 정지)에 몰려 있다.**
PointBlitz 가 이길 곳은 주로 갱신 경로(결정 0008·0009)이고, 그리기 차이는 저사양 GPU(비용 모델) 쪽에서 커질 수 있다.

## 대가
- `event_latency` 는 교체 프레임의 동기화 1 회만큼 보수적(느린) 값이다(그 프레임의 파이프라이닝을 막는다). 모든 구현에 같게 적용한다.
- 동기화 프레임은 파이프라이닝을 막으므로 실제 앱의 처리량보다 보수적인(느린) 값이다. 모든 구현에 같은 방식이라 비교는 공정하다.
- 루프백 서버라 다운로드 시간에 네트워크가 없다. 네트워크는 바이트 × 대역폭으로 더한다(결정 0011).

## 다시 볼 조건
- 브라우저 GPU 타이머가 쓸 수 있게 되면 동기화 프레임과 함께 기록해 차이를 본다.
