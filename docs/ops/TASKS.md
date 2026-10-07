# TASKS

규칙: 첫 미완료 작업부터 한다. 완료 표시는 감독이 병합할 때 한다(날짜·병합 커밋). 단계 정의는 [SPEC §7](../../SPEC.md).

## P0 — 기준 방식 측정

| # | 작업 | 완료 기준 | 상태 |
|---|---|---|---|
| P0.1 | Cargo 워크스페이스 뼈대(크레이트 6개 빈 상태), 툴체인 고정, CI(빌드·fmt·clippy·시험, Windows·Linux) | `cargo build --workspace`·`cargo test --workspace` 가 CI 에서 통과 | [x] 2026-10-07 `fbeaaed` |
| P0.2 | 고정 시점 8곳 `bench/viewpoints/flight-01.json`(ENU, 위치·목표·위·시야각·해상도) + 결정 기록 | 마지막 스냅샷 경계 상자 기준으로 정한 규칙이 결정 기록에 있고, 8곳 모두 점이 화면에 들어온다(캡처로 확인) | [x] 2026-10-07 `fb50aad`(캡처 대신 CPU 투영 검사 + 감독 독립 래스터, [검토](reviews/pr-3.md)) |
| P0.3 | 시나리오 재생 서버: `flight-01` 을 HTTP 로 서빙하고 이벤트 시각을 배속으로 알림(기준 방식·새 방식 공용) | 배속 ×1·×60 재생, 이벤트 순서·시각 로그 | [x] 2026-10-07 `90ec067` |
| P0.4 | 기준 방식 `baseline/three`: three.js 0.185 `PLYLoader` + `Points`, SPEC §5 그대로 | `replay`·`orbit`·`cold` 시나리오 실행, 화면 캡처 저장(고정 시점 8곳 캡처로 P0.2 재확인) | [x] 2026-10-07 `ddd068c` |
| P0.5 | 브라우저 지표 수집(Chrome, CDP): 전송 바이트, 이벤트 반영 시간, 첫 화면, 프레임 시간, 메인 스레드 블록, JS 힙 | SPEC §6.2 지표가 JSON Lines 로 나온다 | [x] 2026-10-07 `53d564e` |
| P0.6 | 기준 방식 측정 실행(RTX 4070 PC) + 결과 `docs/bench/baseline-three.md` | 비교표 three.js 열이 실측으로 채워진다(재현 명령 포함) | [x] 2026-10-07 `d180c19` |
| P0.7 | 소유자에게 목표 수치 결정 요청 | STATUS `소유자 결정 대기` 에 올라간다 | [x] 2026-10-07 `5a572d6`(STATUS 등재) → 목표 수치는 감독 결정(소유자 위임), [근거](reviews/p0-7-targets.md) |

## P1 — native

| # | 작업 | 완료 기준 | 상태 |
|---|---|---|---|
| P1.1 | `pointblitz-io`: PLY 스트림 파서 + 16 B 청크 변환(결정 0008·0014) | 세 레이아웃 픽스처, 증분 입력 시험, `flight-01` 14개 파일 점 수 일치 | [x] 2026-10-07 `9e7ce15` |
| P1.2 | `pointblitz-core`: 세대·청크 저장소, GPU 버퍼, 점 그리기(결정 0006·0007·0009·0013) | 합성 장면 캡처가 기준 영상과 일치(픽셀 단위 시험) | [x] 2026-10-07 `6adc59c` |
| P1.3 | 고정 시점 캡처(헤드리스) + SSIM | three.js 캡처 대비 SSIM 기록 | [x] 2026-10-07 `417c613` |
| P1.4 | `pointblitz-native`: winit 창, 재생 클라이언트 | 14 이벤트 재생 | [x] 2026-10-08 `7f597c5` |
| P1.5 | native 측정 + 비교표 native 열 | `docs/bench/native.md` | [x] 2026-10-08 `7282c2c` |

## P2 — browser (wasm)

같은 `pointblitz-core` 를 wasm 으로 빌드해 브라우저에서 돌린다. 서버 경로(`/events`, `/chunks`)는 native 와 같다(결정 0026). 실행 시점에 Node 는 쓰지 않는다(빌드·측정 도구만).

| # | 작업 | 완료 기준 | 상태 |
|---|---|---|---|
| P2.1 | wasm 빌드 경로: `wasm-bindgen`(CLI 버전 고정) + `web-sys`, 빌드 스크립트, 페이지 뼈대(`web/`), CI 에 wasm 빌드 추가 + 결정 기록(도구 선택, wasm 크기) | 재생 서버 `/static` 으로 연 페이지가 Chrome(헤드리스, 측정과 같은 플래그)에서 WebGPU 어댑터 정보를 출력한다. CI 에서 wasm 산출물이 만들어진다 | [x] 2026-10-08 `9fca9bb` |
| P2.2 | `pointblitz-web` 그리기: canvas WebGPU 표면, core `Scene`·`Renderer`, 요청 때 그리기(결정 0027), 고정 시점 전환 | 마지막 스냅샷을 고정 시점 8곳에서 캡처해 native·three.js 캡처 대비 SSIM 을 기록한다 | [ ] |
| P2.3 | 브라우저 재생 클라이언트: `EventSource` + `fetch` 스트림으로 `/chunks` 를 받아 청크 단위로 장면에 넣기, 마크(`snapshot_received`·`presented` 등, 결정 0020 이름) | 14 이벤트 재생. replay ×60 에서 `main_thread_block` 0 회(SPEC §7.1)를 한 번 확인(판정은 P2.5) | [ ] |
| P2.4 | WebGPU 가 없을 때 WebGL2(wgpu GL 백엔드)로 대체 | WebGPU 를 끈 Chrome 에서 14 이벤트 재생 + 고정 시점 SSIM 기록. 지원하지 않는 기능이 있으면 결정 기록 | [ ] |
| P2.5 | browser 측정 + 비교표 browser 열: three.js · native 와 같은 세션 번갈아(`bench/suite.sh` 확장), WebGPU·WebGL2 둘 다 | `docs/bench/browser.md`. `mem_cpu` 판정은 SPEC 대로 렌더러 프로세스, GPU 프로세스 값을 함께 싣는다(PR #13 검토). wasm 크기·로드 시간 기록 | [ ] |

P3 이후는 P2 가 끝나면 쪼갠다.
