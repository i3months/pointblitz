# 0048 증분 three.js 기준(B1·B2) — 비교 기준 다시 정의

- 상태: **승인**(감독, PR #64 검토 2026-10-09) — 측정 전 고정. 결과는 측정 PR 에서
- 날짜: 2026-10-09
- 결정한 사람: 감독(소유자 지적에 따름, 소유자에게 묻지 않아도 되는 범위로 감독이 정함), 작업자(구현 방법)
- 대체: [0005](0005-baseline-definition.md)(비교용 열 B0 으로 남김)
- 관련: 0019·0025(점 모양), 0022·0026(청크·전달), 0038(측정 규칙), [측정 계획](../bench/comparison-plan-2.md)

## 맥락
- 소유자 질문: **"Rust·wgpu·wasm 을 도입해서 얻은 성능 차이"** 가 얼마인가.
- 0005 의 기준(B0: 이벤트마다 PLY 전체 받기 → 메인 스레드 `PLYLoader` → `Points` 통째로 교체)은 증분이 없다. 지금까지 공개한 배율(5–23 배)에는 **구조(증분·바이너리 전달)의 몫과 Rust·wgpu·wasm 의 몫이 섞여 있다**.
- SkyLens 의 핵심은 온라인 증분이다. 코드 근거(SkyLens 원격 최신 0122bd4 기준):
  - `src/shared/protocol.ts` `SplatChunk`: 조각 단위(`segment`)와 정밀도 단계(`level`)를 가진다. 주석 "1 lands first, a higher one REPLACES it".
  - b7a729d 이전 `src/skylens_client/statusview/splatScene.ts`: `DropInViewer({ dynamicScene: true, … })` 에 조각마다 장면을 하나 두고, 새 조각은 **덧붙이고**, 같은 조각의 높은 `level` 은 **그 조각만 교체**한다("keeps at most one scene per segment — a higher level for a segment replaces the lower one").
  - 그 라이브러리(@mkkellogg/gaussian-splats-3d 0.4.7, `src/loaders/ply/PlyLoader.js`)는 받는 대로 구간을 나눠 해석하지만(progressive), 해석 자체는 **메인 스레드**의 `delayedExecute`(= `window.setTimeout`)에서 한다. 워커는 정렬(`SortWorker`)과 `SplatTree` 에만 쓴다. → 감독 메시지의 "해석은 라이브러리 워커" 와 다르다. 아래 B2 는 워커로 해석해서 **SkyLens 보다 유리하게** 둔다.
- skyrecon 데이터: 미리보기는 앞 스냅샷에 점을 덧붙인 것(바이트 접두가 같음), 정밀은 전체를 다시 쓴 것(SPEC §2.2). 그래서 "조각 덧붙이기 + 정밀은 교체" 가 이 데이터에서 SkyLens 식 증분에 해당한다.

## 결정
세 기준을 둔다. 모두 three.js 0.185.1, 같은 페이지(`baseline/three/main.js`, `mode=` 로 고름), 같은 재생 서버.

| 이름 | 받는 것 | 해석 | 장면 갱신 | 그리기 | 묻는 것 |
|---|---|---|---|---|---|
| **B0** (0005, `mode=full`) | 이벤트마다 PLY 전체(`/data`) | 메인 스레드 `PLYLoader` | `Points` 통째로 교체 | 매 rAF | 지금까지의 비교(유지) |
| **B1** (`mode=b1`) | PointBlitz 와 **같은** `/chunks/<seq>?have=` 스트림(GPU 배치 바이너리, 미리보기는 차분) | 없음 — 청크 바이트에 `Float32Array`/`Uint8Array` 뷰 | 청크마다 `Points` 를 덧붙임, 정밀은 새 세대가 다 오면(마지막 청크 표시) 교체 | 바뀔 때만 | **Rust·wgpu·wasm 대 three.js, 같은 구조** |
| **B1-webgpu** (`mode=b1&renderer=webgpu`) | B1 과 같음 | 없음 | B1 과 같음 | 바뀔 때만 | **같은 API(WebGPU)에서 Rust·wgpu·wasm 대 three.js** — 되면 넣음 |
| **B2** (`mode=b2`) | PLY. 미리보기는 새로 붙은 레코드만(`/data/<file>?have=`) | **Web Worker** 에서 해석(전송은 transfer, 복사 없음) | 미리보기는 덧붙임, 정밀은 교체 | 바뀔 때만 | **현실적인 as-is(SkyLens 식 증분)** |

판정은 몫으로 나눠 표로 보인다: **구조가 준 몫 = B0 → B1**, **Rust·wgpu·wasm 이 준 몫 = B1 → PointBlitz**. B1 은 three.js `WebGLRenderer`(WebGL2)라 B1 ÷ PointBlitz web(WebGPU) 에는 API(WebGPU 대 WebGL2)의 몫이 섞이므로, **같은 API 열을 꼭 함께 싣는다**: B1 ÷ PointBlitz webgl(둘 다 WebGL2), 그리고 B1-webgpu 가 되면 B1-webgpu ÷ PointBlitz web(둘 다 WebGPU). B2 는 "SkyLens 식으로 잘 만든 three.js" 대비 값으로 따로 싣는다.

기준은 "가장 강한 합리적 three.js" 여야 한다(감독). 그래서 B1 의 올리기 방식은 가정하지 않고 스모크로 고른다(아래).

### B1 세부
- 받기: PointBlitz web 과 같은 `fetch` 스트림 + 같은 `ChunkSplitter`(`web/chunks.js`), 같은 `have=<세대>.<점 수>` 규칙. 서버 쪽 변환(요청 때, 0026)도 같다.
- 올리기 — three.js 의 `InterleavedBuffer` 는 타입 배열 하나만 받으므로 청크(위치 f32×3 + 색 u8×4, 16 B)를 그대로 한 버퍼로 쓸 수 없다. 두 안을 **스모크로 비교해 빠른 쪽을 B1 으로** 정한다(진 쪽은 각주):
  - (a) 청크 본문에 `InterleavedBuffer` 두 개 — 위치 `Float32Array`(보폭 4), 색 `Uint8Array`(보폭 16, 오프셋 12, normalized). CPU 복사 없음, GPU 에 같은 바이트를 두 번 올림(점당 32 B).
  - (b) CPU 에서 위치 `Float32Array`(12 B)와 색 `Uint8Array`(4 B)로 나눠 복사한 뒤 일반 `BufferAttribute` 두 개. GPU 점당 16 B, 복사 한 번.
  - 스모크: 같은 세션에서 cold·replay ×60 각 2 회씩 번갈아, `event_latency`(preview·refined p50)·`first_frame`·`mem_cpu`·메인 스레드 멈춤을 본다. 지연이 먼저, 같으면(5 % 안) 메모리가 적은 쪽. 결과와 이유는 구현 PR 에서 이 결정에 적는다.
- 청크 원점(f64)은 `Points.position` 으로.
- 정밀 교체: 새 세대의 청크는 숨긴 그룹에 쌓고, 마지막 청크가 오면 보이게 하고 앞 세대를 `dispose` 한다(PointBlitz 와 같은 원칙, 0022).

### B1-webgpu 세부(되면 넣음)
- three.js `WebGPURenderer`. WebGPU 의 점 프리미티브는 1 px 뿐이라(three.js `PointsNodeMaterial` 문서), 2 px 점은 `Sprite` + `PointsNodeMaterial` 인스턴싱(점마다 사각형)으로 그린다 — PointBlitz 도 WebGPU 에서 점마다 사각형을 그린다. 원판은 프래그먼트에서 반지름 밖을 버린다.
- 받기·올리기·교체는 B1 과 같다(올리기는 B1 스모크에서 이긴 방식).
- 안 되면(인스턴스 속성 제약, 화면이 B0 과 다름 등) 이유를 이 결정에 적고 열을 뺀다.

### B2 세부
- 서버(`/data/<file>?have=<세대>.<점 수>`): `/chunks` 와 **같은 판단**(`chunks::plan`)으로 차분이 되면 PLY 헤더 + 앞 `점 수` 개 뒤의 레코드만 보낸다(`X-PB-Delivery: delta`, `X-PB-Skip`). 아니면 파일 전체. HTTP Range 를 두 번(헤더, 꼬리) 쓰는 것과 같은 바이트를 한 요청으로 보낸다.
- 워커 해석: 헤더를 읽어 속성 위치를 찾고, `DataView` 로 레코드를 돌며 위치(`Float32Array`)와 색(`Uint8Array`)을 만든다. 범용 `PLYLoader` 보다 빠른 전용 해석이다(B2 를 강하게).
- 미리보기는 새 레코드로 만든 `Points` 를 덧붙이고, 정밀은 새 `Points` 로 교체한다.

### 셋 공통 — 같은 그림
- 점: `PointsMaterial` 2 px, `sizeAttenuation: false`, 원판 텍스처 + `alphaTest 0.5`, 밉맵 끔(0019·0025). 해상도 1920×1080, 픽셀 비 1.
- 색: B1·B2 는 sRGB 바이트를 그대로 넘기므로 출력 색공간을 B0 과 같은 결과가 되게 맞춘다. **같은 시점 SSIM 으로 확인**한다(아래).
- 그리기 측정(orbit)은 B0 과 같은 동기화 프레임(0020).

### 해석 위치 비교(각주)
같은 스냅샷 바이트를 두고, B2 의 워커 해석 시간(`parse_end − fetch_end`, 스냅샷별·합계)과 서버의 Rust 변환 시간(같은 스냅샷의 `/chunks` 읽기·청크 만들기, 마지막 스냅샷 약 19 ms)을 표 각주에 나란히 싣는다. "Rust 가 해석을 빨리 한 몫" 이 보이게 한다.

## 확인(측정 전에 끝냄)
- 단위 시험: B2 서버 차분(헤더 + 꼬리)이 `/chunks` 차분과 같은 점 범위인지, B2 워커 해석이 Rust `ply.rs` 와 같은 값인지(스냅샷 1·3).
- 그림: 고정 시점 8곳에서 B1·B2 캡처 대 B0 캡처 SSIM ≥ 0.999(같은 three.js 그리기라 거의 같아야 함), 대 PointBlitz web 은 B0 과 같은 수준(0.9919 근처). B1-webgpu 는 대 PointBlitz web 0.99 이상이어야 넣는다.
- B1 올리기 스모크((a)·(b)), 위 B1 세부.
- 재생 동안 화면에 나온 점 수가 스냅샷 점 수와 같은지(`presented` 마크에 점 수).

## 대가
- 공개 배율이 바뀐다(측정 뒤 README·보고서 갱신). 측정 중에는 "증분 없는 기준 대비" 라는 한 줄을 넣어 둔다.
- B2 는 SkyLens 실제 코드가 아니라 "SkyLens 식 구조를 three.js 로 잘 만든 것" 이다. SkyLens 실제 해석(메인 스레드)보다 유리하다.

## 다시 볼 조건
- SkyLens 가 점군 표시 코드를 넣으면 그 코드를 B2 로 삼는다.
- three.js 가 한 버퍼에 서로 다른 타입 속성을 허용하면 B1 의 두 번 올리기를 없앤다.
