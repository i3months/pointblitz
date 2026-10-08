# SPEC — PointBlitz 사양

이 문서는 PointBlitz 가 무엇을 만족해야 하는지의 단일 출처다. 이유는 [INTENT.md](INTENT.md), 운영은 [PROJECT.md](PROJECT.md),
기술 선택의 근거는 [docs/decisions/](docs/decisions/) 에 있다. 이 문서의 수치·범위는 프로젝트 소유자만 바꾼다(§9).

---

## 1. 범위

| 구분 | 내용 |
|---|---|
| 입력 | skyrecon 점군 PLY(§2) |
| 실행 위치 | 브라우저(wasm), 네이티브 창, 서버(헤드리스 렌더 + 영상 전송) — 같은 렌더 코어 |
| 비교 기준 | 같은 PLY 를 three.js 로 그리는 기존 방식(§5) |
| 결과물 | Rust 크레이트, wasm 패키지, 서버 실행 파일, 비교표와 하드웨어 비용 모델, 재현 스크립트 |
| 범위 밖 | 3D 가우시안 스플랫, 메시 구현(설계만), 점군 압축 코덱 |

## 2. 입력 데이터

### 2.1 형식

skyrecon `crates/core/src/io/ply.rs` 의 `XyzRgbNormal` 레이아웃.

```
ply
format binary_little_endian 1.0
element vertex N
property float32 x
property float32 y
property float32 z
property uint8 red
property uint8 green
property uint8 blue
property float32 nx
property float32 ny
property float32 nz
end_header
```

- 점 하나 27 B. 좌표는 미터 단위 ENU(skyrecon 정렬 원점 기준).
- 파서는 속성을 **이름으로** 찾는다(순서가 다른 `XyzNormalRgb` 레이아웃도 읽는다). 법선이 없는 15 B 레이아웃도 읽는다.

### 2.2 기준 데이터셋 `flight-01`

한 비행의 스냅샷 14개. 파일은 저장소에 넣지 않는다(§8.3).

| # | 파일 | 시각(s) | 종류 | 점 수 | 바이트 |
|---|---|---:|---|---:|---:|
| 1 | event_01_0097.0s_preview_0.ply | 97.0 | preview | 151,510 | 4,091,016 |
| 2 | event_02_0127.1s_refined_0.ply | 127.1 | refined | 336,118 | 9,075,432 |
| 3 | event_03_0298.9s_preview_1.ply | 298.9 | preview | 371,585 | 10,033,041 |
| 4 | event_04_0327.4s_refined_1.ply | 327.4 | refined | 749,216 | 20,229,078 |
| 5 | event_05_0570.7s_preview_2.ply | 570.7 | preview | 798,744 | 21,566,334 |
| 6 | event_06_0623.5s_refined_2.ply | 623.5 | refined | 1,192,254 | 32,191,105 |
| 7 | event_07_0934.8s_preview_3.ply | 934.8 | preview | 1,236,878 | 33,395,953 |
| 8 | event_08_0994.2s_refined_3.ply | 994.2 | refined | 1,640,611 | 44,296,744 |
| 9 | event_09_1333.1s_preview_4.ply | 1333.1 | preview | 1,676,080 | 45,254,407 |
| 10 | event_10_1388.6s_refined_4.ply | 1388.6 | refined | 2,059,581 | 55,608,934 |
| 11 | event_11_1760.7s_preview_5.ply | 1760.7 | preview | 2,113,986 | 57,077,869 |
| 12 | event_12_1805.1s_refined_5.ply | 1805.1 | refined | 2,404,412 | 64,919,371 |
| 13 | event_13_2005.6s_preview_6.ply | 2005.6 | preview | 2,426,023 | 65,502,868 |
| 14 | event_14_2053.0s_refined_6.ply | 2053.0 | refined | 2,502,015 | 67,554,652 |
| | 합계 | | | | 530,796,804 |

### 2.3 스냅샷 사이의 관계 (실측, [docs/data/flight-01.md](docs/data/flight-01.md))

- **preview = 직전 스냅샷 + 새 점.** 직전 스냅샷의 점이 비트 단위로 100 % 그대로 있고, 새 점 2.2만~5.4만 개가 붙는다.
- **refined = 전체 재계산.** 직전 스냅샷과 비트 단위로 같은 점이 0 개다. 초반에는 좌표계 원점까지 옮겨져
  같은 지점의 점이 0.5 m 넘게 어긋나고(정렬 원점이 재정렬마다 바뀜), 후반에는 최근접 거리 중앙값 3 cm 로 수렴한다.
