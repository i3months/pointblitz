# STATUS

- 상태: 진행 중
- 현재 작업: 없음(다음 PR 대기). P4.8 중 npm 배포·그 밖의 대외 알림은 소유자 결정 대기
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: **crates.io 0.1.0 배포 완료**(소유자 결정 0044) — pointblitz, -io, -core, -native, -server, -web 여섯 크레이트, 출처 커밋 `8b8cb79`(태그 v0.1.0). 경과: 첫 실행은 접근할 수 없는 git 시험 의존으로 실패(결정 0046 으로 고침, 아무것도 안 올라감) → 두 번째는 계정 이메일 미인증(소유자 인증) → 다시 돌려 다섯 개 → `pointblitz-web` 은 새 크레이트 수 제한(429) → 이어서 올리는 릴리스(결정 0047)로 13:02 UTC 에 마저 올림. 그 전에 P4.5(V100 두 장비 보정), 성능 보고서(docs/bench/report.md), 서버 영상 120 fps 선택지(결정 0045)
- SPEC §7.1 판정(P4.6, C1~C4, 실행별 중앙값):
  - native: 모두 통과(frame_time p99 2.00 = 목표, 여유 없음)
  - browser WebGPU: frame_time 미달(프레임마다 동기화 비용 — 각주), 나머지 통과
  - browser WebGL2: frame_time·cold main_thread_block 미달
  - server video: 모두 통과. 단 프레임 빠짐은 1 % 초과 실행 2/5(최대 6.32 %, 위상 흐름 — 각주)
  - 메모리·대역폭: 기록만
- 다음 할 일: README 에 설치 안내(crates.io) 추가(작업자 PR), P4.8 의 남은 대외 공개(npm 등)는 소유자 결정
- 소유자 결정 대기: P4.8 npm 패키지 배포·그 밖의 대외 알림(README 결과 공개와 crates.io 배포는 끝남)
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 측정 규칙: 모든 측정은 `bench/suite.sh` 또는 `bench/gpu-watch.mjs`(keep-display·전원 조절 끄기 포함), C1~C4, 시작·끝을 서로 알림
- 막힌 점: 없음
