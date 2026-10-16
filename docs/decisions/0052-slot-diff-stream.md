# 0052 증분 구조 — cumulus3d 0.4.0 의 점 슬롯 차분 스트림(v4)을 그대로 받는다 (0.3.0)

- 상태: 승인(소유자 지시 2026-10-16, 감독 작성)
- 날짜: 2026-10-16
- 결정한 사람: 소유자(방향: "cumulus3d 최신화를 기준으로 작업, 증분 구조"), 감독(형식·구조·측정 계획)
- 관련: 0008(GPU 레이아웃 청크), 0022·0051(청크 v1·v2), 0050(알림 때 변환), SPEC §2·§3.3·§4·§6.2

## 맥락
- cumulus3d 0.4.0(커밋 `ef0803e`)의 **stable display**(`--stable-display`): 초벌이 없고, 구역은 한 번 보이면 ENU(첫 GPS 기준)에서 움직이지 않는다. 예외로 내부값이 안정되기 전(최대 구역 3)에는 앞 구역을 옮기거나 다시 조밀화해 `replace` 를 낸다.
- 같은 모드에서만 **live diff stream v4**(`<out>/live/stream/`)를 쓴다(`crates/cli/src/blocks.rs`). 사건(정밀·replace·final)마다 화면 점군(1/6 추출, 겹침 제거)을 직전과 비교해 바뀐 것만 낸다. 점마다 고정 `u32` 슬롯 id 가 있어 "뷰어는 GPU 버퍼 id 칸에 바로 쓴다".
  - `packs/e_NNNN.bin`(머리 없음, LE): add `n_add`×20 B(`u32 id | f32 x y z | u8 r g b | u8 zone`), move `n_move`×20 B(`u32 id | f32 x y z | u8 r g b | u8 0`), remove `n_remove`×4 B(`u32 id`). 같은 사건에서 풀린 id 는 그 사건에서 다시 주지 않는다. 구역이 바뀐 점은 remove + 새 id add.
  - `manifest.json`: `version 4`, `frame "enu"`, `origin_gps`, `capacity`(지금까지 쓴 최대 id + 1), `events[] {seq, t, zone, kind, points, pack, n_add, n_move, n_remove, bytes}`.
- 실측(V100, theater30 원본 720 장, `--gpu --stable-display --seed 7`, 2026-10-16): 사건 14 개(정밀 7, replace 6, final 1), 마지막 555 s, 최종 화면 점 2,881,929, 차분 합계 125.4 MB(같은 점을 통째로 한 번 = 57.6 MB). replace 사건은 move 가 16만–54만 개. 사건마다 `points = 직전 + add − remove`, `bytes = 20(add+move) + 4·remove`, 팩 파일 크기를 감독이 다시 계산해 모두 맞았다.
- 지금의 PointBlitz(0.2.x)는 스냅샷 통째 교체(청크 v2, 세대)다. SkyLens(develop `faaa855`)도 구역마다 정밀이 오면 그 구역 점을 통째로 바꾼다(모델 서버 → gRPC → Go 코어 → WebSocket, 화면 `ChunkStore` 의 세대·epoch). 점 id·move·remove 는 어느 경로에도 없다.
- 기존 측정(P4.18)은 스냅샷 통째 교체 기준이라 이 구조를 대표하지 않는다. 렌더러 자체 결과(그리기 시간, WebGPU/WebGL2 원인)는 그대로 유효하다.

## 결정
### 1. 형식: v4 레코드를 바꾸지 않고 나른다
- **차분 메시지 = 32 B 머리 + v4 팩 바이트 그대로.** 머리: 마법 `"PBSD"`, 판 `1`, `seq u32`, `n_add u32`, `n_move u32`, `n_remove u32`, `capacity u32`(이 사건 뒤), 예약 4 B. 서버는 팩을 해석하지 않고 머리만 붙인다(변환 비용 0, 0050 원칙 유지).
- 장면 원점은 스트림 하나에 하나(`origin_gps` 의 ENU 원점, f64). 좌표는 v4 그대로 원점 기준 f32 미터다(theater30 범위 수백 m → f32 간격 0.03 mm 미만). 청크 v2 의 16 비트 양자화는 쓰지 않는다 — 슬롯 갱신은 점 단위라 청크 경계 상자가 없다.
- 청크 v2 와 세대(`Scene`)는 그대로 둔다(스냅샷 입력·PLY 재생·기존 사용자용). 두 경로는 장면 종류로 나뉜다.

### 2. 장면: 슬롯 장면 `SlotScene`(pointblitz-core)
- GPU 점 버퍼 하나, 칸 = 슬롯 id, 칸당 16 B(`f32 x y z | u8 r g b | u8 flags`, flags bit 0 = 보임). 그리기는 `capacity` 개 정점 한 번, 안 보이는 칸은 정점 셰이더가 클립 밖으로 보낸다. 점 셰이더·크기·색 규칙(SPEC §3.2)은 그대로.
- 적용: add·move 는 그 칸을 덮어쓰고 보임, remove 는 보임을 끈다. `capacity` 가 버퍼보다 크면 두 배로 늘리고 GPU 안에서 복사한다.
- **올리는 방법은 측정으로 고른다(측정 전 고정)**: (a) CPU 거울 배열에 적용하고 바뀐 칸의 최소…최대 범위를 한 번 `write_buffer`, (b) 팩을 그대로 올리고 계산 셰이더가 칸에 흩뿌림(WebGPU·native, WebGL2 는 (a)). 기준: replace 사건(seq 6·9, move 26만–54만) 적용 → 화면 반영 p50 이 작은 쪽, 차이가 부트스트랩 95 % 구간 안이면 (a)(경로 하나).
- 한 사건의 레코드는 한 프레임에 모두 적용한다(반쯤 적용된 화면 없음). 사건 사이 순서는 `seq` 대로, 빠진 `seq` 가 있으면 기다린다.

