# Hala v4 — uczciwa klatka i pełny dom (program)

Status: ZATWIERDZONY 2026-08-14. Następca Hali 3.0 (zakończona 2026-08-10, PR #536–#557).
Ten dokument jest rejestrem stanu programu —
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

## Pomiar P1 (2026-08-15, #573 zmergowane, MX330, wierny przyrząd)

Pierwszy pomiar SHIPOWANEJ sceny od #554 (F1 mierzył halę bez półcienia, kostki, smug
i cięcia casterów). Rotacja interleaved, 10 konfiguracji × 4 cykle, 120 próbek/konfig;
wall zawiera fence per klatkę (protokół F1); „GPU" = przyrząd per-pass.

| konfiguracja | wall p50 | GPU p50 | scene_pass | delta wall vs full |
|---|---:|---:|---:|---:|
| garage full (shipowana) | 27,43 | **18,88** | 15,50 (82%) | — |
| no interior grain (C1) | 25,60 | 17,67 | 14,30 | −1,83 |
| no shadows | 24,30 | 16,53 | 13,73 | −3,13 |
| no ssao | 24,88 | 17,26 | 15,17 | −2,55 |
| no fx (smugi) | 27,02 | 18,84 | 15,41 | −0,41 |
| battle shadow kernel (softness 0) | 28,64 | 21,37 | 17,49 | **+1,21 (!)** |
| hall only (bez pojazdu) | 25,12 | 17,84 | 14,42 | −2,31 |
| unfurnished hall (bez propów/galerii) | 29,33 | 20,99 | 17,12 | **+1,89 (!)** |
| vehicle on bare slab | 16,58 | 9,21 | 6,33 | −10,85 |
| **garage @4× (kandydat F2)** | 32,71 | **24,80** | 20,91 | **+5,28 (GPU +5,92)** |

Fence p50 5,57 → budżetowo: wall−fence **21,86 ms vs 16,67** (GPU-instrument 18,88 — między
miarami ~3 ms; obie zapisane, delty czytać z rotacji). Koda @720p: GPU 18,88 → 9,84 ms =
×1,92 przy ×2,25 pikseli — **fill-bound potwierdzone (~85%)**.

**Wnioski, które PRZESTAWIAJĄ dietę:**

1. **Dług 1× jest większy niż w rejestrze F1**: ~2,2 ms GPU / ~5,2 ms wall−fence (19,66 było
   pomiarem nie-shipowanej sceny). **Dopłata 4× zmierzona: +5,9 ms GPU** — bramka F2-GO
   wymaga znalezienia **~8 ms GPU** (24,80 → 16,67), nie ~5.
2. **ANOMALIA A (jądro bitewne droższe)**: softness 0 → scene_pass +2,0 ms. Ścieżka bitewna
   próbkuje mapę CHMUR (D21), miękka garażowa nie — dieta półcienia (P4) tnie tapy MIĘKKIEJ
   ścieżki, nie przełącza na bitewną; realny zysk ~0,7–0,9 ms (no-shadows w scene_pass
   = −1,78).
3. **ANOMALIA B (hala bez propów droższa)**: −18k trójkątów → scene_pass +1,6 ms. Propy
   ZASŁANIAJĄ najdroższe piksele wnętrza (ich shading jest tańszy niż to, co zakrywają).
   „Cięcie propów" jako lever jest MARTWE; wygrana leży w nie-cieniowaniu zasłoniętych
   pikseli — **hero-first (P2) i depth prepass (P6) awansują z warunkowego do rdzenia**.
4. **Pokój kosztuje ~9–11 ms scene_pass, pojazd ~2** (goły slab 6,33 vs pełna hala 15,50):
   debet to per-piksel wnętrzowej ścieżki materiału na ~2 Mpx.