- 따라서 전송·갱신 모델은 **preview 는 추가(append), refined 는 전체 교체(replace)** 다([0009](docs/decisions/0009-chunk-model-append-replace.md)).

## 3. 아키텍처

```
                 ┌────────────────────────────── pointblitz-core ──────────────────────────────┐
PLY / 청크 ────► │ chunk store(append·replace·세대) → GPU 버퍼 → 컬링 → LOD → 점 그리기 → 캡처 │
                 └───────────────┬───────────────────────┬───────────────────────┬─────────────┘
                                 │                       │                       │
                       pointblitz-native        pointblitz-web (wasm)     pointblitz-server
                       winit 창                 canvas · WebGPU/WebGL2    헤드리스 → NVENC → WebSocket
```

### 3.1 크레이트

| 크레이트 | 책임 | 의존해서는 안 되는 것 |
|---|---|---|
| `pointblitz-io` | PLY 스트림 파서(바이트 슬라이스·증분 입력), 청크 직렬화 형식 | wgpu, 창, 파일 시스템 경로(입력은 바이트) |
| `pointblitz-core` | 장면·청크 저장소·GPU 버퍼·컬링·LOD·렌더 패스·카메라·고정 시점 캡처 | 창, 네트워크, 브라우저 API |
| `pointblitz-native` | winit 창, 입력, 프레임 타이밍, 로컬 재생 | — |
| `pointblitz-web` | wasm-bindgen API, JS 래퍼(npm 패키지) | — |
| `pointblitz-server` | 헤드리스 렌더, 하드웨어 영상 인코딩, 영상·입력 전송 | — |
| `pointblitz-bench` | 시나리오 재생기, 지표 수집, 비교표·비용 모델 생성 | — |
| `baseline/three` | 기존 방식 재현(JS). 비교 전용, 배포물에 포함하지 않는다 | — |

### 3.2 렌더링 규칙

| 규칙 | 값 | 근거 |
|---|---|---|
| 그리기 | 불투명 점 + 깊이 테스트, 정렬 없음 | [0006](docs/decisions/0006-opaque-points-no-sort.md) |
| 점 모양·크기 | 화면 기준 고정 픽셀 지름의 원(기본 2 px), 기준 방식과 같게 맞춘다 | [0007](docs/decisions/0007-point-size-shape.md) |
| 색 | PLY 의 sRGB 8 bit 를 그대로. 조명 없음(기본) | [0007](docs/decisions/0007-point-size-shape.md) |
| 카메라 | 원근 투영. 좌표 규약은 §3.4 | [0013](docs/decisions/0013-coordinate-conventions.md) |
| 해상도 | 측정 기본 1920×1080, 장치 픽셀 비 1 | §6 |

### 3.3 서버 ↔ 클라이언트

| 실행 위치 | 서버가 하는 일 | 클라이언트가 받는 것 |
|---|---|---|
| 기존 방식 | PLY 정적 서빙 | 이벤트마다 PLY 전체 |
| 브라우저 · 네이티브 | PLY → GPU 레이아웃 청크 변환. preview 는 새 점만, refined 는 새 세대 전체 | GPU 버퍼에 그대로 복사할 수 있는 무압축 청크([0008](docs/decisions/0008-gpu-ready-uncompressed-chunks.md)) |
| 서버 영상 | 서버 GPU 렌더 → 하드웨어 인코딩 | H.264 영상 프레임(WebSocket + WebCodecs), 입력은 역방향 전송([0010](docs/decisions/0010-server-video-transport.md)) |

### 3.4 좌표 규약

- 입력은 ENU(x 동, y 북, z 위), 1 unit = 1 m.
- 카메라 공간은 오른손, x 오른쪽 · y 위 · −z 앞(wgpu/GL 관례). OpenCV 규약(y 아래 · z 앞)과의 변환은 `diag(1, −1, −1)`.
- 변환은 `pointblitz-core` 의 한 모듈에서만 한다.

## 4. 공개 API (초안)

