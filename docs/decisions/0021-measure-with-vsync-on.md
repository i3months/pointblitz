# 0021 측정 기본은 vsync 켬

- 상태: 제안
- 날짜: 2026-10-07
- 결정한 사람: 작업자(제안) → 감독 검토
- 대체: 결정 0020 의 "vsync" 행(끔 기본)

## 맥락
결정 0020 은 rAF 간격이 화면 주사율(16.7 ms)에 묶여 모든 구현이 같아 보이는 것을 피하려고 vsync 를 끈 채(`--disable-gpu-vsync
--disable-frame-rate-limit`) 재기로 했다. P0.6 첫 측정에서 이 설정이 replay 수치를 크게 오염시키는 것이 드러났다.

## 근거 — vsync 끈 첫 측정 (2026-10-07, RTX 4070, Chrome 155, commit 4f4300c)

렌더 루프가 주사율 제한 없이 250만 점을 계속 GPU 에 제출해 GPU 큐가 쌓였고, 교체 프레임의 동기화(1 px 읽기, 결정 0020)가 쌓인 프레임을 모두 기다렸다.

| 시나리오 | 실행 | 교체 프레임 GPU 대기 최대 | 메인 스레드 블록 | frame_interval_max | 이벤트 반영 최대 |
|---|---:|---:|---|---:|---:|
| cold | 5 | 12.8 ms | 3 회, 837~887 ms | 749~785 ms | 979 ms |
| replay ×60 | 3 | 8,422 ms | 24~26 회, 30.4~53.5 s | 9.7~17.7 s | 20.6 s |
| replay ×1 | 1 | 14,670 ms | 501 회, 1,572 s | 27.4 s | 27.6 s |

cold 는 스냅샷이 하나라 큐가 짧아 영향이 작았다. replay 는 실제 앱에서 일어나지 않는 지연이다(실제 앱은 화면 주사율로 그린다).

vsync 를 켠 같은 시나리오(replay ×60 한 번, 확인용): 교체 프레임 GPU 대기 1.8~28.9 ms(점 수에 비례 — 업로드 비용),
이벤트 반영 최대 0.92 s, orbit frame_time_total p50 2.8 ms(vsync 끔과 같음).

재현: `node run.mjs --server … --scenario replay --speed 60 --no-vsync`(끔) / 옵션 없이(켬). 원자료는 `target/`(저장소에 넣지 않음)이므로 위 표가 기록이다.

## 선택지
| 선택지 | 장점 | 단점 |
|---|---|---|
| vsync 끔 유지 | rAF 간격이 주사율에 묶이지 않음 | GPU 큐 폭주로 replay 지연·블록이 측정 인공물이 됨 |
| **vsync 켬(기본)** | 실제 앱과 같은 조건, 교체·지연 수치가 실제 체감과 같음 | `frame_interval` 하한이 16.7 ms(60 Hz)로 묶임 |
| 끔 + 프레임 사이 GPU 대기 | 큐 폭주 없음 | 모든 프레임이 동기화되어 실제 앱과 다름 |

## 결정
- 모든 구현(three.js, PointBlitz native·browser·server)을 **vsync 켬** 으로 잰다. 무제한이 필요하면 `--no-vsync`(native 는 present mode `Immediate`)로 따로 재고 표에 조건을 적는다.
- 그리기 비용(`frame_time_total/cpu/gpu_estimate`)은 동기화 프레임이라 vsync 와 무관하게 결정 0020 그대로 잰다.

## frame_interval 의 의미 (vsync 켬)
- 하한은 화면 주사율 주기(60 Hz → 16.7 ms)다. p50 이 16.7 ms 면 "주사율을 따라간다", 그보다 크면 "프레임을 놓친다".
- 사용자 체감 기준으로 본다: `frame_interval_p95/p99` 가 16.7 ms 를 넘는 비율과 `frame_interval_max`(가장 긴 멈춤)가 끊김을 나타낸다.
- 그리기 성능 비교는 `frame_time_*` 로 하고, `frame_interval_*` 은 "끊기는가" 비교로만 쓴다.
- PointBlitz 도 같은 주사율·같은 vsync 조건으로 잰다(native: `PresentMode::Fifo`, browser: 기본 rAF).

## 대가
- 주사율이 다른 화면(120 Hz 등)에서는 하한이 달라진다. 측정 기기의 주사율을 `device` 에 기록한다(헤드리스는 60 Hz).

## 다시 볼 조건
- 고주사율 화면 비교가 필요해지면 주사율별로 따로 잰다.
