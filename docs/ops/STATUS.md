# STATUS

- 상태: 진행 중
- 현재 작업: P4.7 README(영문·한글)·데모 페이지·JS 래퍼(패키지 구조만)
- 마지막 갱신: 2026-10-08 (UTC)
- 방금 한 일: P4.6 완료 — C1~C4 같은 세션 전체 비교([comparison.md](../bench/comparison.md), [검토](reviews/pr-46.md)). 그 전에 결정 0042(쓰기 스레드 송신 기본, 위상 잠금은 끔으로 확정), SPEC frame_time 기준값 정정(1.7 / 1.9 / 2.0 ms)
- SPEC §7.1 판정(P4.6, C1~C4, 실행별 중앙값):
  - native: 모두 통과(frame_time p99 2.00 = 목표, 여유 없음)
  - browser WebGPU: frame_time 미달(프레임마다 동기화 비용 — 각주), 나머지 통과
  - browser WebGL2: frame_time·cold main_thread_block 미달
  - server video: 모두 통과. 단 프레임 빠짐은 1 % 초과 실행 2/5(최대 6.32 %, 위상 흐름 — 각주)
  - 메모리·대역폭: 기록만
- 다음 할 일: P4.7(README 주장은 comparison.md 와 맞출 것 — 빠짐 분포·native "같은 수준" 표현, 공개 wasm 크기는 CI 값). P4.5 는 V100(소유자 확인 대기)
- 소유자 결정 대기: (P4.5) V100 장비 접근, (마지막) P4.8 공개 시점·대외 주장·npm 배포
- 외부 팀과 정할 것: skyrecon 이 preview "덧붙임" 메타데이터를 낼지([docs/notes/skyrecon-append-metadata.md](../notes/skyrecon-append-metadata.md))
- 측정 규칙: 모든 측정은 `bench/suite.sh` 또는 `bench/gpu-watch.mjs`(keep-display·전원 조절 끄기 포함), C1~C4, 시작·끝을 서로 알림
- 막힌 점: 없음
