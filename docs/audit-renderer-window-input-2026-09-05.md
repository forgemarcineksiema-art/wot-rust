# Audyt: renderer + okno + wejście (2026-09-05)

> **Stan po realizacji (2026-09-05, ten sam dzień):** kolejka ośmiu PR z końca tego raportu
> jest zamknięta — #702 (raport), #703 (repeat klawiszy), #705 (zatrzaski fokusu), #707 (utrata
> urządzenia), #709 (bramka przebiegu wnętrza), #710 (ceremonia pętli + F11 + pacer za
> monitorem), #711 (start bez bake'ów przed pierwszą klatką), #714 (prawda w `renderer_api`).
> Reversed-Z (znalezisko 4) **zmierzone i odrzucone** — wynik w sekcji „Pomiar reversed-Z".

Zakres: `crates/render/renderer_api`, `crates/render/renderer_wgpu`, pętla okna i wejście klienta
(`crates/apps/client/src/{loop_policy.rs, app/lifecycle.rs, app/loop_step.rs, app/input.rs,
app/input_state.rs, app/render.rs, app/garage_render.rs, app/camera_link.rs, aim.rs, camera/*}`),
reguły ratchetu `quality` dotyczące tych warstw (`render_pass_recorder.rs`, `render_sample_count.rs`,
`winit_loop_rules.rs`, `architecture_rules.rs`).

