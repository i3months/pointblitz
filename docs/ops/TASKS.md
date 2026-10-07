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
| P2.2 | `pointblitz-web` 그리기: canvas WebGPU 표면, core `Scene`·`Renderer`, 요청 때 그리기(결정 0027), 고정 시점 전환 | 마지막 스냅샷을 고정 시점 8곳에서 캡처해 native·three.js 캡처 대비 SSIM 을 기록한다 | [x] 2026-10-08 `6ea6dbb` |
| P2.3 | 브라우저 재생 클라이언트: `EventSource` + `fetch` 스트림으로 `/chunks` 를 받아 청크 단위로 장면에 넣기, 마크(`snapshot_received`·`presented` 등, 결정 0020 이름) | 14 이벤트 재생. replay ×60 에서 `main_thread_block` 0 회(SPEC §7.1)를 한 번 확인(판정은 P2.5) | [x] 2026-10-08 `42a0911` |
| P2.4 | WebGPU 가 없을 때 WebGL2(wgpu GL 백엔드)로 대체 | WebGPU 를 끈 Chrome 에서 14 이벤트 재생 + 고정 시점 SSIM 기록. 지원하지 않는 기능이 있으면 결정 기록 | [x] 2026-10-08 `df8182e` |
| P2.5 | browser 측정 + 비교표 browser 열: three.js · native 와 같은 세션 번갈아(`bench/suite.sh` 확장), WebGPU·WebGL2 둘 다 | `docs/bench/browser.md`. `mem_cpu` 판정은 SPEC 대로 렌더러 프로세스, GPU 프로세스 값을 함께 싣는다(PR #13 검토). wasm 크기·로드 시간 기록 | [x] 2026-10-08 `8de61d8` |

## P3 — server video

서버가 같은 `pointblitz-core` 로 헤드리스 렌더 → NVENC H.264 → WebSocket, 브라우저는 WebCodecs 로 디코드해 캔버스에 그리고 입력(카메라)을 같은 연결로 되돌려 보낸다(결정 0010).
서버는 재생 서버의 스냅샷을 그대로 따른다(`/events`·`/chunks` 와 같은 데이터). 대역폭·메모리는 기록만(INTENT 원칙 2) — 화질과 지연을 우선해 비트레이트를 정한다.

| # | 작업 | 완료 기준 | 상태 |
|---|---|---|---|
| P3.0 | 목표 수치(SPEC §7.1 "서버 영상은 P3 착수 때 정한다") — 제안은 [docs/notes/p3-targets-proposal.md](../notes/p3-targets-proposal.md) | 소유자 결정이 SPEC §7.1·§9 에 들어간다(P3.4 측정 전) | [ ] |
| P3.1 | 인코더 경로: 헤드리스 프레임(1920×1080) → NVENC H.264. NVENC 직접 호출(FFI) / 외부 ffmpeg 프로세스 등 비교 결정 기록(GPU→인코더 복사 여부, `unsafe` 범위, 라이선스) | 고정 시점 8곳·orbit 프레임을 인코딩→디코딩해 원본 렌더 대비 SSIM 과 프레임당 인코딩 시간(p50/p95/p99), 비트레이트를 기록한다 | [ ] |
| P3.2 | `pointblitz-server`: 재생 서버를 따라 장면을 갱신하고(세대·차분, 결정 0026 경로 재사용), 프레임을 인코딩해 WebSocket 으로 보냄. 프레임마다 메타데이터(프레임 번호, 반영된 스냅샷, 렌더·인코딩 시각, 마지막으로 반영한 입력 번호) | 14 이벤트 재생 동안 영상이 끊기지 않고 나가며, 테스트 클라이언트(헤드리스)가 받은 프레임 수·메타데이터를 검증한다 | [ ] |
| P3.3 | 브라우저 클라이언트: WebSocket 수신 → WebCodecs 디코드 → 캔버스, 마우스·키 입력을 번호와 함께 보냄. 마크(결정 0020 이름 + 입력→표시) | 14 이벤트가 브라우저 화면에 나타나고, 입력 번호가 돌아온 프레임으로 입력→표시 지연을 기록한다. `main_thread_block` 확인(판정은 P3.4) | [ ] |
| P3.4 | server video 측정 + 비교표 server video 열: three.js·native·browser 와 같은 세션 번갈아(`bench/suite.sh` 확장), 고정 시점 영상 SSIM(대 native 캡처), 비트레이트·인코딩·디코딩 시간, 서버 GPU 사용 | `docs/bench/server-video.md`, SPEC §7.1(P3.0 에서 정한 값) 판정 | [ ] |

WebRTC 비교(결정 0010 "P3 이후")는 P3.4 결과를 보고 따로 쪼갠다. P4 이후는 P3 가 끝나면 쪼갠다.
