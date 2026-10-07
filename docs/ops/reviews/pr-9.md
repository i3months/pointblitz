# PR #9 검토 노트 — P1.2 pointblitz-core + PR #8 후속

- 검토일: 2026-10-07
- 대상 커밋: `f80570d`
- 결과: **통과**
- 재현 환경: 이 PC(RTX 4070, Vulkan 어댑터), rustc 1.99.0, `flight-01`

## 재현한 것

| 확인 | 결과 |
|---|---|
| fmt·clippy(-D warnings)·wasm32 check | 통과 |
| `cargo test --workspace` | 통과(GPU 시험 포함 43) |
| GPU 시험 `cargo test -p pointblitz-core --test render -- --nocapture --test-threads=1` | 어댑터 `NVIDIA GeForce RTX 4070 (Vulkan)`, 4 통과 |
| CI(run 37638522378) | ubuntu·windows 통과(GPU 시험은 skipped) |
| `flight01`(왕복 검사 포함) | 14개 통과, 최대 왕복 오차 1.91e-6 m |
| `skyrecon_roundtrip` | 3 레이아웃 통과. skyrecon 리포는 공개(GitHub 이 LICENSE 파일을 찾지 못하지만 `Cargo.toml` 은 `MIT OR Apache-2.0`) |
| 의존성 라이선스 | `cargo metadata` 로 전체 트리 확인: MIT·Apache-2.0·Zlib·BSD-2·ISC·Unicode-3.0 계열, `r-efi` 는 `MIT OR Apache-2.0 OR LGPL-2.1-or-later`(MIT 선택 가능). GPL 전용 없음 |
| 카메라 규약 | `look_dir_rh`·reversed-Z 행렬 검토. 단위 시험 "북쪽을 볼 때 동쪽이 오른쪽" 이 있어, GPU 시험이 같은 행렬(CPU 투영)을 기준으로 삼아도 축 뒤집힘은 잡힌다 |
| 점 크기 | 셰이더 `clip.xy += corner·point_px/viewport·w` → 화면에서 정확히 point_px 폭. 원 판정 반지름 1(코너 단위) = 지름 2 px |

### 감독 독립 검사 — 실데이터 렌더(커밋 안 함)

event_14(2,502,015 점, 10 청크)를 새 코어로 고정 시점 8곳, 1920×1080 으로 그렸다. `scene.gpu_bytes` 는 40,032,240 B 다.
점이 찍힌 픽셀 비율을 P0.4 three.js 캡처와 비교했다.

| 시점 | PointBlitz(원 discard) | PointBlitz(discard 끔) | three.js(P0.4) |
|---|---:|---:|---:|
| overview_sw | 15.60 % | **15.81 %** | 15.81 % |
| north | 11.63 % | — | 11.81 % |
| east | 11.05 % | — | 11.16 % |
| south | 11.40 % | — | 11.51 % |
| west | 13.37 % | — | 13.61 % |
| top_down | 17.97 % | — | 18.16 % |
| low_south | 7.33 % | — | 7.44 % |
| close_dense | 72.78 % | **78.07 %** | 78.06 % |

- 8곳 모두 장면이 제자리에 그려진다(축·시점·청크 오프셋 정상).
- **three.js 기준 방식은 2 px 에서 사실상 2×2 사각형을 그린다.** 원판 텍스처가 2 px 크기에서 밉맵으로 흐려져 모서리 픽셀도 alphaTest 0.5 를 넘는다.
  PointBlitz 셰이더의 원 판정을 끄면 덮인 픽셀이 three.js 와 소수 둘째 자리까지 같다.

## 체크리스트

| # | 결과 |
|---|---|
| 1 | 통과 — 0023(skyrecon dev 의존)·0024(렌더 코어 설계). 감독이 승인으로 바꿨다. 0022 정정은 정정 표시와 함께 문구만 고쳤다 — 승인된 기록의 사실 정정이라 허용 |
| 2 | 통과 |
| 3 | 통과 — 위 재현 |
| 4 | 해당 없음(성능 수치 없음) |
| 5 | 통과(P1.2 기준: 합성 장면 픽셀 시험). 실데이터·기준 대비 SSIM 은 P1.3 — 아래 중간 1 |
| 6 | 통과 — core 의존: wgpu·bytemuck·glam·io. 창·네트워크·브라우저 API 없음, 자체 `block_on` 으로 런타임 무관 |
| 7 | 통과 — 위 라이선스 확인 |
| 8 | 통과 — Cargo.lock 만 큼(1,511 줄), 데이터 없음 |
| 9 | 통과 |
| 10 | 통과(중간 1 의 0019 문구는 P1.3 에서 정정) |

## 기록만 (중간·낮음)

| 수준 | 내용 |
|---|---|
| 중간 | **기준 방식과 PointBlitz 의 점 모양이 다르다(위 표).** 결정 0007 은 "기준 방식과 같게 맞춘다" 이고, 0019 는 "원판 텍스처 + alphaTest 로 원형 2 px" 라고 적었지만 실제 기준 방식은 2×2 사각형에 가깝다. 이대로면 PointBlitz 가 픽셀을 덜 칠해 그리기 비용 비교가 PointBlitz 쪽으로 기울고 SSIM 도 낮아진다. **P1.3(SSIM) 전에** 새 결정 기록으로 둘 중 하나를 정한다. ① PointBlitz 가 기준 방식의 실제 결과(2 px 에서 2×2)에 맞춘다 — P0.6 측정이 그대로 유효하다. ② 기준 방식을 진짜 원으로 고친다(텍스처 밉맵 끔 등) — P0.6 을 다시 잰다. 감독 의견은 ①이다(2 px 원은 정의가 모호하고, 이미 잰 기준을 바꾸지 않는다). 0019 의 "원형" 문구도 실측대로 정정한다. |
| 낮음 | GPU 시험의 "켜진 픽셀 1~4 개" 는 느슨하다. 원 판정 여부에 따라 결과가 갈리므로 위 결정 뒤 정확한 개수로 단언한다. |
| 낮음 | skyrecon 리포에 LICENSE 파일이 없다(Cargo.toml 표기만 있다). dev 의존이라 배포물 영향은 없지만 0023 에 적는다. |
