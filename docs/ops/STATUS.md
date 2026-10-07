# STATUS

- 상태: 진행 중
- 현재 작업: P2.5 전 측정기 수정(mem_cpu = OS 최대 commit, PR #17 검토 H) → P2.5 browser 측정(작업자)
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #18 P2.4 WebGL2 대체 병합(`df8182e`, WebGL2 대 native SSIM 1.0000 — [검토](reviews/pr-18.md)), 결정 0031 승인. 그 전: PR #17 P2.3(`42a0911`)
- 목표(SPEC §7.1) native 판정: preview·refined·cold·frame_time 통과, **mem_cpu 348.6 MB 미달(목표 300 MB)** — 목표는 바꾸지 않는다(검토 노트 판단 1)
- 다음 할 일: P2.4 → P2.5. **P2.5 판정 전 필수: mem_cpu 를 OS 최대 commit 바이트로 바꾸고 세 구현을 같은 세션에서 다시 잰다(PR #17 검토 H)**
- 소유자 결정 대기: 없음
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음
