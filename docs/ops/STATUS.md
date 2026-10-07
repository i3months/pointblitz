# STATUS

- 상태: 진행 중
- 현재 작업: P2.5 browser 측정 + 비교표 browser 열(작업자)
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: PR #19 mem_cpu = OS 최대 commit 병합(`8b123a5`, [검토](reviews/pr-19.md)), 결정 0032 승인(0020 메모리 행 대체). 그 전: PR #18 P2.4(`df8182e`)
- 목표(SPEC §7.1) native 판정: preview·refined·cold·frame_time 통과, **mem_cpu 348.6 MB 미달(목표 300 MB)** — 목표는 바꾸지 않는다(검토 노트 판단 1)
- 다음 할 일: P2.5 — three.js·native·browser(WebGPU·WebGL2) 같은 세션 번갈아, mem_cpu 새 방법으로 전부 재측정·재판정(native 포함), WebGL2 시작 블록 원인 분리, WebGPU 전용 빌드 A/B
- 소유자 결정 대기: 없음
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 막힌 점: 없음
