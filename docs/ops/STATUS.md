# STATUS

- 상태: 진행 중
- 현재 작업: P3.1 인코더 경로(작업자). P3.0 목표 수치는 소유자 확인 중
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #22 TASKS P3 쪼개기 + 목표 제안 병합(`3a23b0f`, [검토](reviews/pr-22.md)). 그 전: PR #21 P2.5(`8de61d8`) — P2 완료
- SPEC §7.1 판정 요약(같은 세션, P2.5):
  - native: preview·refined·cold·frame_time 통과
  - browser WebGPU(기본·전용): preview·refined·cold·main_thread_block 통과, **frame_time 미달**(동기화 방법 차이 가설 — 진단 필요)
  - browser WebGL2: preview·refined·cold 통과, **main_thread_block 미달(시작 1 회)**, **frame_time 미달**
  - mem_cpu: 기록만(2026-10-08 소유자 결정)
- 다음 할 일: P3.1 → P3.4. P3.0 목표는 P3.4 측정 전에 확정(소유자 답이 없으면 감독이 정함, 위임). P4 공개 전 frame 진단(PR #21 검토)
- 소유자 결정 대기: P3.0 서버 영상 목표 수치([제안](../notes/p3-targets-proposal.md)) — P3.4 전까지 답이 없으면 감독 결정(위임)
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음
