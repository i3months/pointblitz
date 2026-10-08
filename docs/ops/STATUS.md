# STATUS

- 상태: 진행 중
- 현재 작업: SPEC §7.1 frame_time 기준값 정정 PR(작업자) · 서버 위상 맞추기 제안(PR #41) 검토
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #38 병합(`0f22906`, [검토](reviews/pr-38.md)) — 결정 0040(C3·화면 기준 지연·keep-display)·0041(C4·측정 프로세스 전원 조절 끄기) 승인, 0039 는 선택지로만(기본 pacing immediate). P0.6 기준 방식 C1~C4 재측정 frame_time 1.7 / 1.9 / 2.0 ms → **감독 결정: SPEC frame_time 목표를 이 값으로 정정**(소유자 위임)
- SPEC §7.1 판정 요약(지난 단계는 "C4 미상", [validity-audit](../bench/validity-audit.md)):
  - native: preview·refined·cold·frame_time 통과(이전 기준)
  - browser WebGPU: frame_time 미달(동기화 비용 — P4.1), 나머지 통과
  - browser WebGL2: main_thread_block·frame_time 미달
  - server video: P3.4 전부 통과(C4 미상). **C1~C4 측정에서는 기본(immediate) orbit 프레임 빠짐 5/5 회 > 1 % → 미달**. 고칠 길 C(서버 위상 맞추기) 제안 중
  - 메모리·대역폭: 기록만
- 다음 할 일: SPEC 정정 PR → C 제안 결정 → **P4.6 전: C1~C4 같은 세션 전체 비교**(새 frame_time 기준으로 재판정) → P4.6 공개표 → P4.7. P4.5 는 V100(소유자 확인 대기)
- 소유자 결정 대기: (P4.5) V100 장비 접근, (마지막) P4.8 공개 시점·대외 주장·npm 배포
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 측정 규칙: 모든 측정은 `bench/gpu-watch.mjs`(keep-display·전원 조절 끄기 포함), C1~C4, 시작·끝을 서로 알림
- 막힌 점: 없음
