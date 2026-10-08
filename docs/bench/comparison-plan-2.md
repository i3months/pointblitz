# 증분 기준(B1·B2) 비교 측정 계획 — 측정 전 고정

- 날짜: 2026-10-09. 감독 지시(소유자 지적)에 따른 계획, [결정 0048](../decisions/0048-incremental-baselines.md). 바꾸려면 측정 전에 이 문서를 고친다.
- 묻는 것: Rust·wgpu·wasm 을 도입해서 얻은 차이. 구조(증분·바이너리 전달)의 몫과 나눠서 보인다.

## 순서
1. 결정 0048 검토 → 구현(B1·B1-webgpu·B2, 서버 `/data?have=`) → 단위 시험 → 그림 확인(SSIM) → **B1 올리기 스모크**((a) 두 번 올리기 대 (b) CPU 로 나눠 복사, cold·replay ×60 각 2 회 번갈아 — 빠른 쪽이 B1) → 결과를 0048 에 적고 감독 확인.
2. 그다음 같은 세션 전체 비교 한 번(아래). 측정 전·후에 감독에게 알린다.

## 대상·시나리오·회수
| suite 이름 | 라벨 | 무엇 |
|---|---|---|
| `three` | `three.js` | B0 — 0005 기준(비교용으로 유지) |
| `three-b1` | `three-b1` | B1 — three.js + 같은 청크 경로 |
| `three-b1-webgpu` | `three-b1-webgpu` | B1 을 three.js WebGPURenderer 로(되면) |
| `three-b2` | `three-b2` | B2 — three.js SkyLens 식 증분(워커 해석) |
| `web` | `web-webgpu` | PointBlitz web(WebGPU 기본 모듈) |
| `webs` | `web-webgpu-only` | WebGPU 전용 모듈 |
| `webgl` | `web-webgl2` | WebGL2 |
| `native` | `native` | 데스크톱 |
| `video` | `video` | 서버 영상(기본값) |

- 시나리오: cold 5 회, orbit 5 회, replay ×60 3 회. 대상끼리 실행마다 번갈아(`bench/suite.sh`, `IMPLS="three three-b1 three-b1-webgpu three-b2 web webs webgl native video"`(B1-webgpu 가 빠지면 그 이름만 뺀다)).
- 모든 실행은 `bench/gpu-watch.mjs`(→ `measure-env.ps1`).

## 유효성과 실행 처리
P4.6 계획([comparison-plan.md](comparison-plan.md))과 같다: C1–C4, 실패·무효는 같은 조건으로 1 회만 다시, C2 는 `contaminated/`, 유효 3 회 미만이면 "유효 실행 부족".

## 지표
- 기존 지표 전부(SPEC §6.2): `event_latency` preview / refined(화면 기준, p50·p95·최대), cold `first_frame`·`full_frame`, orbit `frame_time` p50 / p95 / p99(동기화 프레임), `bytes_received`.
- 추가로 표에 꼭 싣는 것: **메인 스레드 멈춤**(50 ms 넘는 long task 수, 최대), **mem_cpu**(렌더러·GPU 프로세스 최대 커밋), **JS 힙**(최대).
- B1·B2 의 단계 마크: `fetch_start` / `fetch_end`(받은 바이트) / `parse_end`(B2: 워커에서 돌아온 시각, B1: 마지막 청크를 올린 시각) / `scene_swap` / `presented`.

## 판정표 형식
1. **전체 표**: 대상 8 개 × 지표(중앙값, 최소–최대, 유효 실행 수). SPEC 목표 판정은 PointBlitz 대상에만(기준 B0·B1·B2 는 판정하지 않음).
2. **몫 나누기 표**(지표마다, 중앙값 비):

| 지표 | B0 | B1 | B1-webgpu | web(WebGPU) | webgl(WebGL2) | 구조의 몫 B0 ÷ B1 | **같은 API** B1 ÷ webgl | **같은 API** B1-webgpu ÷ web | 참고 B1 ÷ web(API 섞임) | 합 B0 ÷ web |
|---|---|---|---|---|---|---|---|---|---|---|

   - frame_time 처럼 작을수록 좋은 값은 같은 방향(큰 쪽 ÷ 작은 쪽, 1 보다 작으면 "느려짐" 으로 적는다).
3. **as-is 표**: B2 대 PointBlitz web·native·video.
4. 각주: 그림 확인 결과(SSIM), B1 올리기 스모크 결과(진 쪽 값), **해석 위치 비교** — 스냅샷별 B2 워커 해석 시간 대 같은 스냅샷의 서버 Rust 변환 시간(서버 로그).

## 이 계획이 지키는 것
- 일부러 약하게 만든 기준이 없다: B1 은 PointBlitz 와 같은 바이트를 받고 올리기 방식은 스모크로 빠른 쪽을 고르며, B2 는 SkyLens 실제(메인 스레드 해석)보다 유리한 워커 해석이다. 기준은 "가장 강한 합리적 three.js".
- 측정 뒤 결과가 PointBlitz 에 불리해도 그대로 싣는다.
