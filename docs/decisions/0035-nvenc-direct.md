# 0035 인코더 경로: NVENC 직접 호출(드라이버 DLL 을 실행 때 불러옴) (P3.1)

- 상태: 제안
- 날짜: 2026-10-08
- 결정한 사람: 작업자(제안) → 감독 검토
- 관련: 결정 0010(NVENC H.264 + WebSocket + WebCodecs), 0015(라이선스), 0016(워크스페이스 `unsafe_code = "deny"`), SPEC §8.2, PR #22·#23 검토

## 맥락
서버가 헤드리스로 그린 프레임(1920×1080 RGBA8)을 NVENC 로 H.264 인코딩해야 한다. wgpu 에는 인코더가 없다. 이 PC 에는 NVIDIA 드라이버의 `nvEncodeAPI64.dll`·`nvcuda.dll` 이 있고 ffmpeg 는 없다.
파일 다운로드는 소유자가 채팅에서 직접 허락해야 하므로(작업자 운영 규칙), 다운로드가 필요 없는 길을 먼저 봤다.

## 선택지와 라이선스
| 선택지 | 빌드에 필요한 것 | 배포물에 들어가는 것 | 라이선스 | 판단 |
|---|---|---|---|---|
| A. 외부 ffmpeg 프로세스(`h264_nvenc`), 파이프로 프레임 전달 | ffmpeg 실행 파일 **다운로드** | 넣지 않음(측정 도구로만, 서버 운영자가 따로 설치) | ffmpeg 는 빌드에 따라 LGPL 또는 GPL. Windows 배포 빌드는 GPL 이 흔하다. **외부 프로세스로만 부르고 배포물에 넣지 않으면** PointBlitz 코드의 라이선스에 영향이 없다 | 다운로드·별도 설치가 필요하고, 파이프 복사와 ffmpeg 내부 버퍼만큼 지연이 붙는다 |
| B. ffmpeg 라이브러리 링크(`ffmpeg-next` 등) | ffmpeg 개발 라이브러리 **다운로드** | 링크된 ffmpeg 가 배포물에 포함 | LGPL 빌드면 동적 링크 조건, **GPL 빌드면 배포물이 GPL 이 되어 SPEC §8.2 위반** | 쓰지 않는다 |
| C. crates.io `nvidia-video-codec-sdk` 0.4.0 그대로 사용 | Windows 에서 build.rs 가 `nvEncodeAPI.lib`(**Video Codec SDK 다운로드, NVIDIA 계정·사용권 동의**)와 CUDA toolkit 을 찾는다(`cudarc` 0.16 의 `cuda-version-from-build-system`) | 크레이트(MIT) + cudarc(MIT OR Apache-2.0) | 크레이트·cudarc 는 허용형. SDK 다운로드·동의가 필요 | 빌드 환경이 무거워지고 동의 절차가 필요 |
| **D. NVENC 직접 호출: 바인딩만 가져오고 DLL 은 실행 때 불러옴** | **없음**(crates.io 의 `libloading` 만) | 가져온 바인딩 3 파일(MIT) | 바인딩: `nvidia-video-codec-sdk` 의 bindgen 생성물(MIT, Viliam Vadocz). 원본 헤더 `nvEncodeAPI.h` 는 NVIDIA 가 **이 헤더에 한해 MIT** 로 배포("This copyright notice applies to this header file only"). `libloading` 은 ISC. 드라이버 DLL 은 배포하지 않는다(사용자 PC 의 드라이버) | **고름** |

