# 측정 계획 3 — webgl 고침 뒤 그리기 처리량과 지연 몫의 신뢰 구간 (측정 전 고정)

- 날짜: 2026-10-09. [결정 0049](../decisions/0049-remove-remaining-ambiguity.md). 바꾸려면 측정 전에 이 문서를 고친다.

## 순서
1. 0049 와 이 계획 PR → 감독 검토·병합.
2. webgl 동기화 고침 PR(0049 §1 단위 확인 포함) + `bench/suite.sh` 에 batch 지정(아래) → 감독 재현·병합.
3. 측정 전 대기 GPU 확인(5 초 간격 2 분 평균 ≤ 15 %, 아니면 시작하지 않고 감독에게 알림) → 측정 → 시작·끝 알림.

## 세션 F — 그리기(frame_time)
- 대상 9: `three`(B0), `three-b1`(B1a), `three-b1b`, `three-b1-webgpu`, `three-b2`, `web`, `webs`, `webgl`, `native`.
- 회수: 대상마다 **orbit batch=30 5 회**와 **orbit batch=1 5 회**, 대상·batch 를 실행마다 번갈아.
- 판정값: batch=30 의 `frame_time_total` p50 / p95 / p99(프레임당). batch=1 은 기록(두 모드가 나오면 표시).
- 보조: GPU timestamp(되는 대상만, 0049 §2). 안 되는 대상은 이유.
- 같은 API 의 비 + 부트스트랩 95 % 구간(0049 §3): WebGL2 B1a·B1b 대 webgl, WebGPU B1-webgpu 대 web. 참고: native 대 B1\*.

## 세션 L — 지연
- 대상 5: `three-b1`(B1a), `three-b1b`, `webgl`, `three-b1-webgpu`, `web`.
- 회수: **replay ×60 10 회, cold 10 회**, 대상끼리 실행마다 번갈아.
- 지표: 재생 미리보기 p50, 재생 정밀 p50, cold 마지막 스냅샷, 메인 스레드 멈춤 횟수·합(기록), mem_cpu(기록).
- 판정: 0049 §3 — 비 = median(three.js 쪽) ÷ median(PointBlitz 쪽), 부트스트랩 10,000 번(씨앗 48049), 95 % 구간이 1 을 포함하면 "차이 없음". 쌍: B1a 대 webgl, B1b 대 webgl, B1-webgpu 대 web, 참고 B1\*(지표마다 B1a·B1b 중 좋은 쪽) 대 web.

## 유효성·실행 처리
P4.6·계획 2 와 같다: 모든 실행은 `bench/gpu-watch.mjs`, C1–C4, 실패·무효는 같은 조건으로 1 회만 다시, C2 는 `contaminated/`, 유효 실행이 계획의 절반 미만인 칸은 "유효 실행 부족". 같은 실행이 다시 돌려도 무효면 측정을 멈추고 감독에게 알린다.

## 결과 표
1. 그리기: 대상 9 × (batch=30 p50 / p95 / p99, batch=1 p50 / p95 / p99, GPU timestamp p50 또는 "안 됨: 이유").
2. 같은 API 의 비와 95 % 구간(그리기·지연), 판정(차이 없음 / PointBlitz 가 빠름 / three.js 가 빠름).
3. webgl 고침 전후(P4.6·p48 의 결함 값 대 고친 값).
4. 결과가 PointBlitz 에 불리해도 그대로 싣는다.
