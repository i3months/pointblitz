# STATUS

- 상태: 진행 중
- 현재 작업: P3.4 server video 측정(작업자). 측정 전: serve orbit 시나리오, 입력 최대 이상치 원인 분리
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #27 P3.3 브라우저 영상 클라이언트 병합(`18b7953`, [검토](reviews/pr-27.md)), 결정 0037 승인(VUI 수정은 기능 결함 수정으로 인정)
- SPEC §7.1 판정 요약(같은 세션, P2.5):
  - native: preview·refined·cold·frame_time 통과
  - browser WebGPU(기본·전용): preview·refined·cold·main_thread_block 통과, **frame_time 미달**(동기화 방법 차이 가설 — 진단 필요)
  - browser WebGL2: preview·refined·cold 통과, **main_thread_block 미달(시작 1 회)**, **frame_time 미달**
  - mem_cpu: 기록만(2026-10-08 소유자 결정)
- 다음 할 일: P3.4. 프레임 빠짐 판정은 orbit·첫 프레임 뒤([p3-frame-drop-window](reviews/p3-frame-drop-window.md)). P4 공개 전 frame 진단(PR #21 검토)
- 소유자 결정 대기: 없음(외부 파일 다운로드가 필요해지면 작업자가 소유자에게 직접 허락을 받는다 — 작업자 운영 규칙)
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음(PR #24 병합 뒤 main Ubuntu CI 실패는 PR #25 로 복구)
