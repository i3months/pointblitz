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

"제안" 상태는 작업자가 제안한 것이다. 감독 검토 또는 소유자 확인 뒤 "승인" 이 된다.
