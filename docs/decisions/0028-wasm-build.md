# 0028 wasm 빌드 경로 (P2.1)

- 상태: 승인
- 날짜: 2026-10-08
- 결정한 사람: 작업자 제안 → 감독 승인(PR #15 검토, 2026-10-08)
- 관련: 결정 0002(wgpu, WebGPU 우선), 0003(core 는 창·네트워크·브라우저 API 없음), 0016(툴체인), SPEC §3.1, §7 P2

## 맥락
`pointblitz-core` 를 브라우저에서 돌리려면 wasm 으로 빌드하고 JS 와 이어 붙여야 한다. 실행 시점에 Node 를 쓰지 않는다(INTENT, SPEC §1).

## 선택지와 결정
| 항목 | 선택지 | 고른 것 | 이유 |
|---|---|---|---|
| JS 연결 | **wasm-bindgen** / 손으로 쓴 extern / emscripten | wasm-bindgen | wgpu 의 WebGPU 백엔드가 wasm-bindgen·web-sys 위에 있다. 다른 길은 wgpu 를 못 쓴다 |
| 빌드 도구 | wasm-pack / trunk / **cargo + wasm-bindgen CLI 를 부르는 스크립트(`web/build.sh`)** | 스크립트 | 두 명령이면 된다. wasm-pack 은 npm 패키지용 메타데이터를 만드는데 JS 래퍼·npm 은 P4 에서 정한다(PR #14 검토). trunk 는 개발 서버·자산 처리까지 하지만 우리는 재생 서버로 서빙한다 |
| 버전 고정 | — | crate `wasm-bindgen = "=0.2.129"`, CLI 0.2.129(`build.sh` 가 버전이 다르면 멈춤) | CLI 와 crate 버전이 다르면 생성된 JS 가 맞지 않는다. `=` 로 고정해 `cargo update` 가 몰래 올리지 못하게 한다 |
| 출력 | `--target bundler` / **`--target web`** | web | 번들러 없이 `<script type="module">` 에서 `import init` 으로 바로 연다 |
| 산출물 | 저장소에 넣기 / **`web/pkg/` 를 무시하고 빌드** | 빌드 | 생성물이다. CI 의 `wasm` job 이 빌드하고 크기를 출력한다 |
| 서빙 | — | 재생 서버 `/static/web/index.html`(`--web .`) | 기준 방식과 같은 서버·같은 출처. `.wasm` 은 `application/wasm` 으로 나간다(결정 0018) |
| WebGPU 사용 | — | `Backends::BROWSER_WEBGPU`, 어댑터는 캔버스 표면과 호환되게 요청 | WebGL2 대체(`webgl` 기능)는 P2.4 에서 |
| 어댑터 이름 | — | wgpu 의 `AdapterInfo` 는 WebGPU 에서 이름이 비어 있다. 페이지가 `GPUAdapter.info`(vendor·architecture)를 함께 기록한다 | 측정 기록에 어떤 GPU 인지 남기기 위해 |
| 측정·확인 도구 | — | `bench/web/check.mjs`: 기준 방식과 같은 Chrome·플래그(`baseline/three/args.mjs`), Playwright 도 기준 방식의 설치를 빌려 쓴다 | Node 설치를 하나로 유지 |

## 결과 (이 PC)
- `bash web/build.sh` → `pointblitz_web_bg.wasm` 81,724 B, `pointblitz_web.js` 21,242 B(P2.1 은 어댑터 확인만 들어 있다. 그리기·네트워크가 들어가면 P2.5 에서 다시 잰다).
- Chrome 155 헤드리스(측정과 같은 플래그, 추가 플래그 없음)에서 WebGPU 어댑터를 얻었다: `backend: BrowserWebGpu`, `GPUAdapter.info` = `nvidia / lovelace`(RTX 4070).
- wasm 로드(`init`) 2.7 ms, 어댑터까지 약 80 ms(같은 실행의 마크).

## 대가
- Chrome 은 Windows 에서 `powerPreference` 를 무시한다(콘솔 경고, crbug 369219127). GPU 가 하나인 이 PC 에서는 상관없지만, 노트북처럼 GPU 가 둘이면 내장 GPU 가 잡힐 수 있다. 측정 기록의 `GPUAdapter.info` 로 확인한다.
- CI 가 wasm-bindgen CLI 를 `cargo install` 로 빌드하므로 몇 분이 더 든다(캐시로 줄어든다).

## 다시 볼 조건
- wasm-bindgen 을 올릴 때는 crate·CLI·CI 세 곳을 함께 바꾼다.
- JS 래퍼·npm 패키지를 정할 때(P4) wasm-pack 을 다시 본다.
