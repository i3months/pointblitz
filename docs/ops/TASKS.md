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
| P0.6 | 기준 방식 측정 실행(RTX 4070 PC) + 결과 `docs/bench/baseline-three.md` | 비교표 three.js 열이 실측으로 채워진다(재현 명령 포함) | [ ] |
| P0.7 | 소유자에게 목표 수치 결정 요청 | STATUS `소유자 결정 대기` 에 올라간다 | [ ] |

## P1 — native

| # | 작업 | 완료 기준 | 상태 |
|---|---|---|---|
| P1.1 | `pointblitz-io`: PLY 스트림 파서 + 16 B 청크 변환(결정 0008·0014) | 세 레이아웃 픽스처, 증분 입력 시험, `flight-01` 14개 파일 점 수 일치 | [ ] |
| P1.2 | `pointblitz-core`: 세대·청크 저장소, GPU 버퍼, 점 그리기(결정 0006·0007·0009·0013) | 합성 장면 캡처가 기준 영상과 일치(픽셀 단위 시험) | [ ] |
| P1.3 | 고정 시점 캡처(헤드리스) + SSIM | three.js 캡처 대비 SSIM 기록 | [ ] |
| P1.4 | `pointblitz-native`: winit 창, 재생 클라이언트 | 14 이벤트 재생 | [ ] |
| P1.5 | native 측정 + 비교표 native 열 | `docs/bench/native.md` | [ ] |

P2 이후는 P1 이 끝나면 쪼갠다.
