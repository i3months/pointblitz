# 0049 남은 모호함 세 가지를 실험으로 없앤다 — webgl 동기화 고침, 그리기 판정값 batch=30, 지연 몫의 신뢰 구간

- 상태: **승인**(감독, PR #68 검토 2026-10-09) — 측정 전 고정
- 날짜: 2026-10-09
- 결정한 사람: 감독(소유자 지시 "모호한 부분이 있으면 다시 실험해서 없앨 것" 에 따라 감독이 정함), 작업자(방법)
- 관련: 0020(동기화 프레임), 0033(화면 밖 그리기), 0048(증분 기준), [comparison-incremental.md](../bench/comparison-incremental.md)(중간 결과), [측정 계획 3](../bench/comparison-plan-3.md)

## 맥락
0048 의 중간 결과(p48·p48-orbit)에 판정을 못 한 것이 셋 남았다.
1. **PointBlitz webgl 의 frame_time 은 측정 결함**: 동기화가 캔버스(기본 프레임버퍼)를 1 픽셀 `readPixels` 하는데 PointBlitz 는 화면 밖 텍스처에 그린다. 드라이버가 그 그리기를 기다리지 않아 처음 약 420–480 프레임이 0.0–0.2 ms 로 보이고, 큐가 찬 뒤에야 처리 속도(약 4.5–5 ms)가 보인다(`cpu == ms`). P4.6 부터 같은 모양이다.
2. **WebGPU 의 매 프레임 동기화 값은 두 모드**(약 3 ms / 약 10–15 ms, 시점 5 부터, 원인 미확인). 빠른 모드의 3 ms 도 대부분 동기화(1 픽셀 복사 + `mapAsync`) 비용이다.
3. **지연의 Rust·wgpu·wasm 몫(같은 API 끼리 0.72–1.26 배)이 흔들림 안인지 밖인지** 실행 3–5 회로는 가를 수 없다.

## 결정
### 1. webgl 동기화를 고친다
- PointBlitz web 의 회전 동기화를 **WebGL2 에서도 WebGPU 와 같은 방법**으로 바꾼다: 그 프레임이 그린 화면 밖 텍스처의 첫 픽셀을 작은 버퍼로 복사하고 맵을 기다린다(`viewer.readback_done()`). 그 텍스처를 읽으므로 그리기가 끝나야 끝난다. 캔버스를 읽던 `readPixels` 는 지운다.
- 단위 확인(고침 PR 에서): 고친 뒤 회전 1 회에서 **시점마다 0.5 ms 미만 프레임이 0 개**이고 `ms > cpu` 여야 한다. 그리고 시점 1–4 와 5–8 의 중앙값 차이가 30 % 안이어야 한다(0.1 대 5 ms 같은 갈림이 없음).
- **P4.6 의 공개 webgl frame_time(3.5 / 6.0 / 13.5 ms)은 무효**로 표시하고 정정 대상에 넣는다(공개 문구 정정은 이 결정의 측정 결과까지 나온 뒤 한 번에).

### 2. 그리기 판정값 = batch=30 처리량
- 이 비교의 frame_time **판정값은 batch=30**(30 프레임을 연달아 그리고 한 번 동기화, 프레임당 시간 = 전체 ÷ 30)으로 미리 정한다. 동기화 한 번의 비용이 30 프레임에 나뉘므로 동기화 방식(readPixels / mapAsync / native fence)의 차이와 WebGPU 두 모드의 영향이 1/30 로 준다. 모든 대상(B0·B1a·B1b·B1-webgpu·B2·web·webs·webgl·native)에 같은 방법.
- batch=1 값(기존 정의, SPEC §6.2)도 같은 세션에서 재서 함께 싣는다. 두 모드가 나오면 표시만 하고 판정에 쓰지 않는다.
- **GPU timestamp 는 보조 기록**으로 시도한다: WebGPU `timestamp-query`, WebGL2 `EXT_disjoint_timer_query_webgl2`, native wgpu `TIMESTAMP_QUERY`. 쓸 수 없으면(브라우저가 기능을 노출하지 않음, 값이 거칠게 양자화됨 등) 그 이유를 결과에 적는다. 판정에는 쓰지 않는다.

### 3. 지연의 몫을 신뢰 구간으로 가른다
- 같은 API 쌍만 비교한다: **WebGL2 — B1a·B1b 대 PointBlitz webgl**, **WebGPU — B1-webgpu 대 PointBlitz web**, 참고(API 섞임) — B1\* 대 web.
- 회수: replay ×60 **10 회**, cold **10 회**, 쌍의 대상끼리 실행마다 번갈아(한 세션).
- 지표: 재생 미리보기 p50, 재생 정밀 p50, cold 마지막 스냅샷(지금까지와 같은 정의), 메인 스레드 멈춤 횟수(기록).
- **판정 방법(측정 전 고정)**: 지표마다 비 = median(three.js 쪽) ÷ median(PointBlitz 쪽). 부트스트랩: 두 대상의 실행 값을 각각 복원 추출로 다시 뽑아(실행 수 그대로) 비를 10,000 번 계산, 2.5–97.5 백분위를 95 % 구간으로 한다(난수 씨앗 고정 48049). **구간이 1 을 포함하면 "차이 없음"**, 1 보다 크면 "PointBlitz 가 빠름", 작으면 "three.js 가 빠름". frame_time(batch=30)에도 같은 방법을 쓴다(회수는 계획 3).

### 4. 측정 전 보충(2026-10-09, 감독 결정 — PR #69 단위 확인 뒤, 측정 전)
- **같은 API 쌍은 같은 동기화 방법**: B1-webgpu 의 회전 동기화를 PointBlitz web 과 같게 바꾼다 — 화면 밖 렌더 타깃에 그리고 그 1 px 를 복사 + `mapAsync`(three.js `readRenderTargetPixelsAsync`). 지금까지 쓴 `onSubmittedWorkDone` 은 같은 API 인데 방법이 달라 batch=30 에서도 차이가 남을 수 있다. WebGL2 기준(B0·B1·B2)은 자기가 그린 캔버스를 `readPixels` 하므로 그대로(그 읽기는 그리기를 기다린다).
- **GPU timestamp 를 9 대상 모두 같은 절차로 기록**한다(보조 → 판정 확인용): 회전의 batch 마다 그 batch 의 GPU 시간 ÷ batch.
  - WebGL2 대상(B0·B1a·B1b·B2·PointBlitz webgl): 같은 GL 컨텍스트에서 `EXT_disjoint_timer_query_webgl2` 의 `TIME_ELAPSED` 를 batch 앞뒤에 건다. wgpu 의 GL 백엔드는 timestamp query 를 지원하지 않지만 GL 호출을 제출(`render_offscreen`) 안에서 바로 내보내므로 바깥 쿼리가 같은 GPU 일을 잰다. `GPU_DISJOINT` 가 서면 그 batch 는 버린다.
  - WebGPU 대상(web·webs·B1-webgpu)과 native: 렌더 패스의 `timestampWrites`(시작·끝)로 프레임마다 재고 batch 안에서 더한다. three.js 는 `trackTimestamp` + `resolveTimestampsAsync`.
  - Chrome 은 WebGPU timestamp 를 거칠게(약 0.1 ms 단위) 줄 수 있다 — 결과에 적는다.
- **판정 확인 규칙**: 같은 API 쌍에서 batch=30 비와 GPU timestamp 비의 **방향이 서로 다르면**(한쪽은 1 보다 크고 다른 쪽은 작음, 또는 한쪽 구간만 1 을 포함) 그 칸은 **"판정 보류 — 원인 조사"** 로 둔다. 방향이 같으면 batch=30 이 판정값이다. 이로써 WebGL2 의 맵 완료가 화면 주기에 묶여 batch=30 에 최대 약 1.1 ms 얹히는 문제(PR #69)도 timestamp 로 드러난다.

## 대가
- 측정이 길다(계획 3, 약 1.5 시간). 공개 문구 정정은 그 뒤 한 번에 한다.
- batch=30 은 "한 프레임을 그려 보여 주는 데 걸리는 시간" 이 아니라 "연달아 그릴 때의 처리량" 이다. SPEC §6.2 의 frame_time(batch=1)과 다르다는 것을 표에 적는다.

## 다시 볼 조건
- WebGPU 두 모드의 원인이 밝혀지면 batch=1 값도 판정에 다시 쓸지 본다.
