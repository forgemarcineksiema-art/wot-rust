# Świat 2.0 — program

**Status: ZATWIERDZONY 2026-08-06 (kierunek + relacja + decyzja o florze).** To jest JEDYNY
program świata: konsoliduje „Świat w Skali" (plik wygaszony), domyka defekty geometrii
z bliska (art-direction D6/D19), dodaje moc autorską blueprintu i — decyzją z tej samej
sesji — **wyprowadza z gry florę importowaną**. Świat jest w całości proceduralny.

## Werdykt otwierający i główne decyzje

Werdykt usera (2026-08-03, przejęty ze „Świata w Skali"): „Czołg jest okej, ale budynki,
drzewa, obiekty, nawet rzeka, daleki obraz, no taki ogólny odbiór — są za małe, takie
same, no trzeba świat zrobić lepszy."

Decyzje nadrzędne, od których program się wywodzi:

1. **Jedno drzwiowe hasło: procedural-only.** (2026-08-06): „Florę CC0 usuwamy. Tylko
   procedural. Drzewa z Blendera out." Zero assetów importowanych, zero glTF/`.flora.*`,
   zero prowieniencji licencyjnej do pilnowania. Jedyny język roślinności to generatory
   `world_forge::tree` + karty trawy.
2. **Skala najpierw, kamera druga, werdykty wizualne dopiero po kamerze.** Rejestr
   odchyleń poniżej jest zmierzony; kamera (FOV) przesuwa punkt widzenia wszystkich
   późniejszych werdyktów, więc zapada wcześnie — probe przed/po, decyzja okiem.
3. **Uczciwość nie ruszona:** wierzchołki ⊆ AABB kolizji, sceneria render-only, pnie
   drzew jako gameplay solid zostają (przekierowane na drzewa proceduralne).

## Stan zastany (zmierzony 2026-08-06, rekonesans)

- Fasady: ściana to jeden lity `push_box`, okna to wystające tafle `WindowGlass` (~4 cm),
  cokół hardcode `[0.24, 0.22, 0.20]` + rola `LEGACY` (`world_forge/src/building.rs`,
  `scene_build/src/battlefield.rs`). Tenement ~366 tris przy locku 30–400; polityka już
  sankcjonuje ≤600 dla Tenement/FactoryHall.
- TreeLine/Wreck/RailCover = pojedynczy vertex-colored AABB (`push_surfaced_box`); skała
  to jeden obrócony box 12 tris (`foliage.rs`). Wzorzec „dobrej" konstrukcji istnieje:
  WoodenFence/StoneWall (multi-box w AABB, locki uczciwości).
- Skala: drzewa 7–10 m (realnie 20–35), horyzont 15 m
  (realnie 60–200), wraki +90% ZA DUŻE, zero wariancji instancji. Kamera już docelowa:
  FOV 48° pion (PR 2, 2026-08-07).
- Perf: Ostrogorsk z pełną florą ~438k wierzchołków statics, klatka pod 16.7 ms z małym
  zapasem — każda geometria WYMIENIA lub mierzy, nigdy „dokłada po cichu".

## Rejestr odchyleń — top 10 (przejęty ze „Świata w Skali", aktualizacja: drzewa już 1:1)

| # | Defekt | Stan wejścia programu |
|---|---|---|
| 1 | Drzewa za małe | **JUICZ** — trees-to-scale pass (2026-08-03) dał dojrzałe wysokości; program tylko pilnuje, by drzewa się nie kurczyły z powrotem (locki wysokości stoją) |
| 2 | Kamera 62° pionowo | **JUICZ** (2026-08-07): FOV 48° pion; probe przed/po na 4 mapach, werdykt okiem usera: „zostaw na 48" |
| 3 | Horyzont ~15 m; Prochorowka bez horyzontu | ściana doliny 60–200 m w realu |
| 4 | Gęstość flory 64–190 drzew/km²; 180 drzew backdropu | dolina niesie tysiące |
| 5 | Landmarki: kościoły 13–14 m, wiatrak 6 m, silosy 11 m | realnie 25–35 / 12–18 / 20–30 |
| 6 | Pasy TreeLine 8,4–10 m | realnie 15–25 m |
| 7 | Zero wariancji instancji (Row/Fixed jeden scale+yaw, budynki 0°/90°) | klony niszczą odbiór skali |
| 8 | Teren: cell 5 m, relief ±1,3 m | NIE densyfikować — rzeźbić autorsko |
| 9 | Mgła 0.00013 → 12% zaniku na 1 km | daleki plan w pełnym kontraście = płasko |
| 10 | Wraki 6,8×3,2×12,4 m | **JUICZ** (bless 2026-08-03): wraki mają już footprint ~3,5×2,2×6,5 m; program dokłada im SYLWETKĘ (kadłub+wieża+lufa w AABB), nie wymiary |

## Flora: co „procedural-only" znaczy w kodzie (realizowane w tej samej zmianie)

- `FloraTree` na 4 shipped mapach staje się **proceduralnym `Oak`** (ten sam seed, region,
  wykluczenia, ta sama liczba instancji — to dressing swap, nie zmiana gęstości).
- `hero_oak_trunk_cover` (map_forge) zostaje — przekierowany na `SceneryKind::Oak`:
  proceduralny dąb ma pień jako gameplay solid (`TreeTrunk`, 240 hp, crushable). Dąb
  proceduralny jest realną geometrią (`world_forge::tree::bake_tree`), nie malowidłem.
- `assets/flora/*` (7 plików, ~7,1 MB) znika; `import-flora` w `tools` znika;
  `world_forge::flora` znika. `SceneryKind::FloraTree/FloraPine/FloraBush` zostają jako
  wygaszone warianty (append-only, nigdy nie autorstwione — lock w `flora_integration`).
- Program „Rura Blender–silnik 2.0" jest **porzucony w całości** (decyzja 2026-08-07):
  plan skasowany. Ewentualne resztki (packer półkowy, budżety) wróciłyby wyłącznie jako
  proceduralne LOD, gdyby kiedyś były potrzebne.

## Fale PR (kolejność wg dźwigni obrazu; każdy PR z lockami i pomiarem)

### Fala 0 — Flora: procedural-only (**LANDED 2026-08-06**)
**PR F0** — usunięcie importowanej flory: mapy (FloraTree→Oak, bless goldenów), runtime
(compile/trunk cover, tree_lod retarget na proceduralny dąb, foliage cleanup), tools
(import-flora out), probes (flora_probe/flora_frame_probe retarget na gatunki
proceduralne), assets/flora delete, notices, docs.

### Fala 1 — Skala (oko dostaje linijkę; kamera przed werdyktami wizualnymi)
- **PR 1 — Drzewa 1:1.** (**LANDED 2026-08-06/07**): mature heights + golden hashes already
  from 2026-08-03; F0 removed CC0 flora; this PR locks Mid LOD tip = Close tip (RNG sync +
  Y lift), bole-scale pniaki/logs on felled TreeLine/TreeTrunk, and far-frustum mature floors.
- **PR 2 — Kamera.** (**LANDED 2026-08-07**): FOV 62°→48° pion; `fov_probe` wyrenderował
  przed/po (62/55/48) na 4 mapach; werdykt okiem usera: **48° zostaje**. Grass-zoom
  reference przeszedł na cot(48°/2)=2.2461 — zaokrąglone W GÓRĘ, bo clamp podpiera od 1.0
  i obcięta wartość każe spoczynkowej kamerze czytać się jako powiększona (battle camera =
  skala 1.0; wejście lunety spadło przez to z ~3.3× na ~2.81×); look goldens
  świadomie zostają na stabilnym 55° — werdykt FOV idzie przez pary z `fov_probe`.

### Fala 2 — Cover 2.0 (D6 + D19 + W5)
- **PR 3 — Fasady Tenement/FactoryHall.** (**LANDED 2026-08-07**): prawdziwe otwory w
  płytkich liściach muru (szyba wpuszczona 9 cm w mur, kamienna rama + krzyż szprosów,
  parapet), pas nadproży z kamienia, gzymsy kondygnacji, lizeny narożne, kornisz,
  podwójny portal wejściowy; hala: pilastry na osi przęseł, portal wozowy w szczycie,
  klerstory ze stalowym rytmem szprosów. Cokół z palety (`stone_palette`), nowa rola
  `DRESSED_STONE` (8.0) z własnym połom w `scene.wgsl` (poziome kłady ciosu). Budżet
  400→**1500** (decyzja usera: mocno podnieść): Tenement 274→1290, FactoryHall 122→752
  tris. Pomiar Ostrogorsk (MX330, 1080p): statics 82 398→202 838 wierzchołków (+146 %),
  klatka full-flora ~20,8 ms — przed/po wewnątrz szumu termicznego maszyny; gate
  16,667 ms FAIL to stan wcześniejszy, nie z tego PR. Bless goldenów budynków; lock:
  `urban_glass_is_recessed_into_a_pierced_leaf`.
- **PR 4 — Fasady wiejskie.** (**LANDED 2026-08-07**): Cottage/Townhouse/Church — prawdziwe
  otwory, szyby wpuszczone w mur, parapety; drzwi WCIĘTE w liść (ścianka dzielona na biegi),
  nie przyklejone. Obróbka wg tradycji: drewno (cottage/barn — okiennice zamiast szkła w
  stodole, portal wozowy w OBU szczytach) / kamień (townhouse/church — prezbiterium z
  przyporami, dzwonnica z PRAWIDZIWYMI otworami: 4 słupy narożne + żaluzje). Budżety per
  styl: Cottage 320 i Barn 310 mieszczą się we wspólnym 400; Townhouse 644→800,
  Church 464→600. Lock: `village_glass_is_recessed_and_the_barn_has_none`.
- **PR 5 — TreeLine 2.0.** Boxy LOS do 15–25 m RAZEM z geometrią szpaleru (rzędy pni +
  podszyt w AABB); dowód AABB przed/po; jeden świadomy bless map.
- **PR 6 — Wraki 2.0.** Generator sylwetki (kadłub+wieża+lufa w AABB) + naprawa
  pomylonych osi wraka fabrycznego Ostrogorska; bless.
- **PR 7 — RailCover 2.0.** Geometria rewetmentu/nasypu w AABB.
- **PR 8 — Skały i drobnica.** Nowy `world_forge::rock` (deterministyczny, niski budżet)
  + bogatszy DebrisHeap; `Rock` przestaje być pudłem.
- **PR 9 — Landmarki pion (W5).** Kościoły 25–35, wiatrak 12–18, silosy 20–30, dłuższa
  hala, latarnie 6–9 m; bless.

### Fala 3 — Horyzont i głębia (W3 + W6)
- **PR 10 — Horyzont.** Wzgórza ×3–5; Prochorowka stepowy horyzont; backdrop setki drzew.
- **PR 11 — Perspektywa powietrzna.** Mgła wysokościowa POD capem fairness 0.35@400 m.
- **PR 12 — Rzeka i mikrorelief.** Dolina Bystrej, szerokość rzeki za granicą, autorski
  mikrorelief tam gdzie walka (NIE densyfikacja).

### Fala 4 — Moc autorska (blueprint 2.0, wszystko addytywnie)
- **PR 13 — Wariancja instancji (W4).** Jitter scale/yaw w Row/Fixed, lustro z własną
  skalą, rotacje budynków poza 0°/90°; bless.
- **PR 14 — TownGrid 2.0.** Trzecia forma, per-komórkowy seed, puste działki, born-ruins
  jako dane siatki.
- **PR 15 — `ObjectSpec::District`.** Generator kwartału/wioski: świadomy ulic, miesza
  kinds, deterministyczna ekspansja do coverów; checki raportu; RON-only najpierw.
- **PR 16 — Scatter 2.0.** Pas wzdłuż polilinii (drogi/rzeka), gęstość na powierzchnię,
  region wielokątny; nowe warianty `SceneryOp` addytywnie.
- **PR 17 — Edytor.** Paleta O stawia TownGrid/District, inspektor, delete.

### Fala 5 — Biom i odbiór
- **PR 18 — Biom jako dane.** Doktryna w map-forge-policy: biom = paleta 4 warstw +
  zestaw flory + looki + horyzont; presety step/alpejski; decyzja o śniegu (5. kanał
  splat, poza programem). D12 (krzak) domyka się **proceduralnie**: `TreeSpecies::Bush`
  zostaje jedynym krzakiem — bez CC0.
- **PR 19 — Mapa odbiorowa.** Nowa mapa-wioska z District + Scatter 2.0 + wariancji;
  append `MapId`, blueprint, golden, dossier — dowód, że nowy słownik skraca autorstwo.

## Bramka każdego PR

- Uczciwość: żaden wierzchołek poza AABB kolizji; zmiana boxa LOS = bless goldenów.
- One-look MX330@60: wizualne PR-y z liczbami `perf_capture` / `flora_frame_probe` przed/po.
- Blueprint addytywnie: `serde(default)`, append wariantów, `BLUEPRINT_VERSION` zostaje 1.
- `map_forge` renderer-free; każda liczba z lockiem; `verify.ps1` etapami.

## Test odbioru programu

1. Cztery istniejące mapy przechodzą werdykt okiem w docelowej kamerze.
2. `perf_capture` na Ostrogorsku (po przejściu na proceduralną florę) trzyma < 16.667 ms.
3. Mapa odbiorowa istnieje z goldenem i dossier.
4. Locki uczciwości zielone; `docs/world-scale-program.md` nie istnieje; D6 i D19
   zamknięte; świat jest w całości proceduralny — żadnego bajtu importowanej flory.
