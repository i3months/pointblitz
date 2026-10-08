# STATUS

- 상태: 진행 중
- 현재 작업: P4.4 진단 — three.js 세션 간 frame_time 차이(작업자)
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #33 P4.3 진단 병합(`43d4464`, [검토](reviews/pr-33.md)) — server video 빠짐은 위상 문제(손실 아님), 실행의 약 40 % 가 1 % 초과
- SPEC §7.1 판정 요약:
  - native: preview·refined·cold·frame_time 통과
  - browser WebGPU(기본·전용): preview·refined·cold·main_thread_block 통과, **frame_time 미달**
  - browser WebGL2: preview·refined·cold 통과, **main_thread_block 미달(시작 1 회)**, **frame_time 미달**
  - server video: 전부 통과(중앙값). 단 프레임 빠짐은 위상에 따라 두 덩어리 — 9 회 중 4 회 1 % 초과(최대 15.23 %), 손실 아님(P4.3)
  - 메모리·대역폭: 기록만(INTENT 원칙 2)
- 다음 할 일: P4.1 → P4.7. P4.5 의 V100 접근과 P4.8 공개는 소유자 결정
- 소유자 결정 대기: (P4.5 시점) V100 장비 접근, (마지막) P4.8 공개 시점·대외 주장·npm 배포
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음
