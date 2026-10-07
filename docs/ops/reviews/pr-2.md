# PR #2 검토 노트 — P0.1 워크스페이스·툴체인·CI

- 검토일: 2026-10-07
- 대상 커밋: `8293899`
- 결과: **통과**
- 재현 환경: 이 PC(Windows 11, RTX 4070 12 GB), rustc 1.99.0 (b940084d7 2026-09-28), MSVC

## 재현한 것

| 명령 | 결과 |
|---|---|
| `cargo fmt --all --check` | 통과 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 통과 |
| `cargo build --workspace --all-targets` | 통과 |
| `cargo test --workspace` | 통과(9개 시험 대상, 시험 0개 — 빈 크레이트) |
| `cargo check --target wasm32-unknown-unknown -p pointblitz-io -p pointblitz-core -p pointblitz-web` | 통과 |
| `cargo tree --workspace -e normal` | 외부 크레이트 0. 의존 방향 core→io, native/web/server→core, bench→io — SPEC §3.1 과 일치 |
| GitHub Actions run 37622136402 | head `8293899` 에서 ubuntu·windows 모두 success |

완료 기준("`cargo build --workspace`·`cargo test --workspace` 가 CI 에서 통과")을 충족한다.

## 체크리스트

| # | 결과 |
|---|---|
| 1 | 통과 — 0016 에 항목별 선택지·이유·대가·다시 볼 조건이 있다. 감독이 승인으로 바꿨다 |
| 2 | 통과 — SPEC 변경 없음 |
| 3 | 통과 — 위 표 |
| 4 | 해당 없음(성능 수치 없음). 0001 의 기존 수치는 출처·조건이 붙었고 "참고값" 으로 한정됐다 |
| 5 | 해당 없음(그리기 없음) |
| 6 | 통과 — 위 `cargo tree` |
| 7 | 통과 — 외부 크레이트 없음. CI 액션 `actions/checkout`, `Swatinem/rust-cache` 는 MIT |
| 8 | 통과 — 가장 큰 추적 파일 16 KB, 비밀정보 없음 |
| 9 | 통과 — 커밋·코드·문서에 생성 도구 흔적 없음 |
| 10 | 통과 — 공개 문서 주장 변경 없음 |

PR #1 중간 항목 중 0001 출처(링크한 skylens-renderer-lab 은 공개 리포임을 확인), 0007·0011·0015 형식은 처리됐다. P0.4 원형 점은 P0.4 에서 한다.

## 기록만 (중간·낮음)

| 수준 | 내용 |
|---|---|
| 낮음 | CI 액션이 태그(`@v4`, `@v2`)로 고정돼 있다. 공개 전에 커밋 SHA 고정을 검토한다. |
| 낮음 | `crates/io/Cargo.toml` 의 `[dependencies]` 아래 빈 줄 두 개. |
| 낮음 | 이 PR 에 PR #1 재검토 노트 커밋(`627bdd2`)이 함께 들어왔다. PR #1 병합 전에 docs/foundation 에 올린 감독 커밋이 병합에 포함되지 않았던 것을 이 PR 이 채운다 — 내용 문제 없음. |