5. Ranking diety po P1: **P6 depth prepass** (największy potencjał — overdraw wnętrza) ·
   **P5 unifikacja ziarna** (−1,2 ms zmierzone flagą C1) · **P2 hero-first** (~1–2 ms,
   udział ekranowy hero × koszt wnętrza) · **P4 tapy półcienia** (~0,7–0,9) · **P3 SSAO**
   (łańcuch to dziś 1,26 + 0,33 w scene — zysk ≤ ~1, niżej niż szacowano).
   Zejście pod budżet 1× (2,2 GPU) jest osiągalne bez prepassu; **F2 (+5,9) najpewniej
   wymaga prepassu i wszystkich leverów razem** — decyzja spadnie na bramkę P7 z liczbami.

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

- [x] Fala 0: rozliczenie dokumentacji (#571) + re-bless goldenów po falach #563–#570 (#572 —
      nawrót choroby „kafle studia blessowane, look-goldeny nie"; przegląd klatka-po-klatce,
      jedna zapadka re-derywowana jawnie)
- [x] P1: pomiar-atrybucja + bloki ablacji + wycena @4× (#573 przyrząd, sekcja „Pomiar P1"
      wyżej; ranking diety przestawiony anomaliami A/B)
- [x] P2: hero-first draw order — per-scene `vehicles_first` (garaż true, bitwa false =
      bitowo shipowany porządek). POMIAR: garaż GPU 18,88 → 16,85 ms (−2,0), @4× 24,80 →
      19,82 (dopłata 4× +5,9 → +3,0); goldeny bajt-w-bajt w obu scope'ach. Sygnał regresji
      bitwy przy globalnym reorderze (+0,8 na delcie floty) okazał się W GRANICACH szumu
      między przebiegami — flaga wybrana dla bitowej zamrożoności bitwy; rozstrzygnięcie
      globalnego reorderu wymaga A/B bitwy w JEDNEJ rotacji (follow-up, nieblokujący)
- [WSTRZYMANE — OKO USERA] P3: dieta SSAO garażu. Zmierzone DWA warianty (cap promienia
      24 px; oraz same tapy 12→8 przy pełnym promieniu) — oba przestrajają masę cienia
      pokoju widocznie (52–76% pikseli garażu, delta do 34/255; narożniki i pas pod galerią
      jaśnieją). Zamki wartości TRZYMAJĄ (6/6), ale ten cień strojił user okiem przy relighcie
      #554 — metryka to nie werdykt looku. Kandydat (łagodniejszy wariant) czeka w draft
      PR #578 bez blessu goldenów; zysk ~0,3–0,5 ms GPU z łańcucha 1,26. Werdykt „nie" →
      wpis „odrzucone okiem" i koniec tematu
- [x] P4: jądro półcienia 8 → 6 tapów (zewnętrzny krzyż zostaje, wewnętrzny krzyż → para
      pod 45°; rotacja per piksel = reguła anty-tkaninowa bez zmian; promień 9 texeli i zamek
      ≥8 nietknięte). Przegląd okiem hero + susp_close: nierozróżnialne; goldeny garażu
      scoped re-record; zamki wartości 6/6. POMIAR w rotacji: koszt cieni (delta no-shadows,
      GPU) ~2,35 → ~0,91 ms; ścieżka bitewna nieruszona (dryf zero klatek bitewnych)
- [x] P5: ziarno wnętrz na jednej oktawie — `interior_grain` (drobna oktawa `ground_grain`
      SAMA; szeroka liczyła się i szła do kosza, bo bend czyta tylko gradient drobnej).
      Bend bitowo identyczny — goldeny garażu bajt-w-bajt (zamek). POMIAR: cena flagi C1
      w rotacji −1,0 → −0,26 ms (odzysk ~0,7); absoluty między przebiegami ±1 ms, dowód
      w delcie-delt. Korekta założeń planu: wnętrzowa gałąź `material_detail` prawie nie
      działa w hali (każda powierzchnia NAZYWA materiał → `surface_treatment`), więc
      „unifikacja wartości" z pierwotnego szkicu nie miała czego kupić