```rust
// pointblitz-core
let mut scene = Scene::new(&device, &queue, SceneConfig::default());
let gen = scene.begin_generation();          // refined: 새 세대 시작
scene.append(gen, &chunk);                   // preview·refined 청크 추가
scene.commit(gen);                           // 다 받은 세대를 화면에 올리고 이전 세대 해제
scene.render(&mut encoder, &target, &camera);
```

- 세대(generation)를 바꾸는 동안 이전 세대를 계속 그린다(빈 화면 없음).
- wasm·Python 바인딩은 같은 개념(`begin_generation`, `append`, `commit`, `render`)을 그대로 노출한다.

## 5. 기준 방식 (기존 방식 재현)

skylens 는 skyrecon 점군을 아직 그리지 않으므로, skylens 의 three.js 사용 방식을 같은 데이터에 적용한 것을 기준으로 삼는다
([0005](docs/decisions/0005-baseline-definition.md)).

- three.js 0.185(skylens 와 같은 버전), `PLYLoader` + `THREE.Points` + `PointsMaterial`.
- 이벤트마다 PLY 전체를 HTTP 로 받고, 메인 스레드에서 해석하고, 기존 `Points` 를 버리고 새로 만든다.
- 점 크기·모양·색·해상도는 §3.2 와 같게 맞춘다.

## 6. 측정

### 6.1 시나리오

| 이름 | 내용 |
|---|---|
| `replay` | `flight-01` 14 이벤트를 원래 간격(또는 배속 ×N)으로 도착시킨다 |
| `orbit` | 마지막 스냅샷을 다 받은 뒤 고정 시점 8곳을 순서대로 돌며 각 시점에서 N 프레임 |
| `cold` | 빈 상태에서 마지막 스냅샷 하나만 받는다(첫 화면 시간 측정) |

고정 시점 8곳은 `bench/viewpoints/flight-01.json` 에 데이터로 고정한다(위치·목표·위 방향·시야각·해상도).

### 6.2 지표

| 지표 | 정의 | 단위 |
|---|---|---|
| `bytes_total` / `bytes_event` | 클라이언트가 받은 바이트(전체 / 이벤트별) | B |
| `event_latency` | 이벤트 도착 → 그 이벤트의 점이 모두 보이는 첫 프레임 | ms |
| `first_frame` | 접속 → 첫 점이 그려진 프레임(`cold`) | ms |
| `frame_time` | p50 / p95 / p99, CPU·GPU 따로 | ms |
| `main_thread_block` | 50 ms 넘는 메인 스레드 작업의 수와 합(브라우저) | 개, ms |
| `mem_cpu` / `mem_gpu` | 프로세스(또는 JS 힙) 최대 / GPU 버퍼 합 | B |
| `ssim` | 같은 시점에서 기준 영상 대비 SSIM(Wang 2004, 11×11 가우시안 σ 1.5) | — |
| 서버 영상 전용 | 인코딩 시간, 비트레이트, 입력 → 화면 지연 | ms, bps |

지표 레코드: `{metric, value, unit, target, device, scenario, commit, samples}` — JSON Lines.

### 6.3 하드웨어 비용 모델

단계마다 하드웨어 독립 양과 그것을 좌우하는 하드웨어 값을 짝지어 기록하고, 측정 장비에서 계수를 맞춘다
([0011](docs/decisions/0011-hardware-cost-model.md)).

| 단계 | 하드웨어 독립 양 | 하드웨어 값 |
|---|---|---|
| 전송 | 바이트 | 네트워크 대역폭 |
| 해석 | 바이트, 점 수 | CPU 단일 스레드 처리량 |
| 업로드 | 바이트 | PCIe · 메모리 대역폭 |
| 컬링 · LOD | 청크 수 | GPU 연산 |
| 래스터 | 그려진 점 수 × 점 면적(px) | GPU 정점 처리량, 필레이트 |
| 인코딩 | 해상도 × fps | 인코더 처리량 |

측정 장비: NVIDIA V100 16 GB(헤드리스), RTX 4070 12 GB(데스크톱). 다른 GPU 는 공개 사양으로 추정하고 추정값임을 표시한다.

### 6.4 비교표 틀

| 지표 | three.js | native | browser (wasm) | server video |
|---|---|---|---|---|
| bytes_total | | | | |
| event_latency (preview, p50) | | | | |
| event_latency (refined, p50) | | | | |
| first_frame (cold) | | | | |
| frame_time p95 (2.5 M pts) | | | | |
| main_thread_block | | — | | — |
| mem_cpu / mem_gpu | | | | |
| ssim vs reference | 기준 | | | |