## 결정
D. `crates/server/src/nvenc/`:
- `sys/` — `nvidia-video-codec-sdk` 0.4.0 의 `windows_sys/nvEncodeAPI.rs`·`guid.rs`·`version.rs` 를 가져옴. 링크 시점 import(`extern "C"` 블록)를 지우고 경로 한 줄만 고침. 두 MIT 고지문 전문을 `nvenc/NOTICE.md` 에 둠(SPEC §8.2 "차용 코드는 출처와 라이선스"). 생성 코드라 이 모듈만 경고를 끔.
- `mod.rs` — `nvcuda.dll` 에서 `cuInit`·`cuDeviceGet`·`cuCtxCreate_v2` 셋만 불러 CUDA 컨텍스트를 만들고, `nvEncodeAPI64.dll` 의 `NvEncodeAPICreateInstance` 로 함수 표를 얻는다. NVENC 가 입력 버퍼를 직접 만든다(`nvEncCreateInputBuffer`) — 프레임을 잠근 버퍼에 복사 → 인코딩 → 비트스트림을 잠가 복사.
- **`unsafe` 범위**: `nvenc` 모듈에만 `#![allow(unsafe_code)]`(FFI 와 원시 포인터 복사). 워크스페이스 기본 `deny` 는 그대로(결정 0016). 나머지 서버 코드는 안전한 `Encoder::new`·`encode` 만 쓴다.
- Windows 전용(`cfg(windows)`). 다른 OS 에서는 서버가 "Windows only" 를 알리고 끝난다(Linux 의 `libnvidia-encode.so` 경로는 V100 서버에서 필요할 때 같은 방식으로 추가 — 다시 볼 조건).
- 인코딩 설정: H.264, 프리셋 P1 + `ULTRA_LOW_LATENCY` 조정(B 프레임 없음), 첫 프레임만 IDR(GOP 무한, IDR 에 SPS/PPS 반복), **상수 QP**. 대역폭이 제약이 아니므로(INTENT 원칙 2) 화질로 QP 를 고른다 → **기본 QP 18**.
- 입력 형식 `NV_ENC_BUFFER_FORMAT_ABGR` = 메모리에서 R,G,B,A — core 캡처 배치와 같아 변환이 없다.

API 버전: 바인딩은 NVENC API 12.1, 이 PC 드라이버(591.86)는 13.0 까지 지원(`NvEncodeAPIGetMaxSupportedVersion`), CUDA 드라이버 13.1.

## 결과 (`pointblitz-server encode-test`, RTX 4070, 1920×1080, 60 fps 가정)
두 흐름: views(고정 시점 8곳에 30 프레임씩 머무름, 각 마지막 프레임 표본), orbit(개관 시점 둘레 1.5°/프레임 240 프레임, 30 프레임마다 표본).
디코딩은 **Chrome WebCodecs**(실제 클라이언트와 같은 디코더, `bench/web/decode.mjs`)로 했다 — `avc1.64002a`(High, level 4.2), 240/240 프레임.

| QP | views SSIM 대 native 캡처(평균, 최저) | orbit SSIM 대 인코딩 전(진단) | orbit 비트레이트 | views 비트레이트 | 인코딩 p50 / p95 / p99 (orbit) | 입력 복사 p50 |
|---:|---|---:|---:|---:|---|---:|
| 18 | **0.9968**, 0.9934(close_dense) | 0.9953 | 74.9 Mbps | 6.2 Mbps | 2.80 / 2.90 / 2.96 ms | 0.61 ms |
| 23 | 0.9943, 0.9878 | 0.9901 | 54.1 Mbps | 4.3 Mbps | 2.58 / 2.67 / 2.75 ms | 0.62 ms |
| 28 | 0.9881, 0.9731 | 0.9794 | 36.1 Mbps | 3.2 Mbps | 2.40 / 2.50 / 2.60 ms | 0.62 ms |

- 화질 목표(SSIM ≥ 0.95, native 캡처 대비, SPEC §7.1)는 세 QP 모두 넘는다. views 의 "인코딩 전 대비" SSIM 은 native 대비와 같은 값이다(서버 렌더 = native 렌더, P2.2 의 SSIM 1.0000 과 같은 결과).
- **"픽셀이 다른 비율"이 12~19 %(close_dense 91 %)** 로 크다: 압축이 완전한 검정 배경을 검정에 가까운 값으로 바꾸기 때문이다. SSIM 이 검정 배경 때문에 높게 나온다는 PR #10 검토 지적과 같은 문제다 — P3.4·P4 에서 마스크 SSIM 을 함께 싣는다.
- 인코딩은 프레임당 약 2.4~2.8 ms + 입력 복사 0.6 ms 로, 60 fps(16.7 ms)의 한 프레임 안에 넉넉히 들어간다.

## 대가
- 지금은 GPU → CPU(읽기) → NVENC 입력 버퍼로 한 번 오간다(프레임당 8 MB). 렌더 텍스처를 CUDA·D3D 로 공유해 복사를 없애는 것은 P3.4 측정에서 이 복사가 의미 있게 크면 다시 본다(결정 0010 의 대가 항목).
- 가져온 바인딩은 API 12.1 에 고정된다. 새 기능이 필요하면 같은 방식으로 새 헤더에서 다시 만든다.

## 다시 볼 조건
- Linux 서버(V100)에서 돌릴 때: 같은 바인딩의 Linux 판을 가져오고 `libnvidia-encode.so`·`libcuda.so` 를 불러온다.
- 렌더→인코더 복사가 지연에서 의미 있게 크면 공유 메모리 경로.