- [ ] P6 (warunkowy): depth prepass garażu — po pomiarach PRZEPROJEKTOWANY: wariant 1×
      (reuse głębi ssao_prepass) kupuje budżetowi NIC (1× jest przy budżecie), a F2 wymaga
      wariantu @4× (osobny depth-only pass w 4×, nowy PassId) z ryzykiem dziur ULP — żaden
      shader nie ma `@invariant` na pozycji; przed budową trzeba go dodać do depth-only
      i scene/vehicle/ground i dowieść bajtami. To JEDYNY pozostały lever w skali luki F2
- [x] P7: bramka F2-GO ZAPISANA 2026-08-15 — **NO-GO na dziś.** Stan po diecie P2+P4+P5
      (rotacje interleaved, MX330): 1× GPU-instrument ~17,2 ms (wall−fence 18,0–20,7 —
      dwie miary się rozjeżdżają o ~3 ms, obie protokołowane) vs budżet 16,67; kandydat
      @4× GPU ~20,2 → **luka ~3,5 ms**. Dostępne dźwignie: P3 za okiem usera (~0,4)
      i P6b prepass @4× (projekt powyżej). F2 POZOSTAJE ZAPARKOWANE z ceną — one-look
      zabrania i klatki pod 60 FPS, i opcji jakości; wraca po P6b albo po decyzji
- [ ] P8: F2 4×MSAA per-scene + parytet goldenów (G9) — za bramką P7 (dziś NO-GO)
- [ ] P9: decyzja D2 pomiarem — automatycznie odroczona razem z F2 (bramka D2 liczy od @4×)
- [x] R1: legenda mm inspektora (#580) — pasek pod pasmem nameplate'u, swatche PRÓBKOWANE
      z `color_for_mm` na kotwicach gradientu (10/40/90/150/230), zamek
      `the_legend_is_the_scale_it_explains` + zamek pasma; harness wiesza tę samą legendę
      nad widokiem inspektora, golden przegrany scoped (0,8% pikseli)
- [x] R2: dźwięk pracy naprawy (ten PR) — głos `RatchetWork` w audio crate (klucz nasadowy:
      3 pociągnięcia z przyspieszającą zapadką i jaśniejszym tikiem osadzenia, deterministyczny
      per seed), event `RepairWork { seconds }` spięty z `REPAIR_BEAT_S` (jedno źródło beatu
      dla dźwięku, podnośnika i nameplate'u); zamki: praca słyszalna nad bedem w każdej
      tercji beatu i KOŃCZY SIĘ z beatem (finishing clunk ma ostatnie słowo)
- [x] R3: mechanik przy naprawie (ten PR) — `WorkCue` w hangar_mechanic: na czas beatu
      mechanik schodzi z rundy, staje twarzą do hero i podchodzi 1,2 m ku KRAWĘDZI ringu
      (stopa ≥8,55 m — zamek ringu 8 m nietknięty, nowy zamek próbkuje pełny beat), pracuje,
      wraca symetrycznie; zegar rundy PAUZUJE na beat (garaż akumuluje pauzę), więc koniec
      beatu = bitowo punkt rundy — zero snapu (zamek bit-for-bit). Goldeny bajt-w-bajt
      (mroźna sekunda nie niesie cue). Klucz (R2), podnośnik, nameplate i mechanik odpowiadają
      JEDNEMU `REPAIR_BEAT_S`
- [ ] R4: prawo faz w hali (G7)
- [ ] R5: B2 GI do celu albo zamknięcie pomiarem
- [x] W1: proficiency = 1,0 (ten PR) — pas w game_core zwinięty do pinu (MIN==MAX==1,0,
      default 1,0; formuła kary zostaje jako szew przyszłego systemu załogi, dowodnie martwa
      zamkiem `the_crew_is_pinned_fully_trained`); suwak, `GarageHit::CrewProf`, klawisze −/=
      i rects usunięte; stare save'y migrują W GÓRĘ przez clamp (zamek); kolumna CREW zostaje
      jako prezentacja ról; goldeny garage_screen/option_list przegrane scoped (0,23%)
- [ ] W2: ekran wyników bitwy
- [ ] W3: lokalna historia bitew
- [ ] W4: ekran SETTINGS
- [ ] W5: font PL (Latin-2)
- [ ] W6: paleta przycisku BATTLE