## 7. 단계와 완료 기준

| 단계 | 내용 | 완료 기준 |
|---|---|---|
| P0 | 워크스페이스·CI, 시점 고정, 시나리오 재생기, 기준 방식(three.js) 측정 | 비교표 three.js 열이 실측으로 채워진다 |
| P1 | `pointblitz-io` + `pointblitz-core` + `pointblitz-native` | 14 이벤트 재생, 비교표 native 열, 기준 대비 SSIM 기록 |
| P2 | `pointblitz-web` (WebGPU, WebGL2 대체) | 비교표 browser 열 |
| P3 | `pointblitz-server` (헤드리스 + NVENC + WebSocket/WebCodecs) | 비교표 server video 열 |
| P4 | 비용 모델, 공개용 비교표·README·데모 | 공개 |
| P5 | 메시(지형·건물·텍스처 표면), Python 바인딩 | 별도 SPEC 갱신 |

목표 수치는 P0 에서 기준 방식을 잰 뒤 정했다(§7.1).

### 7.1 목표 수치 (P0.7)

PointBlitz 를 재기 전에 정했다. 근거·이유: [docs/ops/reviews/p0-7-targets.md](docs/ops/reviews/p0-7-targets.md). 기준 방식 값: [docs/bench/baseline-three.md](docs/bench/baseline-three.md).

측정 조건(모든 목표 공통): `flight-01`, RTX 4070 12 GB · i7-13700F · Windows 11, 루프백 서버, 1920×1080, vsync 켬(결정 0021), 측정 방법 결정 0020.
`replay` 는 ×60 으로 판정하고 ×1 은 같은 목표로 따로 보고한다.

| 지표 | 목표 | 적용 | 기준 방식(×60 중앙값) |
|---|---|---|---|
| `event_latency` preview p50 | ≤ 50 ms | native · browser | 466.5 ms |
| `event_latency` refined p50 | ≤ 200 ms | native · browser | 609.1 ms |
| cold 마지막 스냅샷 `event_latency` | ≤ 300 ms | native · browser | 882.2 ms |
| `main_thread_block` | 0 회(`replay` ×60 · `cold`) | browser | 16 회 · 6,305 ms |
| `mem_cpu` 최대(`replay` ×60) | **기록만**(2026-10-08 소유자: 메모리 제약 없음) — 이전 감독 값 ≤ 300 MB | browser 렌더러 / native 프로세스, OS 최대 commit(결정 0032) | 860.7 MB(이전 방법) |
| `frame_time_total` p50 / p95 / p99(`orbit`, 2.5 M 점) | 각각 기준 방식 이하 | native · browser | **1.7 / 1.9 / 2.0 ms**(2026-10-08 재측정 2: C1–C4 준수, 밉맵 끔 — 결정 0025·0040·0041). 이전 2.8 / 3.8 / 6.0 ms(P0.6: C3 범위 밖 11/14, C4 미상) |

- `first_frame` 은 보고만 한다.
- 판정은 측정 유효성 규칙 C1–C4(결정 0038·0040·0041)를 모두 지킨 같은 세션 측정으로 한다. 그 전 단계의 판정은 "C4 미상" 으로 문서에 남긴다(docs/bench/validity-audit.md).
- 위 표의 기준 방식 지연·`main_thread_block` 값은 P0.6(C4 미상)이다. 목표가 절대값이라 판정에 쓰이지 않으며, C1–C4 재측정 2 값은 preview / refined 478.4 / 625.5 ms, cold 943.0 ms 다(docs/bench/baseline-three.md).
- `frame_time` 비교는 두 구현의 점 모양이 같을 때만 유효하다(결정 0025).
- 저사양 GPU 는 비용 모델(결정 0011)로 추정해 보고한다.

#### 서버 영상(P3) 목표 — 2026-10-08 소유자 결정(P3 측정 전)

조건은 위와 같고, 서버와 브라우저가 같은 PC·같은 GPU 를 쓴다(결과에 적는다).

