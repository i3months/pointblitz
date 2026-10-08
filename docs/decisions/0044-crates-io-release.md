# 0044 crates.io 배포와 릴리스 CI

- 상태: 제안(배포 범위·버전·방식은 소유자 결정, 2026-10-08 채팅) → 감독 검토
- 날짜: 2026-10-08
- 결정한 사람: 소유자(crates.io 에 올림, 라이브러리 + 실행 파일, 첫 버전 0.1.0, 릴리스 CI 를 만들고 토큰은 소유자가 넣음), 작업자(메타데이터·워크플로 세부)
- 관련: 결정 0001(Rust), 0003(한 코어 세 대상), P4.8(대외 공개)

## 맥락
- PointBlitz 는 Rust 워크스페이스이고, 지금까지 `publish = false`(버전 0.0.0)로 배포를 막아 두었다. 소유자가 crates.io 배포와 배포용 CI 를 정했다.
- crates.io 이름 `pointblitz`, `pointblitz-core`, `-io`, `-native`, `-server`, `-web` 은 2026-10-08 기준 모두 비어 있다.
- crates.io 에 올린 버전은 지울 수 없다(yank 만 됨). 그래서 배포 전 검사를 CI 에 넣는다.

## 결정
| 항목 | 고른 것 | 이유 |
|---|---|---|
| 올리는 크레이트 | `pointblitz-io`, `pointblitz-core`(라이브러리), `pointblitz-native`, `pointblitz-server`(`cargo install` 실행 파일), `pointblitz-web`(wasm 라이브러리), **새 대표 크레이트 `pointblitz`**(core·io 를 다시 내보냄) | 소유자 결정. 대표 이름으로 찾는 사람이 바로 쓰게 한다 |
| 올리지 않음 | `pointblitz-bench`(측정 도구) `publish = false` | 측정·저장소 안에서만 쓴다 |
| 버전 | 워크스페이스 공통 0.1.0 | 소유자 결정. 0.x 는 API 가 바뀔 수 있다는 뜻(P5 에서 바뀔 가능성) |
| 내부 의존성 | `{ path, version = "0.1.0" }` | crates.io 는 경로만 있는 의존성을 받지 않는다 |
| 메타데이터 | README(저장소 README), keywords, categories, documentation(docs.rs) | crates.io·docs.rs 표시 |
| 배포 전 검사(CI, 매 PR) | `cargo publish --workspace --exclude pointblitz-bench --dry-run` | 패키지가 깨지면 배포 전에 알 수 있게 |
| 배포(릴리스 CI) | 태그 `v*` 를 푸시하면: 태그 버전 = 워크스페이스 버전 확인 → fmt·clippy·test → `cargo publish --workspace --exclude pointblitz-bench`(의존 순서는 cargo 가 정함) | 손으로 순서를 맞추다 틀리지 않게 |
| 토큰 | crates.io API 토큰을 저장소 Secret `CARGO_REGISTRY_TOKEN` 에 **소유자가** 넣는다(범위: publish-new, publish-update; 크레이트 `pointblitz*`) | 계정 비밀 정보는 작업자가 다루지 않는다 |

## 대가
- 공개 API(타입·함수 이름)가 0.1.0 으로 굳는다 — 바꾸면 0.2.0.
- docs.rs 는 Linux 에서 문서를 빌드한다: server 의 NVENC 는 실행 중 로드라 빌드에는 영향 없다.

## 다시 볼 조건
- npm(`pointblitz-web` 의 JS 패키지)은 별도 결정(P4.8, 지금은 보류).
