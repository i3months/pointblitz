# 0029 브라우저 뷰어 구조 (P2.2)

- 상태: 승인
- 날짜: 2026-10-08
- 결정한 사람: 작업자 제안 → 감독 승인(PR #16 검토, 2026-10-08)
- 관련: 결정 0020(측정), 0024(코어), 0027(요청 때 그리기), 0028(wasm 빌드)

## 맥락
wasm 으로 빌드한 core 를 브라우저 캔버스에 그린다. 프레임 루프·네트워크·측정 마크를 JS 와 Rust 중 어디에 둘지, GPU 완료를 어떻게 알지 정한다.

## 선택지와 결정
| 항목 | 선택지 | 고른 것 | 이유 |
|---|---|---|---|
| 경계 | 루프·네트워크까지 Rust(web-sys) / **Rust 는 `Viewer`(장면·그리기), JS 가 루프·네트워크·마크** | 후자 | 브라우저 API(rAF, fetch 스트림, EventSource, performance.now)는 JS 에서 바로 쓰는 편이 짧고, 기준 방식 페이지와 마크 코드를 맞추기 쉽다. Rust 쪽 API 는 SPEC §4 의 개념(청크 넣기·카메라·그리기)과 같아 다른 바인딩(Python)에도 그대로 옮길 수 있다 |
| `Viewer` API | — | `create(canvas)`(async), `insert(chunk)`, `set_camera(eye, target, up, fov)`, `render()`, `gpu_done()`(Promise), `points()`, `gpu_bytes()`, `info()` | |
| 그리기 시점 | 매 rAF / **바뀐 것이 있을 때만(rAF 안에서 확인)** | 바뀔 때만 | 결정 0027 과 같은 원칙. rAF 콜백은 할 일이 없으면 바로 돌아간다 |
| GPU 완료 | `device.poll`(브라우저에서는 막을 수 없음) / **`queue.on_submitted_work_done` → Promise** | 콜백 | 브라우저에서 기다리는 유일한 방법이다. 콜백은 `Send` 여야 해서 작은 `Send` 신호(`oneshot.rs`)를 거쳐 `future_to_promise` 로 넘긴다 |
| 캔버스 형식 | — | 브라우저가 주는 sRGB 아닌 형식(Chrome: `Bgra8Unorm`), `alpha_mode: Opaque` | PLY 색 바이트를 그대로(결정 0024) |
| 메모리 | — | `MemoryHints::MemoryUsage`(결정 0027 과 같게) | |
| 캡처 | 텍스처를 읽어 오기 / **Playwright 캔버스 스크린샷** | 스크린샷 | 사용자가 보는 합성 결과 그대로. 각 시점은 그리기 → `gpu_done` 뒤에 찍는다 |
| 청크 나누기 | — | JS 에서 헤더 16 바이트째 `point_count` 로 경계를 찾는다(결정 0022) | 받은 버퍼에서 복사 없이 `Uint8Array` 조각을 넘긴다(wasm 메모리로는 한 번 복사됨) |

## 결과 (이 PC, Chrome 155 헤드리스)
- 마지막 스냅샷 2,502,015 점, GPU 정점 버퍼 40,032,240 B(native 와 같음).
- 고정 시점 8곳: browser 대 native **SSIM 1.0000, 다른 픽셀 0.00 %**(8곳 모두), browser 대 three.js 0.9919(native 와 같음). [quality-flight-01.md](../bench/quality-flight-01.md).
- wasm 357,375 B, JS 66,132 B(그리기 포함, 네트워크 전).

## 대가
- `insert` 는 JS 버퍼를 wasm 메모리로 한 번 복사한다(청크당 최대 4 MB). P2.3 에서 시간을 재고, 크면 다시 본다.

## 다시 볼 조건
- P2.5 에서 메인 스레드 블록이 생기면 청크 넣기를 나누거나 워커(OffscreenCanvas)로 옮기는 것을 검토한다.
