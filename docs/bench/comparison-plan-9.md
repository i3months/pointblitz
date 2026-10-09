# 측정 계획 9 — 최종 비교: as-is 대 to-be(P4.18, 측정 전 고정)

- 날짜: 2026-10-09. 소유자 결정(감독 전달): 소유자 보고서용으로 as-is 대 최종 to-be 를 같은 세션에서 새로 잰다. 판정 규칙은 결정 0049 §3, 그림 규칙은 결정 0048·0051. 바꾸려면 측정 전에 이 문서를 고친다.
- 빌드: **0.2.0 배포 커밋 664eaa1**(태그 v0.2.0). 하나의 빌드에서 모든 대상을 잰다.

## 대상
| 역할 | suite 이름 | 무엇 |
|---|---|---|
| **as-is** | `three-b2` | SkyLens 식 증분 three.js — PLY, 미리보기는 새 레코드만(`/data?have=`), Web Worker 해석, 정밀은 교체(결정 0048) |
| Rust 몫 기준(같은 데이터) | `three-b1`(올리기 (a)), `three-b1b`, `three-b1-webgpu` | three.js 가 PointBlitz 와 같은 v2 청크를 해석 없이 받고, 첫 패스 표시에서 새 세대로 바꿈(결정 0051) |
| **to-be** | `web`(WebGPU), `webgl`, `native`, `video` | PointBlitz 0.2.0. video 는 `VIDEO_DATA=1`(서버가 같은 장비의 데이터를 직접 읽음, 결정 0050) |

## 회수·방법
- `bench/suite.sh` 한 세션, 대상끼리 실행마다 번갈아: **replay ×60 10 회, cold 10 회, orbit 5 회(batch 30, GPU 타임스탬프)**.
  `IMPLS="three-b2 three-b1 three-b1b three-b1-webgpu web webgl native video" B1_UPLOAD=a VIDEO_DATA=1 ORBIT_BATCHES="30" COLD=10 ORBIT=5 X60=10`.
- 유효성·재실행: C1–C4, 실패·무효는 같은 조건으로 1 회만 다시, C2 는 끝에, 같은 실행이 다시 돌려도 무효면 멈추고 감독에게 알림. 측정 전 대기 GPU 확인(2 분 평균 ≤ 15 %).
- **받기 바닥**: 세션 시작·끝에 40 MB 정적 파일을 Chrome 이 받는 시간 5 회씩(`bench/fetch-floor.mjs`).
- 측정 동안 다른 빌드·GPU 작업 없음.

## 지표
- **지연**: 재생 미리보기·정밀의 **첫 반영**(`event_first_latency`)과 **완전 반영**(`event_latency`) p50, cold 첫·완전(video 는 화면 기준 이름이 있으면 그 값). B2 는 첫 반영이 따로 없어 첫 = 완전(정의대로).
- **그리기**: orbit batch 30 프레임당 시간 p50 / p95 / p99, GPU 타임스탬프 p50(결정 0049).
- **기록**: mem_cpu, JS 힙, 받은 바이트, 메인 스레드 멈춤(횟수·합).

## 판정(부트스트랩 10,000 번, 씨앗 48049, 95 % 구간이 1 을 포함하면 "차이 없음")
1. **as-is ÷ to-be**: B2 ÷ web, B2 ÷ webgl, B2 ÷ native, B2 ÷ video — 지연(첫·완전, 미리보기·정밀·cold), 1 보다 크면 to-be 가 빠름.
2. **같은 API 의 Rust·wgpu·wasm 몫**: B1\* ÷ webgl(WebGL2, B1\* = 지표마다 B1a·B1b 중 좋은 쪽, 어느 쪽인지 표시), B1-webgpu ÷ web(WebGPU) — 지연(첫·완전)과 그리기(batch 30, GPU 타임스탬프와 방향이 다르면 "판정 보류").
3. 목표 확인(SPEC §7.1): web·video 정밀 첫 반영 p50 ≤ 50 ms.

## 그림(고정 시점 8 곳 SSIM, 측정 뒤 같은 빌드로)
- to-be 끼리: web 대 native, webgl 대 native, video 대 native.
- as-is 대 to-be: B2 대 web.
- 각각 차이 픽셀 중 기준 그림의 점 영역 1 px 팽창 안 비율(결정 0051 의 근거 보충과 같은 방법).

## 결과 문서
- 표: 대상 8 × 지표, 판정표(1·2), 목표 확인, 받기 바닥, 그림. 결과가 불리해도 그대로 싣는다. 원 기록 경로를 적는다(저장소에 넣지 않음).
