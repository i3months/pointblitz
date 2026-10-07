# 0014 PLY 파서를 직접 둔다

- 상태: 제안
- 날짜: 2026-10-07
- 결정한 사람: 작업자(제안)

## 맥락
skyrecon-core 의 `read_ply` 는 파일 경로를 받고 std::fs·rayon 에 의존한다. wasm 과 스트림(일부만 도착한 바이트)에서는 쓸 수 없다.

## 선택지
| 선택지 | 장점 | 단점 |
|---|---|---|
| **pointblitz-io 에 스트림 파서** | 바이트 슬라이스·증분 입력, wasm 동작, 속성 이름 매칭 | 코드 중복 |
| skyrecon-core 의존 | 중복 없음 | wasm 불가, 경로 기반 |
| 범용 PLY 크레이트 | 성숙 | 범용이라 느릴 수 있고 증분 입력 미지원 |

## 결정
`pointblitz-io` 에 PLY 헤더 파서 + 바이너리 레코드 → GPU 청크([0008](0008-gpu-ready-uncompressed-chunks.md)) 변환기를 둔다.
skyrecon 의 두 레이아웃과 법선 없는 15 B 레이아웃을 시험 픽스처로 고정하고, skyrecon `write_ply` 출력과 왕복 시험을 한다.

## 다시 볼 조건
- skyrecon-core 가 바이트·스트림 API 와 wasm 지원을 갖추면 의존으로 바꾼다.
