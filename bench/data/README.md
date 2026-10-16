# 데이터셋 위치

PLY 원본은 저장소에 넣지 않는다(SPEC §8.3). 아래 sha256 으로 같은 파일인지 확인한다.

## flight-01

- 받는 곳: 프로젝트 소유자에게 요청(공개 배포 위치는 정해지면 적는다)
- 로컬 경로 예: `bench/data/flight-01/`(gitignore 됨)

| 파일 | sha256 |
|---|---|
| event_01_0097.0s_preview_0.ply | `e8b4b7c1c59e43293c144525c7f220fb89b083b02fb809d5366750265a9b0036` |
| event_02_0127.1s_refined_0.ply | `4d2a21f2e100f9cd88dbb9aea1e9af0651c9cac51dcce72d3158869b10599426` |
| event_03_0298.9s_preview_1.ply | `79dc3b34c54c76d698483ae619cf0f9b69a743bc3f3968147ce619a3b221423d` |
| event_04_0327.4s_refined_1.ply | `44ee300e2fe3363fb921c279137147325c85acb84e56e7f26aa44119432a2b9e` |
| event_05_0570.7s_preview_2.ply | `01f4fae95057460fb00d64287ed699334198e758bf3060b4d77acfa4ce544f81` |
| event_06_0623.5s_refined_2.ply | `ddaf32fda37cf2b3d85db2f2c64f25c5b43d548e4544bedf6346a7d9f3667cc5` |
| event_07_0934.8s_preview_3.ply | `d77cbd4aff24f2a5ef921f7bd340baf22ebd29480684e674da83c4f4bc4ef737` |
| event_08_0994.2s_refined_3.ply | `bcab2f9ed03fe95f04206402f3d9cc54298e7df65a81a8428e13689ea90b4f69` |
| event_09_1333.1s_preview_4.ply | `12a630445593cc78a53147de758742079bcfc737051dd10b1680af224631b798` |
| event_10_1388.6s_refined_4.ply | `96b9014296e8cc5b8bfd1520ccad818e907b633fd44b6d6579b88be413bd4290` |
| event_11_1760.7s_preview_5.ply | `a87f17e3efe6eea5f8cddd0bd073daf6610638d274b4ff795d76c94912a5cabf` |
| event_12_1805.1s_refined_5.ply | `1aa5bb14d4e6342ef60c9d1067390bc4aa77da5242d9582df76be702b8e569a3` |
| event_13_2005.6s_preview_6.ply | `7301f4a340da7862bd395ca2e767d4d608c0954cdf21ec89ff61f2928542dfc8` |
| event_14_2053.0s_refined_6.ply | `e1648a6d0c566d286a83d0abe52132e3354347769072a21433f2c7e7ebb6ce90` |

## theater30-stable (슬롯 차분 스트림 v4, 결정 0052)

cumulus3d 0.4.0(`ef0803e`) `--gpu --stable-display --seed 7` 로 theater30 원본 720 장을 돌린 실행의 `live/stream`
디렉터리. 사건 14 개(팩 13 개, 마지막 `final` 은 팩 없음), capacity 2,881,929, 팩 합계 125,354,792 B.

- 받는 곳: 프로젝트 소유자에게 요청
- 로컬 경로 예: `bench/data/theater30-stable/live/stream/`(gitignore 됨)
- 시험: `PB_THEATER30_STREAM=<…/live/stream>` 이면 `pointblitz-io` 의 `tests/theater30.rs`(사건마다
  `points = 직전 + add − remove`, `bytes = 20(add+move) + 4·remove`), `pointblitz-core` 의 `tests/slots.rs`
  (실제 사건 적용 시간)가 돈다. 없으면 건너뛴다.
- 고정 시점: `bench/viewpoints/theater30-stable.json`(`pointblitz-bench viewpoints <live/stream> …`, 결정 0017 방식으로
  마지막 상태에서 뽑음).

| 파일 | sha256 |
|---|---|
| manifest.json | `e815d53c8b4897d15df87c72fa98ff83b1db90aec4b7df490ce5f2a1dcc8806f` |
| packs/e_0001.bin | `0e8569114de7c6249a831b2adcea541da77f2db06f32ba24e11f2ed150af2057` |
| packs/e_0002.bin | `4eb18b181be1bf695dbd851db7f213fa0854b617af78efcbd2243ce2de28357a` |
| packs/e_0003.bin | `4087c51f8dce2cc8f9971c4550dfc81603c74ad06460b7c9055fa6f1635e08ef` |
| packs/e_0004.bin | `c009902bcde237b17281bb3359bc2cc4fce356e9319bfc407b294004d56f1104` |
| packs/e_0005.bin | `242d24836288f075997586ad0006d6d23d9fb09930a64551972d3ce9d9585127` |
| packs/e_0006.bin | `97e78f2162ed4db408d4e802348b3b354687bab98b2c5928ac1ad70671b2e8f9` |
| packs/e_0007.bin | `f3eaeaa614870ee818c6f2e70c19936697bf3f47da41e5af6494b42e4484c0a4` |
| packs/e_0008.bin | `a5dfacf04d0f9ad27019e3dbf1fad374dac4596702c331eb731b23f6a37c0c7f` |
| packs/e_0009.bin | `f91ca20be6e0d2a98852339aa085e568aa9ce7dd55dfe3071b870fe370aaede1` |
| packs/e_0010.bin | `8cd0c13d9a4ff9d7790319ef6e2d554dd0ce727a6faa93304a111db13d6dcccf` |
| packs/e_0011.bin | `88a844b2efcdc29cd216ed1b3772e033973c25cf1562809de36ce86c856deacd` |
| packs/e_0012.bin | `90c7997f3d596eb5d7f9a17788f6eeddc638022de32277a27833c0ba760269a9` |
| packs/e_0013.bin | `d2d9d76123e45c13d3b2cc0605e7eb6f73ebf6ff94d5a318ec59b601fe9513a7` |
