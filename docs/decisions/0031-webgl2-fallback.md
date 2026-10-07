# 0031 WebGPU 가 없을 때 WebGL2 로 대체 (P2.4)

- 상태: 승인
- 날짜: 2026-10-08
- 결정한 사람: 작업자 제안 → 감독 승인(PR #18 검토, 2026-10-08)
- 관련: 결정 0002(wgpu, WebGPU 우선·WebGL2 대체), 0028(wasm 빌드), 0029(뷰어), 0030(재생 클라이언트), SPEC §7 P2

## 맥락
WebGPU 가 없는 브라우저(또는 꺼진 경우)에서도 같은 core 로 그린다. wgpu 의 GL 백엔드(`webgl` 기능)가 WebGL2 위에서 돈다.

## 선택지와 결정
| 항목 | 선택지 | 고른 것 | 이유 |
|---|---|---|---|
| 백엔드 선택 | 어댑터 요청 실패 뒤 재시도 / **표면을 만들기 전에 `is_browser_webgpu_supported()` 로 정함** | 먼저 정함 | 캔버스는 처음 받은 컨텍스트 종류(webgpu·webgl2)를 바꿀 수 없다. 페이지 `?backend=auto|webgpu|webgl`(기본 auto) |
| 장치 한도 | 기본 한도 / **`downlevel_webgl2_defaults().using_resolution(adapter)`** | WebGL2 한도 | WebGL2 는 wgpu 기본 한도를 못 맞춘다. 우리 파이프라인(정점 버퍼·동적 uniform 오프셋·Depth32Float)은 이 한도 안이다 |
| GPU 완료 | — | `Viewer.poll()` 을 rAF 마다 부른다(막지 않는 `PollType::Poll`) | GL 백엔드는 장치를 poll 해야 완료 콜백이 돈다. 처음에는 `gpu_done` 이 끝나지 않아 페이지가 멈췄다. WebGPU 에서는 할 일이 없다 |
| 어댑터 정보 | — | WebGL 일 때는 `navigator.gpu.requestAdapter` 를 부르지 않는다. wgpu 가 GL 어댑터 이름(ANGLE 문자열)을 준다 | PR #16 검토 낮음 |
| WebGPU 끄기(시험) | `--disable-features=WebGPU` / **`--disable-features=WebGPUService`** | 후자 | 앞의 것은 이 Chrome 에서 효과가 없었다. 후자는 `navigator.gpu` 는 남지만 어댑터를 주지 않아 auto 가 WebGL2 로 간다(실제 미지원 브라우저와 같은 경로). 측정기에 `--chrome-args` 추가 |

지원하지 않아 뺀 기능은 없다.

## 결과 (이 PC, Chrome 155 헤드리스, `--disable-features=WebGPUService`)
- 백엔드 `Gl`, 캔버스 `Rgba8Unorm`, 어댑터 `ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 … Direct3D11 …)`.
- 고정 시점 8곳: WebGL2 대 native **SSIM 1.0000, 다른 픽셀 0.00 %**(8곳 모두), 대 three.js 0.9919. WebGPU·Vulkan 과 같은 화면.
- replay ×60: 14 받음·14 표시, 마지막 2,502,015 점, 오류 0. preview p50 34.4 ms, refined p50 143.3 ms. cold 181.4 ms. 청크 하나의 메인 스레드 시간 최대 1.70 ms.
- mem_cpu(렌더러) 189.8 MB(replay) / 108.3 MB(cold), GPU 프로세스 334.2 / 281.0 MB.

## 대가
- **시작할 때 메인 스레드 블록 1 회(84~90 ms)**: WebGL2 에서는 `Viewer.create`(컨텍스트·장치·셰이더 파이프라인 생성)가 동기로 돈다. WebGPU 에서는 같은 단계가 비동기라 블록이 없다.
  데이터를 받기 전의 한 번이고, 재생 중에는 블록이 없다. SPEC §7.1 판정(`main_thread_block` 0 회)은 P2.5 에서 한다. 고친다면 후보는 그리기를 Worker(OffscreenCanvas)로 옮기는 것이다.
- **wasm 크기**: `webgl` 기능으로 GL 백엔드(naga GLSL 출력 포함)가 들어가 357,375 B → 4,252,058 B 가 됐다. 로드 시간 영향은 P2.5 에서 잰다(`first_frame`, `wasm_init`).
  줄이는 후보: wasm-opt, 또는 WebGPU 전용·WebGL 포함 두 빌드를 나눠 필요한 쪽만 받기.

## 다시 볼 조건
- P2.5 에서 시작 블록이나 wasm 크기가 판정·로드 시간에 걸리면 위 후보를 결정 기록과 함께 적용한다.
