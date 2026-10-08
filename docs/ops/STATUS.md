# STATUS

- 상태: 진행 중
- 현재 작업: P4.5 비용 모델 1단계(RTX 4070, PR #36 검토 중) · server video 프레임 고르게 하기(B, 감독 결정) · P4.6 초안
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #37 서버 영상 프레임 버퍼(B) 병합(`99fd180`, [검토](reviews/pr-37.md)) — 0039 조건부 승인: orbit 빠짐 5.5 → 0.28 %, 대가 입력→표시 +16~25 ms
- SPEC §7.1 판정 요약:
  - native: preview·refined·cold·frame_time 통과
  - browser WebGPU(기본·전용): preview·refined·cold·main_thread_block 통과, **frame_time 미달**
  - browser WebGL2: preview·refined·cold 통과, **main_thread_block 미달(시작 1 회)**, **frame_time 미달**
  - server video: 전부 통과(중앙값). 단 프레임 빠짐은 위상에 따라 두 덩어리 — 9 회 중 4 회 1 % 초과(최대 15.23 %), 손실 아님(P4.3)
  - 메모리·대역폭: 기록만(INTENT 원칙 2)
- 다음 할 일: **측정 유효성 C3·화면 기준 입력→표시 채택**([결정](reviews/measurement-c3-screen-latency.md)) — 지난 판정 실행 C3 감사, 적응형 pacing A/B 를 C3 로 다시 · P4.6 전 필수: 0039 수용 기준 · PR #36 후속 3건 · P4.5(V100 은 소유자 확인 뒤) · P4.6 · P4.7
- 소유자 결정 대기: (P4.5 시점) V100 장비 접근, (마지막) P4.8 공개 시점·대외 주장·npm 배포
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음
