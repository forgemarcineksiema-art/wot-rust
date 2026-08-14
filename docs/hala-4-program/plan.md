# Hala v4 — uczciwa klatka i pełny dom (program)

Status: ZATWIERDZONY 2026-08-14. Następca Hali 3.0 (`docs/hala-3-program/plan.md`,
zakończona 2026-08-10, PR #536–#557). Ten dokument jest rejestrem stanu programu —
aktualizowany po każdym zmergowanym PR.

Decyzje bazowe (usera, wiążące):

- Wchodzą wszystkie cztery fale: rozliczenie, perf („uczciwa klatka"), rzemiosło, produkt.
- **Proficiency załogi przybite do 1,0, suwak schowany** — kred „progression is proof,
  never power" nie znosi darmowego, zdominowanego wyboru karzącego niewiedzę.
- D2 (odbicie planarne posadzki) wchodzi WYŁĄCZNIE warunkowo, za bramką pomiaru.
- M (HangarBlueprint / wiele hangarów) pozostaje skreślone na stałe — nie wraca w tym programie
  ani w żadnym następnym bez nowej decyzji.

## Dług wejściowy — pomiar F1 (#543), przepisany do drzewa

Liczby żyły dotąd wyłącznie w ciele commita `4e255fe6^2`; program, który ma je spłacić,
musi je nosić przy sobie. MX330 @ 1920×1080 offscreen, shipowane 1×, gorący box,
rotacja interleaved:

| pozycja | wartość |
|---|---|
| klatka garażu GPU p50 / p95 | **19,66 ms / 22,55 ms** vs budżet 16,67 ms |
| scene_pass | **15,96 ms = 81% klatki GPU** (fill-bound) |
| shadow / ssao-chain / bloom / post / fxaa | 0,61 / 1,36 / 0,51 / 0,50 / 0,61 ms |
| ablacja: no interior grain (C1) | −1,16 ms |
| ablacja: no shadows | −2,10 ms (pass kosztuje 0,61 — reszta siedzi W scene_pass) |
| ablacja: no ssao | −3,35 ms (łańcuch kosztuje 1,36 — jw.) |
| fence (koszt przyrządu) | 8,47 ms |
| bitwa w tym samym procesie | 19,49 ms wall p50 — „pokój kosztuje tyle co pole bitwy" |

Werdykt F1: debet ~3 ms, zarejestrowany; poprzeczka MX330 odroczona do po W1 decyzją
2026-08-07. Konsekwencja: **F2 (4×MSAA garażu) twardo zagate'owane** tym debetem, a rezyduum
G9 (goldeny garażu muszą renderować to, co gra shipuje) wróci w chwili, gdy F2 wejdzie.

Fakty nośne pod dietę (zweryfikowane na masterze `4d4115b4`):

- Delty ablacji PRZEKRACZAJĄ koszty własnych passów — różnica mieszka w scene_pass:
  półcień garażu to 8 tapów porównania przy rozrzucie 9 texeli (`shadow_common.wgsl`;
  bitwa: 4 tapy / 1 texel), a promień SSAO jest przypięty do clampu 48 px (wąski FOV 32°
  + bliskie głębie 5–15 m) — 12 rozrzuconych `textureLoad` na piksel AO, wrogie cache'owi.
- Hero jest rysowany OSTATNI (`draw_world_opaque`: statics → … → vehicles → sky) — cała
  jego sylwetka to overdraw najdroższego shadera klatki.
- Hala jedzie slotem terenu jako JEDEN mesh w chunku 80 m — frustum culling wewnątrz
  pokoju nie robi nic; ~13,4k trójkątów, więc koszt jest per-piksel, nie per-wierzchołek.
- `WOT_MSAA=4` działa end-to-end już dziś — cenę F2 da się ZMIERZYĆ zanim powstanie
  jakikolwiek kod.
- C1: `detail_normal` liczy `ground_grain` tylko dla gradientu, a `material_detail` osobno
  liczy `value_noise` na tej samej płaszczyźnie — jedna ewaluacja `value_noise_grad`
  obsłużyłaby obie.

## Fala 0 — rozliczenie (1 PR)

Dokumentacja mówi prawdę o stanie: checklista Hali 3.0 odhaczona wg faktów; werdykt
„the garage owes its rebuild" zdjęty z ROADMAP (zastąpiony listą długów v4); rejestr
G-audytu zaktualizowany (G5/G6/G11 zamknięte przez B2/F1); proza wartości garażu
w `art-direction-program.md` zgodna z własną tabelą (72,4% dark, pod boundem); ten
dokument (dług F1 w drzewie).

## Fala P — uczciwa klatka (dieta → F2 → G9 → D2)

Reguła fali: measure → cut → re-measure w JEDNYM procesie interleaved (`perf_capture`,
blok garażu); absoluty z gorącego boxa niecytowane — tylko delty z rotacji; goldeny
re-recordowane wyłącznie garage-scope z uzasadnieniem i side-by-side; nietkniętość bitwy
dowodzona bajtami w obu scope'ach.

- **P1 — pomiar-atrybucja.** Rozbudowa `garage_frame_time_capture`: tabela per-pass dla
  KAŻDEJ konfiguracji (dziś drukowana tylko dla pierwszej); bloki interleaved: hall-only /
  vehicle-only / shell-only (konstruktory ablacyjne w `scene_build` z testami podzbioru) /
  no-fx / battle-kernel (softness 0) / **garage @4×** (pierwsza połowa liczby GO/NO-GO) /
  **@720p** (potwierdzenie fill-bound). Wynik: rankingowa tabela atrybucji + zmierzona
  dopłata 4×. Ranking z P1 przesądza kolejność i skład P2–P6.
- **P2 — hero-first draw order** (cel 0,7–1,5 ms): pojazdy przed statics w
  `draw_world_opaque`; early-Z zabija shading hali za kadłubem. Dowód bajtowy goldenów
  w OBU scope'ach; przy dryfie bitwy (koplanarne kontakty) — flaga garage-only.
- **P3 — dieta SSAO garażu** (cel 1,5–2,5 ms z 3,35): wnętrzowy cap promienia/tapów przez
  nową lane uniformu; bitwa pisze 0 = bit-exact.
- **P4 — dieta jądra półcienia** (cel 0,8–1,5 ms z 2,10): 8 → 4–6 rotowanych tapów;
  A/B ograniczone blokiem battle-kernel z P1 przed edycją shadera.
- **P5 — unifikacja ziarna wnętrz** (cel 0,5–0,9 ms z 1,16): jedna ewaluacja
  `value_noise_grad`; gałąź outdoor nietknięta.
- **P6 (warunkowy) — depth prepass garażu**: tylko jeśli po P2–P5 klatka nad budżetem
  i zmierzony overdraw netto > ~1,3. Nowy `PassId` (append-only) + aktualizacja testów
  frame-graphu. (Cache shadow-map tylko przy braku ≤0,5 ms do bramki.)
- **P7 — bramka F2-GO** (zapis pomiaru): garage @4× po diecie, GPU p50 − fence ≤ 16,67 ms
  na MX330. Dieta musi znaleźć ~5–7 ms (3 debetu + zmierzona dopłata 4×, szacunek wstępny
  +2–4 ms — P1 zastępuje go liczbą). Fail → F2 parkuje z JAWNYM zapisem niedoboru tutaj:
  one-look zabrania i klatki poniżej 60 FPS na nazwanym boxie, i opcji jakości tym bardziej.
- **P8 — F2 + G9.** Polityka per-scene ZOSTAJE w `msaa.rs` (`scene_sample_count(ScenePurpose)`;
  `shipped_sample_count` forwarduje do wariantu Battle — bramka `render_sample_count.rs`
  nie zmienia właściciela); `SceneRenderer::set_sample_count` przebudowuje 8 pipeline'ów
  z REUŻYCIEM bind-group-layoutów przy swapie sceny (decyzja wykonawcza Hali 3.0: rebuild
  przy swapie, nie drugi zestaw); FXAA w garażu ZOSTAJE w v1 (artefakt A/B w PR; bypass
  tylko jeśli przegląd go zażąda). W TYM SAMYM PR parytet G9: ścieżka review garażu
  przechodzi na purpose garażu, goldeny garażu re-record na 4×, bitwa bajtowo dowiedziona,
  przyrząd mierzy odtąd shipowany garaż.
- **P9 — decyzja D2** (odbicie planarne deku): prototyp za flagą probe, wyceniony jako blok
  ablacji PRZED jakimkolwiek kodem shipowanym; bramka D2-GO: @4× p50 + zmierzony koszt D2
  ≤ 15,7 ms (margines 1 ms — pokój nie pożycza z powrotem długu, który właśnie spłacił).
  Fail → zapis odroczenia z ceną, tutaj.

## Fala R — rzemiosło

- **R1 — legenda mm inspektora**: pasek legendy w HUD przy aktywnym overlay'u; kolory
  PRÓBKOWANE z `color_for_mm()` (`vehicle/armor_overlay.rs`) — lock: legenda == funkcja
  skali; golden `garage_inspector` re-record scoped.
- **R2 — dźwięk pracy naprawy**: głos klucz/grzechotka w `crates/runtime/audio` (czysty
  DSP, deterministyczny), wpięty w beat 3,2 s (`garage/wear.rs::tick_repair`); lock poziomów
  w mikserze (wzór G1: praca słyszalna nad bedem).
- **R3 — mechanik podchodzi do naprawy** (K2↔L2): na czas beatu cel na KRAWĘDZI ringu 8 m
  po stronie pojazdu — kontrakt „nigdy w ringu" NIETKNIĘTY (lock 240 próbek zostaje);
  po beacie wraca do rundy; całość pod `MECHANIC_ENABLED`.
- **R4 — G7, prawo faz w hali**: `solid::chamfer` / `roundness::segments_for_radius` na
  krawędziach hali/propów; pomiar trójkątów przed/po; scoped re-record goldenów.
- **R5 — B2 do celu (1,4 m / 32 promienie)**: NAJPIERW przyspieszenie gathera (cel 2×);
  wchodzi tylko przy prewarm ≤1 s — inaczej zapis „zamknięte pomiarem" w tabeli na
  `MAX_EDGE_M` w `hangar_bake.rs`.
- G12 (6 slotów światła pełnych): nie ruszamy; jeśli którykolwiek punkt zażąda slotu —
  decyzja jawna w tym PR.

## Fala W — produkt (garaż jako hub)

- **W1 — proficiency = 1,0**: suwak, `GarageHit::CrewProf` i klawisze −/= usunięte; `Crew`
  przybite; stare `crew_proficiency` z persystencji wczytywane czysto (migracja w górę);
  kolumna CREW zostaje jako prezentacja załogi; locki: `assembled_spec` bez kary, stary
  plik round-tripuje.
- **W2 — ekran wyników bitwy**: agregacja CLIENT-SIDE z już odbieranych zdarzeń ingest
  (`DamageLog`, potwierdzenia killi, outcome) — zero zmian protokołu; przepływ
  G → ekran wyników → garaż; ekran pod regułą
  `every_garage_screen_is_under_an_image_lock` (nowy golden).
- **W3 — lokalna historia bitew**: rekord (data, mapa, pojazd, wynik, statystyki z W2)
  do pliku obok `garage.json` — pierwsza odpowiedź na „a record that a battle happened
  at all" z ROADMAP; locki: round-trip + zepsuty plik → `.bak`.
- **W4 — ekran SETTINGS**: wejście z garażu (Escape); v1 = master_gain (koniec przybitego
  0,85 w `mixer.rs`) + override pory dnia (klawisz L dostaje dom w UI) + rezerwacja pod
  strukturę z `docs/wgpu-capability-model.md`; persystencja obok garage.json; golden ekranu.
- **W5 — font PL**: bake Latin-2 w `ui_kit/font/bake.rs` (dziś `0x20..=0x7E`); assert
  ASCII → assert POKRYCIA charsetu; `ui_strings.rs` bez zmiany struktury; lock: żaden
  string nie renderuje luki glifu.
- **W6 — paleta przycisku BATTLE**: czerwień SIGNAL wciągnięta do palety
  (`art-direction-program.md`) albo zapisana jako świadomy wyjątek — jednym małym PR.

## Kolejność i zależności

Fala 0 → **P1 najpierw** (rankinguje resztę fali P). Fale R i W niezależne od P — idą
przeplotem między PR-ami diety (osobne gałęzie, 1 branch = 1 PR z mastera). Twarde
zależności: P7 po P2–P5(±P6) · P8 po P7-GO · P9 po P8 · W3 po W2 · R5 za własnym pomiarem.
Rozmiar: ~20–23 PR.

## Poza zakresem (jawnie)

M/HangarBlueprint (skreślone na stałe) · wejście multiplayer z garażu (fala N4 programu
multiplayer — szew sieciowy, nie hala) · onboarding/tutorial · edytor keybinds (viewer może
dojść później) · zmiana silnika / realtime GI / opcje jakości dla gracza.

## Checklista

- [ ] Fala 0: rozliczenie dokumentacji (ten PR)
- [ ] P1: pomiar-atrybucja + bloki ablacji + wycena @4×
- [ ] P2: hero-first draw order
- [ ] P3: dieta SSAO garażu
- [ ] P4: dieta jądra półcienia
- [ ] P5: unifikacja ziarna wnętrz
- [ ] P6 (warunkowy): depth prepass garażu
- [ ] P7: bramka F2-GO (zapis pomiaru)
- [ ] P8: F2 4×MSAA per-scene + parytet goldenów (G9)
- [ ] P9: decyzja D2 pomiarem
- [ ] R1: legenda mm inspektora
- [ ] R2: dźwięk pracy naprawy
- [ ] R3: mechanik przy naprawie
- [ ] R4: prawo faz w hali (G7)
- [ ] R5: B2 GI do celu albo zamknięcie pomiarem
- [ ] W1: proficiency = 1,0
- [ ] W2: ekran wyników bitwy
- [ ] W3: lokalna historia bitew
- [ ] W4: ekran SETTINGS
- [ ] W5: font PL (Latin-2)
- [ ] W6: paleta przycisku BATTLE
