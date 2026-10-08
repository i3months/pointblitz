# 결정 기록

모든 기술 선택의 이유를 남긴다. 형식과 규칙은 [PROJECT.md §4](../../PROJECT.md).

| # | 제목 | 상태 |
|---|---|---|
| [0001](0001-language-rust.md) | 구현 언어: Rust | 승인 |
| [0002](0002-gpu-api-wgpu.md) | GPU 추상화: wgpu | 승인 |
| [0003](0003-one-core-three-targets.md) | 렌더 코어 하나, 실행 위치 셋 | 승인 |
| [0004](0004-skyrecon-point-clouds-only.md) | 입력은 skyrecon 점군만 | 승인 |
| [0005](0005-baseline-definition.md) | 비교 기준(기존 방식)의 정의 | 승인 |
| [0006](0006-opaque-points-no-sort.md) | 불투명 점 + 깊이 테스트, 정렬 없음 | 승인 |
| [0007](0007-point-size-shape.md) | 점 모양·크기·색 | 승인 |
| [0008](0008-gpu-ready-uncompressed-chunks.md) | 클라이언트 형식: GPU 레이아웃 무압축 청크 | 승인 |
| [0009](0009-chunk-model-append-replace.md) | 갱신 모델: preview 추가, refined 세대 교체 | 승인 |
| [0010](0010-server-video-transport.md) | 서버 영상: NVENC + WebSocket + WebCodecs | 승인 |
| [0011](0011-hardware-cost-model.md) | 비교표는 하드웨어 비용 모델과 함께 | 승인 |
| [0012](0012-docs-and-review-process.md) | 문서는 한 리포, 감독은 로컬 별도 세션 | 승인 |
| [0013](0013-coordinate-conventions.md) | 좌표 규약 | 승인 |
| [0014](0014-ply-parser.md) | PLY 파서를 직접 둔다 | 승인 |
| [0015](0015-license.md) | 라이선스: MIT OR Apache-2.0 | 승인 |
| [0016](0016-toolchain-workspace-ci.md) | 툴체인·워크스페이스·CI | 승인 |
| [0017](0017-fixed-viewpoints.md) | 고정 시점 8곳을 정하는 규칙 | 승인 |
| [0018](0018-replay-server.md) | 시나리오 재생 서버: std HTTP + SSE | 승인 |
| [0019](0019-baseline-three-details.md) | 기준 방식(three.js) 구현 세부와 실행 환경 | 승인 |
| [0020](0020-measurement-method.md) | 측정 방법: 무엇을 어떻게 재는가 | 승인 |
| [0021](0021-measure-with-vsync-on.md) | 측정 기본은 vsync 켬(0020 vsync 행 대체) | 승인 |
| [0022](0022-chunk-format-v1.md) | 청크 형식 v1 세부 | 승인 |
| [0023](0023-skyrecon-dev-dependency.md) | skyrecon-core 개발 의존성, 리비전 고정 | 승인 |
| [0024](0024-core-render-design.md) | 렌더 코어 설계(P1.2) | 승인 |
| [0025](0025-baseline-disk-without-mipmaps.md) | 기준 방식 원판 텍스처 밉맵 끔 | 승인 |
| [0026](0026-chunk-delivery-and-native-client.md) | 청크 전달 프로토콜과 native 재생 클라이언트(P1.4) | 승인 |
| [0027](0027-native-p15-fixes.md) | P1.5 수정: 요청 때 그리기, 메모리 우선 할당, 차분 꼬리 읽기, 병렬 인코딩 | 승인 |
| [0028](0028-wasm-build.md) | wasm 빌드 경로(wasm-bindgen CLI 고정, web/build.sh) | 승인 |
| [0029](0029-web-viewer.md) | 브라우저 뷰어 구조(Rust Viewer + JS 루프, gpu_done) | 승인 |
| [0030](0030-web-replay-client.md) | 브라우저 재생 클라이언트, 측정기 공용화, 탐색 전 메모리 표본 | 승인 |
| [0031](0031-webgl2-fallback.md) | WebGPU 없을 때 WebGL2 대체 | 승인 |
| [0032](0032-mem-cpu-os-peak.md) | mem_cpu = OS 최대 commit(0020 메모리 행 대체) | 승인 |
| [0033](0033-browser-measurement.md) | 브라우저 측정 방법(화면 밖 동기화 프레임, 읽기 동기화)과 WebGPU 전용 모듈 | 승인 |
| [0034](0034-memory-hints-speed-first.md) | 메모리 할당 설정은 속도로 고른다(0027 2번 대체) | 승인 |
| [0035](0035-nvenc-direct.md) | 인코더 경로: NVENC 직접 호출(드라이버 DLL 실행 때 로드, 바인딩 MIT) | 승인 |
| [0036](0036-video-server-stream.md) | 영상 서버 구조와 전송 형식(고정 60 fps, 프레임+메타 한 메시지, 입력 번호) | 승인 |
| [0037](0037-video-browser-client.md) | 서버 영상 브라우저 클라이언트(WebCodecs), 디코더 지연 수정(VUI bitstream_restriction) | 승인 |
| [0038](0038-p34-measurement-rules.md) | P3.4 측정 정의와 유효성 규칙(GPU 기록, 오염 기준) | 승인 |
| [0039](0039-video-frame-pacing.md) | 서버 영상 프레임 고르게 하기: 클라이언트 한 프레임 버퍼(`?pacing=`) | 승인(선택지로만, 기본 immediate) |
| [0040](0040-frame-clock-screen-latency-adaptive-pacing.md) | 측정 화면 시계(C3)·화면 기준 지연·적응형 프레임 다듬기(선택지, 기본은 immediate) | 승인 |
| [0041](0041-cpu-state-c4-power-throttling.md) | 측정 CPU 상태(C4)와 전원 조절 끄기(측정 프로세스만) | 승인 |
| [0042](0042-server-tick-phase-lock.md) | 서버 영상: 쓰기 스레드 송신(기본), 서버 틱 위상 잠금(선택지 — 끔으로 확정) | 승인 |
| [0043](0043-linux-nvenc-second-machine.md) | Linux NVENC 와 두 번째 장비(V100 서버) 측정 — 비용 모델 두 장비 보정 | 승인 |

"제안" 상태는 작업자가 제안한 것이다. 감독 검토 또는 소유자 확인 뒤 "승인" 이 된다.