### 3. 전달
- `pointblitz-io`: `slots` 모듈 — 머리 인코드·디코드, v4 디렉터리 읽기(`manifest.json` 을 따라가며 새 사건의 팩을 내놓음), 레코드 검사(id < capacity, 길이 = 20(add+move) + 4·remove).
- 재생 서버는 이 저장소의 재생 서버인 `pointblitz-bench replay --stream <dir>`([0018](0018-replay-server.md))다(PR #94 에서 위치 확인). `pointblitz-server` 는 서버 영상으로서 같은 스트림을 따라간다.
- `pointblitz-server` 재생 서버: `--stream <live/stream 디렉터리>` 로 v4 사건을 원래 시각(`t`)대로 알리고, `/slots/<seq>` 로 차분 메시지를, `/slots/state` 로 지금까지를 합친 하나의 메시지(새로 붙는 클라이언트용)를 보낸다. 서버 영상은 같은 디렉터리를 같은 장비에서 직접 읽는다(0050).
- 브라우저(web)·native 클라이언트는 알림을 받으면 그 사건의 메시지를 받아 `SlotScene` 에 적용한다.

### 4. SkyLens 연결(이 저장소 밖, 기록)
- SkyLens 는 cumulus3d 를 라이브러리로 부르고(`cumulus3d_cli::compose`, 싱크 없음) 메모리 이벤트를 받는다. 0.4.0 은 차분을 **파일 싱크로만** 낸다(`DefaultSinks::with_live_stream`). 그래서 SkyLens 가 쓰려면 (i) 싱크 출력 디렉터리를 켜고 `pointblitz-io::slots` 로 따라 읽거나, (ii) cumulus3d 에 차분 메모리 이벤트를 더해야 한다. (ii) 는 cumulus3d 쪽 변경이라 소유자가 정한다. PointBlitz 는 (i)·(ii) 어느 쪽이든 같은 차분 메시지를 받도록 API 를 둔다(`SlotScene::apply(&[u8])`).
- SkyLens 의 protobuf `Chunk` 에 새 kind(차분 메시지 payload)를 더하고, 코어 저장소는 차분을 `seq` 순으로 모두 보관하거나 합친 상태로 접는다. 이것은 SkyLens 변경이며 0.3.0 배포 뒤 별도 PR 로 한다.

### 5. 측정(0049 규칙, 계획은 측정 전 문서로 고정)
- 입력: 위 theater30 stable 실행(`manifest.json` 14 사건, 팩 13 개)을 사건 시각대로 재생. flight-01 대신 이 데이터가 증분 구조의 기준 데이터다.
- as-is: SkyLens 식 three.js 가 같은 v4 차분을 적용(`BufferAttribute` 칸 덮어쓰기 + `addUpdateRange`, remove 는 칸 숨김) — 증분 구조에서 three.js 로 할 수 있는 가장 나은 방식. 참고로 "구역 통째 교체"(지금 SkyLens, pointblitz 0.2.1)도 같은 사건으로 잰다.
- to-be: 브라우저 WebGPU · webgl · native · 서버 영상.
- 지표: 사건 반영(`event_latency` = 사건 알림 → 그 사건 레코드가 모두 보이는 첫 프레임, 사건 종류별), 받은 바이트, 마지막 288만 점에서 그리기 시간(orbit, batch 30), 메인 스레드 멈춤, mem_cpu. 목표 수치는 측정 계획 문서에서 측정 전에 정한다.

## 구현 메모(PR #94)
- **사건별 capacity**: v4 `manifest.json` 에는 마지막 capacity 만 있다. 메시지 머리의 capacity(이 사건 뒤)는 그 사건까지 add id 의 최댓값 + 1 로 계산한다(cumulus3d 의 `capacity++` 규칙과 같음, theater30 에서 마지막 값이 매니페스트의 2,881,929 와 같다).
- **`/slots/state` 는 `seq` 가 필수**(`?seq=N`, 없으면 400): 재생 서버는 `/events` 구독마다 시계가 따로라 "지금까지" 는 클라이언트가 정한다.
- **접은 상태 = `clear()` 뒤 적용**: 접은 상태는 add 만 있는 보통 메시지라 머리에 표시가 없다. 클라이언트가 `SlotScene::apply_state`(장면을 비우고 그 seq 부터 시작, 보관 중인 뒤 사건은 이어서 적용)로 적용한다. 새 장면은 seq 1 또는 접은 상태로만 시작한다(`expect` 로 명시할 수 있다). 사건 메시지를 몇 번 다시 받아도 못 받으면 그 사건까지 접은 상태로 대신한다.

## 대가
- 차분은 바이트가 더 많다(이 실행 125.4 MB, 통째 한 번의 2.2 배). replace 의 move 가 크기 때문이다. 대역폭은 제약이 아니다(INTENT 원칙 2).
- 장면 종류가 둘(`Scene` 세대, `SlotScene` 슬롯)이 된다. 그리기 셰이더는 같고 버퍼 관리만 다르다.
- 0.3.0 은 새 API·새 메시지를 더한다. 청크 v2 는 그대로라 0.2.x 클라이언트와 스냅샷 경로는 호환된다.

## 다시 볼 조건
- cumulus3d 가 차분을 메모리 이벤트로 내거나 v4 레코드를 바꾸면 §1·§3 을 다시 본다.
- (a)·(b) 둘 다 replace 사건 반영이 프레임 하나(16.7 ms)를 넘으면 사건을 여러 프레임에 나눠 적용할지 다시 본다(지금은 "반쯤 적용된 화면 없음" 이 우선).