Metoda: przeczytany kod (ok. 9,5k linii renderer_wgpu, 4,5k renderer_api, ~13k klienta w zakresie),
testy tych warstw, reguły ratchetu, historia gita od 2026-07, źródło winit 0.30.13 z rejestru cargo
(zachowanie fokusu i repeatu na Windows). **Nie uruchamiałem** gry ani testów w tym worktree (świeża
ścieżka = pełna rekompilacja 33 crate'ów; master jest zielony po #699). Liczby wydajnościowe cytuję
z komentarzy i dokumentów, nie z własnych pomiarów.

## Ocena

| Obszar | Ocena | Jednym zdaniem |
|---|---|---|
| Renderer (`renderer_wgpu`) | **7,5 / 10** | Architektura klatki dojrzała i zamknięta testami; słabe tryby awarii (utrata urządzenia), brak reversed-Z, zbędna praca w przebiegu wnętrza. |
| Okno / pętla | **6,5 / 10** | Pacer dobry i przetestowany; reszta na poziomie prototypu: start blokuje okno, brak pełnego ekranu, zmiana monitora ignorowana, ceremonia w `loop_policy` zamknięta testami na stringi. |
| Wejście | **6 / 10** | Łańcuch mysz → cel → działo staranny i dobrze zamknięty; surowa warstwa ma realny błąd (auto-repeat klawiszy), sztywne bindingi bez warstwy akcji, reset przy utracie fokusu połowiczny. |
| `renderer_api` | **5 / 10** jako kontrakt | Struktury `RenderBackend`, `RenderFeaturePlan`, `DebugToolPlan`, `PipelineWarmupPlan`, `WgpuLabelPolicy` deklarują rzeczy, których backend nie robi. To dług „prawdy w kodzie", którego ratchet nie widzi. |

Realistycznie: to jest renderer i pętla **działającego prototypu z bardzo dobrą dyscypliną
pomiarową**, nie produkt. Rdzeń klatki (graf przebiegów, rejestrator, budżety, polityka MSAA) jest
lepszy niż w większości indie projektów. Wszystko wokół klatki (start, awaria, okno, ustawienia,
bindingi) jest na etapie „działa na moim laptopie".

## Znaleziska

Posortowane od najpoważniejszych. Każde ma plik i linię, skutek, dowód i naprawę z testem.

### 1. Auto-repeat klawiszy nie jest filtrowany (WYSOKI, wejście)

`crates/apps/client/src/app/input.rs:12-18` bierze tylko `event.state`, ignoruje `event.repeat`
(winit 0.30 ma to pole: `event.rs:647`). Windows generuje powtórzone `Pressed` co ~33 ms po ~500 ms
trzymania. Skutki w `on_driving_keyboard` (`input.rs:57-101`):

- **V trzymane** → `toggle_camera_mode()` na każdy repeat: widok miga TPP↔snajper ~30 razy na
  sekundę, każde przełączenie odpala `UiClick` (`render.rs:592-596`) i reset zoomu.
- **Esc trzymany w bitwie** → `open_pause_menu` / `close_pause_menu` na przemian; grab kursora
  włącza się i wyłącza co repeat.
- **Spacja trzymana podczas przeładowania** → `fire_pending` zatrzaskiwane co repeat →
  `register_fire_intent_feedback` (`loop_step.rs:210-242`) gra `UiReject` i czerwony puls
  ~30 razy na sekundę przez całe przeładowanie.
- **1/2/3 trzymane** → `select_ammo` wysyłane w każdej paczce ticków (szum na drucie).
- **G trzymane** → `open_garage()` co repeat (`garage/actions.rs:34-63`: `dust_from_the_field`,
  `wear_from_the_field`, `garage.open()` w kółko).

Dowód, że autorzy znają problem: Shift (`input.rs:214-224`, komentarz „Swallow winit key-repeat")
i Alt (`input.rs:73-79`) są chronione ręcznie. Pozostałe klawisze krawędziowe nie.

Naprawa: jeden `if event.repeat { return; }` w `on_keyboard` dla klawiszy krawędziowych (strzałki
w garażu mogą repeat przyjmować). Test: szew testowy `on_battle_keyboard(PhysicalKey, bool)` **nie
umie wyrazić repeatu** — musi dostać trzeci argument, inaczej błąd pozostaje nietestowalny.

### 2. Utrata urządzenia GPU bez ścieżki odzyskania; fallback na rasteryzator programowy (WYSOKI, renderer)

- `window_renderer.rs:207-213`: `CurrentSurfaceTexture::Lost` → `surface.configure` i `Ok(())`.
  Po prawdziwym TDR (reset sterownika) urządzenie jest martwe; `configure` na martwym urządzeniu
  nic nie naprawia. Gra zostaje czarnym oknem z `warn!` co klatkę, bez wyjścia, bez komunikatu,
  bez odtworzenia `WindowRenderer`. `gpu_context.rs:81-88` tylko loguje. Komentarz w kodzie
  uczciwie mówi, że wcześniej „polityka obiecywała handler, który nie istniał" — handler istnieje,
  ale nadal nic nie robi.
- `gpu_context.rs:104-112`: gdy `HighPerformance` adapter zawiedzie, klient okienkowy bierze
  `force_fallback_adapter: true` (DX12 WARP). Na laptopie z popsutym sterownikiem gra „działa"
  w 1 FPS bez słowa. Dla `GpuDeviceType::Cpu` ścieżka okienkowa powinna odmówić głośno.

Min-spec to laptop Optimus z MX330 — resety sterownika przy uśpieniu i aktualizacjach to
codzienność tej klasy sprzętu. Naprawa: licznik kolejnych `Lost`/błędów → odtworzenie
`WindowRenderer` (jest to możliwe: cały stan GPU wynika z `battle_scene_meshes` i katalogu
pojazdów) albo czyste wyjście z komunikatem. Test: bez GPU da się zamknąć tylko politykę
(„N kolejnych Lost = recreate"), ale to lepsze niż nic.

### 3. Start blokuje proces przed pierwszą klatką (ŚREDNI, okno)

Kolejność w `app/mod.rs:872-884` i `lifecycle.rs:14-58`:

1. `prebake_playable_vehicle_assets()` — 8 proceduralnych bake'ów pojazdów **zanim istnieje okno**
   (`vehicle_assets.rs:41-52`; własny komentarz: „several-hundred-millisecond stall" per rodzaj,
   chyba że `target/forge` jest na dysku — w paczce dystrybucyjnej nie będzie).
2. Ścieżka `WOT_CONNECT` (`mod.rs:689-701`): pętla `pump` + `sleep(10 ms)` do **60 s** bez okna.
3. `resumed` tworzy okno, potem synchronicznie: bake mapy (275–517 ms wg komentarzy),
   `preload_battle_vehicle_assets`, `WindowRenderer::new` (adapter, urządzenie, ~15 pipeline'ów
   kompilowanych synchronicznie, `cache: None` wszędzie), upload atlasów.

Okno pojawia się i wisi. Windows po ~5 s bez pompowania komunikatów pokazuje „Nie odpowiada".
Nie mierzyłem łącznego czasu; komentarze sumują się do sekund. Naprawa: stan „ładowanie"
(pierwsza klatka = czysty kolor + tekst), bake'i na workerze (wzór już istnieje:
`MapPrebake`, `hangar::prewarm`), pipeline cache. Test: `resumed` musi zwrócić przed końcem
pierwszego bake'u (fakt, nie liczba).

### 4. Standardowa głębia bez reversed-Z przy 0,5 m…2600 m (ŚREDNI, renderer, do zmierzenia)

`renderer_api/src/scene.rs:305-311`: `Mat4::perspective_rh` + `Depth32Float` + `Less`
(`scene_pipeline.rs:163`, `vehicle_pipeline.rs:101`). Rozdzielczość głębi ≈ `ulp(1) · z² / near`
= 1,2e-7 · z²:

| Odległość | Kwant głębi |
|---|---|
| 600 m (pojedynek snajperski) | ~4 cm |
| 1000 m | ~12 cm |
| 2500 m (fartuch mapy) | ~75 cm |

Przy powiększeniu ×20 w lunecie i detalu T-54 rzędu 2–5 cm (rantach luków, śrubach) 4 cm kwantu
to kandydat na iskrzenie z-fightingiem dokładnie w widoku, przez który gracz celuje.
Komentarz w `depth_convention.rs` („precision is dominated by the near plane, so the raise costs
nothing") jest prawdziwy dla głębi standardowej i właśnie dlatego reversed-Z jest darmowym zyskiem:
zamiana `near/far` w projekcji, `Clear(0.0)`, `Greater/GreaterEqual` w 9 pipeline'ach, linearizacja
w `ssao.wgsl`, niebo na płaszczyźnie dalekiej = 0. Mapy cieni nietknięte. **Najpierw pomiar**:
sonda snajperska na 600 m przed i po, różnica pikseli na rantach luku.

### 5. Reset wejścia przy utracie fokusu: fałszywa przesłanka i niepełny zakres (ŚREDNI, wejście)

`input.rs:114-124`: „An unfocused window receives no key or button releases". Na Windows winit
0.30.13 **syntetyzuje** zwolnienia klawiszy przy `WM_KILLFOCUS` i wciśnięcia przy `WM_SETFOCUS`
(`platform_impl/windows/keyboard.rs:95-103`). Więc:

- przesłanka jest fałszywa na jedynej platformie, którą gra wysyła;
- `release_driving` (`input_state.rs:19-26`) czyści WASD/hamulec/ogień, ale **nie** `free_look`,
  `free_look_return_pitch`, `sniper_hold_return`, `shift`, `wheel_pending_lines`. Dziś ratują je
  syntetyczne zwolnienia winit; na innej platformie lub po zmianie winit Alt+Tab zostawia free-look
  zatrzaśnięty (mysz rusza kamerą, nie działem);
- syntetyczne **wciśnięcia** przy powrocie (Alt nadal trzymany podczas Alt+Tab) uruchamiają
  `begin_free_look` — nieszkodliwe, ale nieobjęte testem.

Test fokusu istnieje tylko po stronie garażu (`garage/actions.rs:871-892`); żaden nie sprawdza
zatrzasków bitwy. Naprawa: `release_driving` → `release_all_latches`; test na każdy zatrzask.

### 6. Pacer znaje odświeżanie tylko z chwili startu; brak pełnego ekranu, ratchet go zakazuje (ŚREDNI, okno)

- `lifecycle.rs:38-42`: `refresh_rate_millihertz` czytane raz w `resumed`. Przeniesienie okna na
  monitor 144 Hz → pacer trzyma 60; z 144 na 60 → 144 beatów na 60 Hz panelu (Mailbox gubi
  klatki). Brak obsługi `WindowEvent::Moved` i `ScaleFactorChanged`.
- Pacer to zegar ścienny bez synchronizacji z vblank: faza dryfuje względem panelu, więc co kilka
  minut jedna klatka podwójna lub zgubiona. Znane ograniczenie wgpu (brak present feedback), warto
  zapisać jako świadomy kompromis.
- Brak jakiegokolwiek trybu pełnoekranowego (borderless). `quality/tests/winit_loop_rules.rs:14`
  asercja `!lifecycle.contains("Fullscreen")` **zakazuje** go dodać. Dla gry na Steam borderless to
  minimum; ratchet zamroził wygodę deweloperską jako regułę.

### 7. Ceremonia w pętli i testy na stringi w ratchecie (ŚREDNI, jakość)

- `loop_policy.rs:140-152`: `event_driven_phases()`, `uses_manual_event_polling()` (stała
  `false`), `ClientLoopPhase`, `ClientLoopAction::CaptureInput` (`loop_step.rs:20`: `=> {}`) —
  nic nie robią.
- `quality/tests/winit_loop_rules.rs:5-33`: `contains("ApplicationHandler")`,
  `contains(".with_maximized(true)")`, `contains("uses_manual_event_polling() -> bool")`.
  To teatr testowy: zamyka literówki, nie zachowanie, a blokuje zmianę (pełny ekran).

Kontrast: testy pacera (`loop_policy.rs:189-251`, `tests/winit_loop_policy.rs`) są behawioralne
i dobre. Zalecenie: usunąć ceremonię i stringowe reguły, zostawić testy zachowania.

### 8. `renderer_api` deklaruje możliwości, których backend nie ma (ŚREDNI, uczciwość kodu)

- `RenderBackend` (`renderer_api/src/lib.rs:260-268`): `WindowRenderer` go nie implementuje.
- `RenderFeaturePlan::baseline()` (`feature_plan.rs:43-60`) włącza `ForwardPlus`,
  `OcclusionCulling`, `DebugDraw`. Renderer robi zwykły forward z 6 światłami w uniformie,
  culling frustum po chunkach, żadnego occlusion cullingu, żadnego debug draw.
- `DebugToolPlan::first_week()` (`debug_tools.rs:22-42`): 13 narzędzi „pierwszego tygodnia".
  Zaimplementowane w rendererze: zero (jest `FrameProfiler`, ale nie przez tę ścieżkę).
  `docs/engineering-rules.md` mówi „Debug tools and GPU labels are first-week systems" — typy są,
  narzędzi nie ma.
- `WgpuLabelPolicy::required_startup_labels` (`gpu_diagnostics.rs:5-12`) wymienia
  `shadow_map_2048`, `tank_pbr_pipeline`, `terrain_depth_prepass_pipeline`,
  `shell_tracer_vertex_buffer`. Faktyczne etykiety to `sun_shadow_map`, `shadow_pipeline_scene`,
  `ssao_prepass_scene`, `scene_fx_v`.
- `PipelineWarmupPlan` / `PipelineCacheMode::ReleasePrewarm`: każdy `create_render_pipeline` ma
  `cache: None`.

Te typy mają własne testy (`capability_tiers`, `feature_fallbacks`, `debug_tool_plan`,
`pipeline_key`, `bind_group_policy`), które zamykają zgodność typu z samym sobą. To ten sam wzór
„kłamstwa w docs" z audytu 2026-08-03, tylko w kodzie. Albo backend ma to robić, albo typy
znikają.

### 9. Przebieg wnętrza (Z6) rysuje całą flotę co klatkę bez względu na przebicia (NISKI, renderer, perf)

`draw.rs:480-497`: druga pętla po `vehicle_draws` z pipeline'em wnętrza. „Intact hulls collapse in
`vs_interior` and cost no fragment" — ale vertex shader i tak przetwarza wszystkie indeksy
(T-54 ~21,5k trójkątów × 14 pojazdów) przez większość meczu, gdy nikt nie jest przebity. Bramka na
poziomie klatki: pusty `frame.armor_damage` → pomiń pętlę. Zmierzyć na MX330; zysk pewnie
0,2–0,5 ms VS w scenie, za darmo.

### 10. Ostrzeżenia budżetu bez limitu częstotliwości (NISKI)

`scene_renderer/resources.rs:23-28, 64, 85, 168-172, 189-193`: `tracing::warn!` co klatkę przy
przekroczeniu. Regresja = 60 wpisów na sekundę. Raz na sekundę lub raz na zmianę wartości.

### 11. Drobne

- `draw.rs:423-428`: komentarz uzasadnia kolejność rysowania „dziurą (None) w group 1 układu
  sceny". `scene_pipeline.rs:131` ma `[camera, foliage, shadow]` — dziury nie ma. Uzasadnienie
  nieprawdziwe, kolejność być może nadal słuszna z innego powodu.
- `render.rs:741`: `set_dynamic_mesh(&[], &[])` co klatkę bitwy (czyszczenie po garażu) — dwa
  zerowe `write_buffer`; należy do `ensure_scene` na przejściu.
- `draw.rs:66-78`: tekstura `bloom_black_fallback` 1×1 tworzona przy każdym odtworzeniu łańcucha
  HDR, gdy bloom wyłączony — trywialne, ale trwała 1×1 wystarczy.
- Resize: każde `Resized` rekonfiguruje surface od razu, a następna klatka odtwarza HDR, LDR,
  głębię, łańcuch SSAO i bloom (`post.rs:186-290`, `ssao.rs:118-170`). Przy przeciąganiu okna
  na 4K to dziesiątki MB alokacji per zdarzenie, bez debounce. Akceptowalne, do zapisania.
- Kursor `Confined` (Windows nie ma `Locked`, `input.rs:340-343`): przy otwarciu menu ESC kursor
  pojawia się tam, gdzie dodryfował do krawędzi. Kosmetyka.
- Testy GPU (13 plików w `renderer_wgpu/tests`) przy braku adaptera robią `eprintln!` + `return` i
  **przechodzą**. Na maszynie bez GPU bramka jest zielona nie sprawdzając nic. Przydałby się tryb
  `WOT_REQUIRE_GPU=1`, w którym pominięcie = porażka.
- Brak jakiegokolwiek dokumentu sterowania; jedyny opis bindingów to `info!` w `lifecycle.rs:53`.

### 12. Znane i zapisane, nadal otwarte

- Klatka garażu 17,2 ms GPU przeciw budżetowi 16,67 ms na MX330 (`docs/ROADMAP.md:127`) —
  pacer w garażu nigdy nie trafia 60. Nie moje odkrycie, ale należy do tej oceny.
- Cała klatka CPU (ticki, predykcja, HUD, minimapa, populacja trawy, LOD drzew, wierzchołki FX)
  na jednym wątku; workery tylko do bake'ów. Instrument F9 mierzy p95 odstępów klatek, ale nie
  ma podziału CPU/GPU ani etapów CPU. Gdy p95 skacze, nie wiadomo z czego.

## Co jest dobre (i co warto chronić)

- **Rejestrator przebiegów** (`pass_recorder.rs`): jedno miejsce otwiera `RenderPass`, liczy
  draw/trójkąty/instancje na każdej klatce bez feature'ów GPU, sloty timestampów w kolejności
  kodowania (brak dziur w resolve). Ratchet `render_pass_recorder.rs` broni tego strukturalnie.
- **Graf klatki** (`frame_graph.rs`): tabela `FRAME_GRAPH` z warunkami; kod pyta tabelę zamiast
  dublować `if`-y; `PassId` append-only.
- **Polityka MSAA w jednym miejscu** (`msaa.rs` + ratchet `render_sample_count.rs`) po tym, jak
  instrument i gra rozjechały się na 4×/1×. Lekcja zamieniona w regułę, nie w komentarz.
- **Podział `prepare_frame` / `encode_frame`** (`draw.rs:17-33`): tworzenie zasobów tylko w
  `&mut self`, kodowanie w `&self`; koniec `RefCell` w ścieżce klatki.
- **Budżety obcinają, nie gubią** (`resources.rs`): przepełnienie = ostatnie obiekty, nie zamrożona
  klatka. Klient budżetuje FX przeciw `fx_vertex_budget()` — dwa końce jednej liczby.
- **Pacer + `timer_resolution`**: jedno `unsafe` w całym workspace, w kwarantannie, z testem.
- **Łańcuch celowania** (`aim.rs`, `camera_link.rs`, `sniper.rs`): światowy promień celu,
  rozwiązanie strzału jedno dla działa i celownika, wysokość optyki jako liczba referencyjna
  z pasmem, sterownik czasowo-optymalny zamiast P-gain — każde z testem i z liczbami.
- **Modal ESC** (`input.rs`, `pause_menu.rs`): jedna geometria dla rysowania i hit-testu, nic
  nie podświetlone przy otwarciu, zwolnienie kluczy jazdy — dobrze przemyślane i zamknięte
  8 testami.
- **Kolejność w beacie**: `about_to_wait` (ticki) → `RedrawRequested` (render), mysz zbierana
  z raw input (`DeviceEvent::MouseMotion`, domyślnie tylko przy fokusie), aplikowana per klatka,
  nie per tick — zamknięte testem `fixed_ticks_do_not_consume_mouse_look…`.

## Rekomendowana kolejka PR

Każdy mały, z testem, w tej kolejności:

1. **Repeat klawiszy** — filtr w `on_keyboard`, szew testowy z flagą `repeat`, 3 testy
   (V, Esc, Spacja). Godzina roboty, usuwa błąd, który każdy gracz trafi w pierwszym meczu.
2. **Zatrzaski przy fokusie** — `release_all_latches`, poprawka komentarza o winit, test na
   free-look i Shift-hold.
3. **Polityka utraty urządzenia** — licznik `Lost`, odtworzenie `WindowRenderer` lub czyste
   wyjście; odmowa adaptera `Cpu` w ścieżce okienkowej z komunikatem.
4. **Pomiar reversed-Z** — sonda snajperska 600 m, jeśli różnica widoczna: PR z 9 pipeline'ami.
5. **Bramka przebiegu wnętrza na pustym `armor_damage`** + pomiar kanapką A→B→A.
6. **Usunięcie ceremonii pętli i stringowych reguł** `winit_loop_rules.rs`; zdjęcie zakazu
   `Fullscreen`; borderless na F11 z zapisem stanu okna.
7. **Stan ładowania**: pierwsza klatka przed bake'ami, bake'i na workerze, pipeline cache.
8. **`renderer_api` mówi prawdę**: usunąć `RenderBackend`, `DebugToolPlan`, `PipelineWarmupPlan`,
   poprawić `RenderFeaturePlan::baseline` i `required_startup_labels` do tego, co jest — albo
   zaimplementować. Ratchet powinien porównywać etykiety z faktycznymi `label: Some(...)`.

## Pomiar reversed-Z (2026-09-05, po kolejce)

Hipoteza ze znaleziska 4: kwant głębi ~4 cm na 600 m iskrzy z-fightingiem na okuciach w lunecie
×20. Zmierzone na tej maszynie sondą A/B jednym binarium: reversed-Z za przełącznikiem
`WOT_REVERSED_Z=1` (projekcja z zamienionymi płaszczyznami, `Greater`/`GreaterEqual` w dziewięciu
pipeline'ach kamery, clear 0, niebo na z = 0, linearizacja SSAO z zamienionymi płaszczyznami,
promień fokusu cieni odwrócony); trzy kadry Prochorowki 960×540: kontakt z chase-kamery
(kontrola), snajper 300 m / 8° (kadr przeglądowy), snajper 600 m / 3° (najwęższy stopień drabiny).

| Kadr | Piksele różne w kadrze | W polu celu | Delta > 40 w polu celu |
|---|---|---|---|
| kontakt 55° | 1,44 % (max 96) | 3,39 % | 6 px |
| snajper 300 m / 8° | 9,19 % (max 86) | 20,5 % | 0 px |
| snajper 600 m / 3° | 34,3 % (max 63) | 46,0 % | 1 px |

Histogram delt: 96–99 % wszystkich różnic to < 16/255 — szum próbkowania SSAO i ditheru, który
zmienia się wraz z każdą zmianą bitów głębi. Wycinki kadru 600 m przed i po są dla oka
identyczne; wzmocniona mapa różnic pokazuje kilka izolowanych pikseli na okuciach, żadnej
struktury z-fightingu ani w bazie, ani po zmianie. Kadr statyczny nie widzi migotania w czasie,
ale błąd kwantyzacji pokazałby się jako piksele „nie tej powierzchni” — jest ich 0–1.

**Werdykt:** brak widocznego zysku; reversed-Z nie ląduje (~40 linii w 14 plikach i odczyt env
w `renderer_api` za nic). Hipoteza wycofana. Kod eksperymentu (diff + sonda) leży poza repo
w scratchpadzie sesji; odtworzenie zajmuje godzinę, gdyby kiedyś far plane poszedł dalej niż
2,6 km albo near poniżej 0,5 m — wtedy arytmetyka znów będzie inna.

## Czego nie sprawdziłem

- Nie kompilowałem ani nie uruchamiałem nic w tym worktree.
- Nie mierzyłem czasu startu, kosztu przebiegu wnętrza ani z-fightingu na 600 m — punkty 3, 4, 9
  to hipotezy z kodu i arytmetyki, oznaczone jako „do pomiaru".
- Shadery WGSL czytałem tylko w zakresie uniformu `Camera`; nie oceniałem jakości oświetlenia
  ani zgodności `wgsl_layout`.
- Edytor (`crates/apps/editor/src/app.rs`) ma własną pętlę winit z własnym `FRAME_INTERVAL` i
  własnym grabem kursora; nie audytowałem jej, ale to druga kopia tej samej maszynerii.
- Ścieżka zdalna (`RemoteSession`, `pump`) poza zakresem, poza blokującym startem w punkcie 3.
