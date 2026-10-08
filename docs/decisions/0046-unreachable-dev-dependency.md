# 0046 도달할 수 없는 git 의존을 워크스페이스에서 뺀다

- 상태: **승인**(감독, PR #57 검토 2026-10-08)
- 날짜: 2026-10-08
- 결정한 사람: 감독(방향: 기본 빌드에서 도달할 수 없는 git 의존을 없애고, 캐시 없이 통과하는지 CI 로 보인다; release.yml 은 그대로), 작업자(방법)
- 관련: 결정 0014·0023(skyrecon 호환 시험), 0044(crates.io 배포)

## 맥락
- 첫 릴리스(태그 v0.1.0, 실행 37767331275)가 clippy 에서 멈췄다: `pointblitz-io` 의 dev-dependency `skyrecon-core`(git `AH100-1/skyrecon`, 고정 리비전)를 받지 못했다. 그 저장소는 2026-10-08 익명·토큰 모두 404 다(비공개 전환 또는 이름 변경으로 본다). **아무것도 배포되지 않았다**(test·publish 단계 건너뜀, crates.io 에 여섯 크레이트 모두 없음).
- 평소 CI 는 rust-cache 에 남은 사본 덕에 지나갔다 — 캐시가 비면 같은 이유로 깨지는 잠재 결함이었다.

## 결정
| 항목 | 고른 것 | 이유 |
|---|---|---|
| 호환 시험 | `crates/io/tests/skyrecon_roundtrip.rs` 를 **워크스페이스 밖 시험 크레이트 `tools/skyrecon-compat`** 로 옮긴다(워크스페이스 `exclude`, `publish = false`). skyrecon 에 접근할 수 있는 곳에서 `cargo test --manifest-path tools/skyrecon-compat/Cargo.toml` | 시험은 버리지 않는다(skyrecon 이 쓴 PLY 를 우리가 읽는지 — 의미 있는 시험). 기능 플래그는 dev-dependency 를 선택으로 만들 수 없어 쓰지 않았다 |
| 워크스페이스 | `pointblitz-io` 의 dev-dependency 와 패키지 `exclude` 를 지운다. Cargo.lock 에 skyrecon 이 남지 않는다 | 기본 빌드·배포가 공개 저장소만으로 된다 |
| CI 확인 | ci.yml(ubuntu)에 **빈 CARGO_HOME 으로 `cargo fetch --locked`** — 캐시 없이 모든 의존을 받을 수 있는지 | 릴리스 작업은 캐시가 없다. 캐시가 가린 결함을 PR 에서 잡는다 |
| release.yml | 그대로(캐시를 붙여 가리지 않는다) | 감독 방향 |
| 태그 | 고친 커밋이 main 에 들어가면 원격 태그 v0.1.0 을 지우고 그 커밋에 다시 단다(감독 허용, 소유자에게 알린 뒤). 커밋 이력은 고쳐 쓰지 않는다 | 배포된 것이 없어 0.1.0 은 crates.io 에 비어 있다 |

## 확인
- 옮긴 시험: 이 PC(캐시에 skyrecon 사본 있음)에서 `--offline` 으로 3 개 통과.
- 빈 CARGO_HOME 으로 `cargo fetch --locked` 통과(이 PC).

## 다시 볼 조건
- skyrecon 저장소에 다시 접근할 수 있게 되면(공개, 또는 CI 에 읽기 권한) 호환 시험을 CI 에 돌린다.
