# 0033 브라우저 측정 방법과 WebGPU 전용 모듈 (P2.5)

- 상태: 제안
- 날짜: 2026-10-08
- 결정한 사람: 작업자(제안) → 감독 검토
- 관련: 결정 0020(동기화 프레임), 0029(뷰어), 0030(재생 클라이언트), 0031(WebGL2), 0032(mem_cpu), PR #18 검토(시작 블록 원인, wasm 크기 A/B)

## 맥락
P2.5 에서 브라우저 구현을 기준 방식·native 와 같은 정의로 잰다. 처음 만든 `orbit`(동기화 프레임)에서 두 문제가 드러났다.

1. **메모리 폭증**: 캔버스에 960 프레임을 한 애니메이션 프레임 안에서 그리자 렌더러 메모리가 WebGPU 1.8 GB, WebGL2 3.1 GB 가 됐다(cold 는 110~150 MB).
   화면 밖 텍스처로 그려도 WebGPU 는 1.7 GB 였다. 하나씩 끄며 좁혀 보니 **"실제 그리기 + 매 프레임 `onSubmittedWorkDone`"** 조합에서만 커졌다
   (그리기만: 132 MB, 완료 대기만: 143 MB, 둘 다: 1,666 MB — 프레임 수에 비례, 약 1.9 MB/프레임). wgpu 의 감싼 코드는 약속 하나·콜백 하나뿐이라, 메모리를 붙잡는 쪽은 Chrome 으로 본다.
2. **WebGL2 의 GPU 완료 대기**: WebGL 펜스 상태는 애니메이션 프레임 경계에서만 바뀌어 동기화 프레임이 약 16.7 ms 로 잡혔다.

## 결정
| 항목 | 고른 것 | 이유 |
|---|---|---|
| 동기화 프레임 대상 | **화면 밖 텍스처**(캔버스 크기·형식, `Viewer.render_offscreen`) | native `orbit` 과 같다. 캔버스를 건드리지 않는다 |
| WebGPU 완료 대기 | **첫 픽셀을 256 B 버퍼로 복사하고 `mapAsync`** (`Viewer.readback_done`) | GPU 가 그 프레임을 끝내야 매핑된다 — 기준 방식의 1 픽셀 `readPixels` 에 해당. 메모리 폭증이 없다. 결과가 비동기로 돌아오므로 `frame_time_total` 은 위쪽 한계(상한)다 |
| WebGL2 완료 대기 | **같은 컨텍스트에서 1 픽셀 `readPixels`**(기본 프레임버퍼, wgpu 의 바인딩을 저장·복원) | 기준 방식과 똑같은 방법 |
| 평소 프레임 | `gpu_done` 은 기다리는 쪽이 있을 때만(`presented` 표식·`setView`) | 매 프레임 부르면 위 1 의 메모리 문제가 실제 사용에서도 생긴다 |
| 시작 마크 | `wasm_init_start/end`(모듈 받기·컴파일·인스턴스화), `viewer_create_start/end`(컨텍스트·장치·파이프라인) | PR #18 검토: WebGL2 시작 블록의 원인을 나눈다 |
| wasm 모듈 | `webgl` 기능을 crate 기능으로(기본 켬). `bash web/build.sh webgpu` → `web/pkg-webgpu`(WebGL2 없음), 페이지 `?pkg=webgpu` | WebGPU 사용자도 4.25 MB 를 받는 문제(PR #18 검토)를 같은 세션 A/B 로 재기 위해. WebGPU 전용 모듈은 WebGPU 가 없으면 "built without WebGL2" 오류를 낸다 |
| `navigator.gpu` 가 아예 없을 때 | 기본 모듈의 `auto` 가 WebGL2 로 간다(초기화 스크립트로 `navigator.gpu` 를 지우고 확인) | PR #18 검토 낮음 |
| 측정 구현 이름 | `web-webgpu`(기본 모듈), `web-webgpu-only`(WebGPU 전용 모듈), `web-webgl2`(WebGPU 끔) | `run.mjs --label` |

## 결과 ([docs/bench/browser.md](../bench/browser.md))
- orbit 렌더러 `mem_cpu`: 1.7~3.1 GB → WebGPU 131 MB, WebGPU 전용 110 MB, WebGL2 162 MB(확인 실행).
- 같은 세션 suite(orbit 5 회): `frame_time_total` p50 / p95 / p99 — WebGPU 5.10 / 7.00 / 7.54, WebGPU 전용 5.00 / 7.00 / 7.40, WebGL2 3.50 / 13.50 / 14.04 ms(native 1.55 / 1.83 / 1.99, three.js 2.7 / 3.6 / 6.0).
- 시작 마크: WebGL2 의 긴 작업 75~80 ms 는 `Viewer.create`(71~76 ms, GL 에서 동기)에 걸친다. WebGPU 는 같은 단계가 비동기라 블록이 없다.
- 모듈: wasm 4,253,077 B 대 385,107 B, `wasm_init` 18.3~25.1 ms 대 5.5~9.7 ms(루프백), `first_frame` 은 흔들림 안.
- `navigator.gpu` 가 없으면 기본 모듈이 WebGL2 로 간다(확인).

## 대가
- WebGPU 의 `frame_time_total` 은 동기화 방법 때문에 native·기준 방식보다 불리하게(크게) 나온다. 표에 방법을 함께 적는다.

## 다시 볼 조건
- Chrome 이 `onSubmittedWorkDone` 메모리 문제를 고치면 다시 확인한다.
- WebGPU 타임스탬프 쿼리를 기본으로 쓸 수 있게 되면 GPU 시간을 따로 잰다.
