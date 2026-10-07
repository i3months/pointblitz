# 화질 비교 — flight-01 마지막 스냅샷 (P1.3)

같은 고정 시점 8곳(bench/viewpoints/flight-01.json)에서 PointBlitz(native, 헤드리스)와 기준 방식(three.js, 밉맵 끔 — 결정 0025)을 캡처해
SSIM(SPEC §6.2: 11×11 가우시안, σ 1.5, RGB 평균)을 잰다.

- 데이터: `event_14_2053.0s_refined_6.ply`, 2,502,015 점(PointBlitz 10 청크)
- 장비: RTX 4070, driver 591.86. PointBlitz = wgpu Vulkan, three.js = Chrome ANGLE D3D11
- 해상도 1920×1080, 점 2 px 원형, 배경 검정

| 시점 | SSIM | 밝은 픽셀 three.js | 밝은 픽셀 PointBlitz | 격자 three.js | 격자 PointBlitz | 픽셀이 다른 비율 |
|---|---:|---:|---:|---:|---:|---:|
| overview_sw | 0.9909 | 15.60 % | 15.60 % | 21.01 % | 21.01 % | 8.19 % |
| north | 0.9955 | 11.63 % | 11.63 % | 16.06 % | 16.06 % | 6.62 % |
| east | 0.9953 | 11.05 % | 11.05 % | 14.84 % | 14.89 % | 6.22 % |
| south | 0.9950 | 11.40 % | 11.40 % | 14.93 % | 14.93 % | 6.08 % |
| west | 0.9939 | 13.38 % | 13.37 % | 19.05 % | 19.05 % | 7.38 % |
| top_down | 0.9828 | 17.98 % | 17.97 % | 23.05 % | 23.05 % | 10.72 % |
| low_south | 0.9977 | 7.34 % | 7.33 % | 10.29 % | 10.29 % | 4.65 % |
| close_dense | 0.9841 | 72.89 % | 72.78 % | 97.66 % | 97.66 % | 18.34 % |
| **평균** | **0.9919** | | | | | |

"픽셀이 다른 비율" 은 RGB 가 한 값이라도 다른 픽셀이다. 같은 픽셀의 평균 색 차는 north 기준 1.96(0–255). 남는 차이는 겹친 점 중 어느 것이 앞에 보이느냐
(깊이 형식: three.js 24 bit 일반 Z, PointBlitz f32 reversed-Z)와 래스터화 규칙 차이로 보인다. 위치 어긋남은 없다(±1 px 이동이 0 이동보다 나쁘다).

## 재현
```sh
cargo build --release -p pointblitz-bench
target/release/pointblitz-bench capture --ply <ply dir>/event_14_2053.0s_refined_6.ply \
  --viewpoints bench/viewpoints/flight-01.json --out target/bench/pointblitz-native
# three.js 캡처: replay 서버를 띄운 뒤 (baseline/three/README)
node baseline/three/capture.mjs --server http://127.0.0.1:<port> --out target/bench/baseline-three
target/release/pointblitz-bench compare target/bench/baseline-three target/bench/pointblitz-native \
  --viewpoints bench/viewpoints/flight-01.json
```

## browser (P2.2)

같은 시점 8곳을 브라우저(Chrome 155 헤드리스, WebGPU, `bench/web/capture.mjs` — 캔버스 스크린샷)에서 캡처했다.

| 비교 | SSIM 평균 | 픽셀이 다른 비율 |
|---|---:|---:|
| browser(WebGPU) 대 native(Vulkan) | **1.0000** | 8곳 모두 0.00 % |
| browser(WebGPU) 대 three.js | 0.9919 | native 와 같은 값(4.65~18.34 %) |

같은 `pointblitz-core` 가 브라우저 WebGPU(Bgra8Unorm 캔버스)와 native Vulkan(Rgba8Unorm 캡처)에서 픽셀 단위로 같은 화면을 낸다.
three.js 와의 차이는 구현(겹친 점의 앞뒤·래스터화)에서 오며, 실행 대상과는 상관없다.

재현:
```sh
bash web/build.sh
pointblitz-bench replay --data <ply dir> --web . --port 8700
node bench/web/capture.mjs --server http://127.0.0.1:8700 --out target/bench/pointblitz-web
pointblitz-bench compare target/bench/pointblitz-native target/bench/pointblitz-web --viewpoints bench/viewpoints/flight-01.json
```