| 지표 | 목표 | 비고 |
|---|---|---|
| `event_latency` preview p50 | ≤ 80 ms | 다른 대상의 목표 + 인코딩·전송·디코딩·다음 영상 프레임 |
| `event_latency` refined p50 | ≤ 250 ms | |
| cold(첫 영상 프레임에 마지막 스냅샷이 보일 때까지) | ≤ 350 ms | |
| 입력 → 표시 지연 p50 / p95 | ≤ 50 / 80 ms | 입력 번호가 돌아온 영상 프레임이 화면에 그려질 때까지 |
| 영상 프레임 빠짐(orbit) | ≤ 1 % | **정의**: 클라이언트 화면의 60 Hz 주기 중 새 영상 프레임이 나오지 못한 주기의 비율(서버가 못 만든 것·전송·디코딩이 늦은 것을 모두 포함하는 끝에서 끝까지 값) |
| 화질: 고정 시점 SSIM(영상 프레임 대 **native 캡처**) | ≥ 0.95 | 인코딩 전 서버 프레임 대비 SSIM 은 진단값으로 함께 싣는다 |
| 클라이언트 `main_thread_block` | 0 회 | |
| 비트레이트 · 받은 바이트 · 서버 GPU 시간 · 메모리 | 기록만 | INTENT 원칙 2 |

제안 근거: [docs/notes/p3-targets-proposal.md](docs/notes/p3-targets-proposal.md).
- 대역폭(`bytes_total`)은 목표가 아니라 기록이다(INTENT 원칙 2).
- 메모리(`mem_cpu`, `mem_gpu`)도 목표가 아니라 기록이다(2026-10-08 소유자 결정: "메모리는 무조건 많이 써도 된다"). 메모리를 줄이려고 속도·지연을 내주는 선택은 하지 않는다.
- 소유자가 다른 값을 정하면 그 값이 우선한다(§9 에 기록).


## 8. 공통 규칙

### 8.1 품질

- 모든 기능은 시험이 있어야 끝난 것이다. 그리기 결과는 고정 시점 캡처로 확인한다.
- 성능 수치는 재현 명령과 함께 기록한다. 측정값에 맞춰 기준을 고치지 않는다.

### 8.2 라이선스

- MIT OR Apache-2.0. 의존성은 이와 호환되는 것만 쓴다(GPL·AGPL 금지).
- 차용 코드는 파일 머리에 출처와 라이선스를 쓴다.

### 8.3 데이터

- PLY 원본은 저장소에 넣지 않는다. `bench/data/README.md` 에 위치와 sha256 을 적는다.
- 비밀정보(API 키·서버 주소·계정)는 어디에도 넣지 않는다.

## 9. 변경 기록

| 날짜 | 변경 | 결정한 사람 |
|---|---|---|
| 2026-10-07 | 초판. 범위·입력·아키텍처·기준 방식·측정·단계 | 프로젝트 소유자 |
| 2026-10-07 | §2.2 합계 바이트 정정(530,814,804 → 530,796,804, PR #1 검토 H1), §6.3 RTX 4070 용량 12 GB 명시 | 프로젝트 소유자 |
| 2026-10-07 | §7.1 목표 수치(P0.7) 추가 | 감독(소유자 위임) |
| 2026-10-08 | §7.1 `mem_cpu` 를 목표에서 기록만으로(이전 목표 ≤ 300 MB 는 표에 남김). 이유: 메모리는 대역폭처럼 제약이 아니다. INTENT 원칙 2 에 메모리 추가. 참고: P1.5 native `mem_cpu` 미달(348.6 MB, 이전 방법)과 결정 0032 의 재측정 필요가 알려진 뒤의 변경이다 | 프로젝트 소유자(채팅) |
| 2026-10-08 | §7.1 서버 영상(P3) 목표 추가(제안 그대로, 화질은 native 캡처 대비), 프레임 빠짐 정의 | 프로젝트 소유자(채팅) |
| 2026-10-08 | §7.1 `frame_time_total` 기준값 정정 2.8 / 3.8 / 6.0 → 1.7 / 1.9 / 2.0 ms(기준 방식 재측정 2, C1–C4 준수, 밉맵 끔). 이유: 기준 측정 유효성 정정 — P0.6 원 측정은 C3 범위 밖 11/14·C4 미상. 판정 이력은 문서에 보존(docs/bench/baseline-three.md, validity-audit.md), 판정은 P4.6 C1–C4 전체 비교에서 새 기준으로. §7.1 에 판정 유효성(C1–C4) 문구 추가 | 감독(소유자 위임 2026-10-08) — 기준 측정 유효성 정정 |
