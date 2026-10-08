# STATUS

- 상태: 진행 중
- 현재 작업: P4.5 비용 모델 1단계(RTX 4070, PR #36 검토 중) · server video 프레임 고르게 하기(B, 감독 결정) · P4.6 초안
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #39 비용 모델 후속 병합(`059270e`). PR #38(0040 적응형)·#40(P0.6 재측정)은 측정 환경 문제로 보류
- SPEC §7.1 판정 요약:
  - native: preview·refined·cold·frame_time 통과
  - browser WebGPU(기본·전용): preview·refined·cold·main_thread_block 통과, **frame_time 미달**
  - browser WebGL2: preview·refined·cold 통과, **main_thread_block 미달(시작 1 회)**, **frame_time 미달**
  - server video: 전부 통과(중앙값). 단 프레임 빠짐은 위상에 따라 두 덩어리 — 9 회 중 4 회 1 % 초과(최대 15.23 %), 손실 아님(P4.3)
  - 메모리·대역폭: 기록만(INTENT 원칙 2)
- 다음 할 일: 결정 0040(C3·keep-display·display_hz) → 적응형 pacing A/B(C3) → **P0.6 기준 방식 재측정**(P0.6 14 회 중 11 회가 화면 꺼짐으로 C3 범위 밖 — SPEC frame_time 목표 근거 정정, 감독 결정) · PR #36 후속 3건 · P4.5(V100 은 소유자 확인 뒤) · P4.6 · P4.7
- 소유자 결정 대기: (P4.5 시점) V100 장비 접근, (마지막) P4.8 공개 시점·대외 주장·npm 배포
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: **측정 환경 — 13:50 무렵부터 PC 의 CPU 쪽이 느린 상태**(작업자 P0.6 재측정의 해석 속도 절반, 감독 PR #38 재현의 서버 틱 지연 p95 23.5 ms). C1~C3 로 못 잡는다 → C4(CPU 상태) 규칙을 정하고 다시 잴 때까지 **PR #38 병합·P0.6 기준값 결정 보류**
