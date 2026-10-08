# STATUS

- 상태: 진행 중
- 현재 작업: P3 완료. P4 작업 쪼개기(작업자 — TASKS 에 P4 행 추가 PR)
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #28 P3.4 server video 측정 병합(`4f6ca7f`, [검토](reviews/pr-28.md)), 결정 0038 승인. 비교표 server video 열 = [docs/bench/server-video.md](../bench/server-video.md)
- SPEC §7.1 판정 요약:
  - native: preview·refined·cold·frame_time 통과
  - browser WebGPU(기본·전용): preview·refined·cold·main_thread_block 통과, **frame_time 미달**
  - browser WebGL2: preview·refined·cold 통과, **main_thread_block 미달(시작 1 회)**, **frame_time 미달**
  - server video: 전부 통과(중앙값). 단 프레임 빠짐은 **7 회 중 2 회가 1 % 초과**(4.86 %, 1.85 %)
  - 메모리·대역폭: 기록만(INTENT 원칙 2)
- 다음 할 일: P4 쪼개기. **P4 공개 전 진단**: ① 브라우저 frame_time(동기화 방법과 무관한 측정, PR #21) ② WebGL2 꼬리 ③ server video 클라이언트 쪽 프레임 빠짐(PR #28) ④ three.js 세션 간 frame_time 차이 원인
- 소유자 결정 대기: 없음
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음
