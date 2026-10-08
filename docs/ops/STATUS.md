# STATUS

- 상태: 진행 중
- 현재 작업: P4.7 마무리(README·데모·패키지 구조 — 남은 것은 검토 기록만의 정리), P4.8 npm 배포는 소유자 결정 대기
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: P4.5 완료 — Linux NVENC + V100 서버로 비용 모델 두 장비 보정(결정 0043, [검토](reviews/pr-51.md)). P4.7: README 첫 결과 공개(소유자 승인 "감독 검토 뒤 공개"), 데모 조작·Try it, npm 패키지 구조(배포 안 함). P4.6 완료(C1~C4 같은 세션 전체 비교)
- SPEC §7.1 판정(P4.6, C1~C4, 실행별 중앙값):
  - native: 모두 통과(frame_time p99 2.00 = 목표, 여유 없음)
  - browser WebGPU: frame_time 미달(프레임마다 동기화 비용 — 각주), 나머지 통과
  - browser WebGL2: frame_time·cold main_thread_block 미달
  - server video: 모두 통과. 단 프레임 빠짐은 1 % 초과 실행 2/5(최대 6.32 %, 위상 흐름 — 각주)
  - 메모리·대역폭: 기록만
- 다음 할 일: P4.7 남은 정리(cost-model 문구 중간 2건 등) → P4.8(npm 배포 등 남은 대외 공개, 소유자 결정)
- 소유자 결정 대기: P4.8 npm 패키지 배포·그 밖의 대외 알림(README 결과 공개는 승인됨)
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 측정 규칙: 모든 측정은 `bench/suite.sh` 또는 `bench/gpu-watch.mjs`(keep-display·전원 조절 끄기 포함), C1~C4, 시작·끝을 서로 알림
- 막힌 점: 없음
