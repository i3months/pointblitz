# 0023 skyrecon-core 를 개발 의존성으로 두고 리비전을 고정한다

- 상태: 승인
- 날짜: 2026-10-07
- 결정한 사람: 작업자 제안 → 감독 승인(PR #9 검토, 2026-10-07)

## 맥락
결정 0014 는 PLY 파서를 직접 두되 "skyrecon `write_ply` 출력과 왕복 시험을 한다" 고 했다. PR #8 검토가 이 시험이 없다고 지적했다
(특히 `XyzNormalRgb` 레이아웃의 실제 출력이 확인되지 않았다).

## 선택지
| 선택지 | 장점 | 단점 |
|---|---|---|
| **skyrecon-core 를 `[dev-dependencies]` 로, git 리비전 고정** | skyrecon 의 실제 `write_ply` 로 만든 파일을 읽어 본다. 고정이라 skyrecon 이 바뀌어도 시험이 갑자기 깨지지 않는다 | 시험 빌드에 nalgebra·rayon 등이 따라온다. CI 가 GitHub 에서 받아야 한다 |
| 손으로 만든 픽스처만 | 의존성 없음 | skyrecon 쪽 변화(타입 이름, 속성 순서)를 못 잡는다 |
| skyrecon 출력 파일을 저장소에 넣기 | 빌드 의존성 없음 | 바이너리 파일 관리, 생성 경로가 시험에 안 드러남 |

## 결정
`crates/io` 의 개발 의존성으로 `skyrecon-core`(github.com/AH100-1/skyrecon, rev `9f8a0f5e…`)를 둔다. 배포물(라이브러리 의존성)에는 들어가지 않는다.
시험 `crates/io/tests/skyrecon_roundtrip.rs` 는 `XyzRgbNormal`(27 B), `XyzNormalRgb`(27 B), 법선 없는 15 B 를 `write_ply` 로 써서
`parse_header`·`points`·`PlyStream`(13 B 조각)으로 다시 읽고 위치·색이 같음을 단언한다.

라이선스: skyrecon 은 MIT OR Apache-2.0(결정 0015 와 호환).

## 결과
3 개 시험 통과(이 PC). skyrecon 의 두 레이아웃 모두 PointBlitz 파서와 위치·색이 비트 단위로 같다.

## 대가
- skyrecon 의 PLY 형식이 바뀌면 리비전을 올리는 PR 에서 드러난다(자동으로 따라가지 않는다).

## 다시 볼 조건
- skyrecon 이 바이트·스트림 입력과 wasm 을 지원하면 결정 0014 를 다시 본다(파서를 공유할지).
