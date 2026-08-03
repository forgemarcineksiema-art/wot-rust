# Świat w Skali — program

**Status: ZATWIERDZONY kierunkowo przez usera (2026-08-03), decyzje skali per typ czekają na
werdykt okiem.** Werdykt otwierający (user): „Czołg jest okej, ale budynki, drzewa, obiekty,
nawet rzeka, daleki obraz, no taki ogólny odbiór — są za małe, takie same, no trzeba świat
zrobić lepszy."

Pomiar z 2026-08-03 potwierdza werdykt liczbami. Wzorzec kalibracji: T-54 z blueprintu —
3,5 m szer. × 2,53 m wys. × 6,54 m kadłuba (`assets/vehicles/t54_1951.vehicle.json`). Czołg
jest 1:1; świat wokół niego jest systematycznie za mały o 25–75%, pozbawiony wariancji
per instancję i pozbawiony głębi dalekiego planu.

## Mechanika, którą trzeba znać przed czytaniem liczb

Rysowany rozmiar budynku to NIE tabela stylów `world_forge` — bake jest skalowany per oś do
autorskiego AABB kolizji z blueprintu mapy (`crates/world/scene_build/src/battlefield.rs:816-822`),
a `crates/foundation/terrain/src/map_build.rs:73` stawia środek na gruncie, więc
`half_extents_m[1] × 2 = realna wysokość w metrach`. Styl daje proporcje; blueprint daje rozmiar.
Honesty doctrine działa na naszą korzyść: podniesienie jednej liczby w `*.map.ron` podnosi
obraz i kolizję RAZEM — ale właśnie dlatego każda zmiana skali jest też zmianą gameplayu
(LOS/cover) i wymaga świadomego blessu goldenów map (v35 `map_content_hash`).

## Rejestr odchyleń — top 10 wg widoczności w grze

| # | Defekt | Zmierzone | Referencja realna | Odchyłka |
|---|---|---|---|---|
| 1 | **Drzewa — wszystkie gatunki**: Pine ~7,5 m, Poplar ~9, Oak ~8,5, Willow ~8, FloraTree 7,26, FloraPine 7,38 (`world_forge/src/tree.rs:38-133`, `assets/flora/*.flora.json`) | 7–10 m | sosna 25–35, dąb 20–30, topola 25–35, wierzba 15–25 | **−60…−75% — najgorsza liczba świata** |
| 2 | **Kamera**: 62° pionowego FOV (~95° poziomo), oko 7,6 m nad ziemią, 24° w dół (`client/src/camera/types.rs:30-49`) | 62° | 30–45° dla third-person pojazdu | ~1,5–1,7× za szeroko; diorama |
| 3 | **Horyzont**: wzgórza zamykające ~15 m nad dnem doliny (`bystra-valley.map.ron:29`, `ostrogorsk.map.ron:20`); **Prochorowka nie ma horyzontu wcale** (`scene_build/backdrop.rs:29-31`) | 15–30 m | ściana doliny 60–200 m | **−85% / brak** |
| 4 | **Gęstość flory**: 64–190 drzew/km² na mapach; 180 drzew backdropu na 5,6 km obwodu horyzontu (`scene_build/backdrop.rs:36`) | — | dolina rolnicza niesie tysiące | oko nie ma „linijki" skali na dystansie |
| 5 | **Landmarki**: kościoły 13–14 m (`bystra-valley.map.ron:166`, `ostrogorsk.map.ron:130`), wiatrak 6 m (`bystra-valley.map.ron:201`), wieże silosów 11 m (`ostrogorsk.map.ron:445`) | 6–14 m | kościół z wieżą 25–35, młyn 12–18, silos 20–30 | −50…−60% |
| 6 | **Pasy zadrzewień (TreeLine)**: 8,4–10 m wysokości, 14 obiektów na 4 mapach | 9–10 m | szpaler/pas 15–25 m | −50% |
| 7 | **Zero wariancji instancji**: budynki bez per-instance scale (wymuszony AABB) i tylko rotacje 0°/90° (`battlefield.rs:815-822,830`); `Row`/`Fixed` jeden scale+yaw na CAŁY szereg (`map_forge/compile.rs:230-266`); 48 kamienic z 2 foremek szachownicą (`ostrogorsk.map.ron:537-538`); 24 identyczne latarnie; lustro reużywa scale bliźniaka (`terrain/scenery.rs:117-135`) | — | — | klony wprost niszczą odbiór skali |
| 8 | **Teren**: cell 5 m (czołg = jedna komórka), relief ±1,3 m przy fali 70–85 m | — | — | brak detalu pod-czołgowego; NIE densyfikować — rzeźbić autorsko (measurements.md) |
| 9 | **Mgła/perspektywa powietrzna**: density 0.00013 → 12% zaniku na 1 km (`renderer_api/src/lighting.rs:211`); cap fairness `MAX_FOG_AT_VIEW_RANGE 0.35@400 m` (`scene_build/weather.rs:104-123`) | — | — | daleki plan w pełnym kontraście = płasko |
| 10 | **Wraki**: (3.4, 1.6, 6.2) = 6,8 × 3,2 × 12,4 m, 8 szt. na 4 mapach — **+90% ZA DUŻE** (jedyny błąd w drugą stronę: żywe czołgi wyglądają przy nich jak zabawki); wrak fabryczny z pomylonymi osiami 6,4 szer. × 3,2 dł. (`ostrogorsk.map.ron:211,218`) | 12,4 m dł. | T-54: 6,54 m | +90% |

**Runnery-up:** daleki LOD dębu (5,6 m) NIŻSZY niż bliski (~8,5 m) — drzewa kurczą się z
odległością (`foliage.rs:153-159` vs `tree.rs:38-52`); rzeka bez doliny — spadek terenu 1,0 m
na 60 m (`bystra-valley.map.ron:82-86`); rzeka za granicą 52 m vs ~28 m na mapie
(`backdrop.rs:89`); latarnie 4 m — realne 6–9 (`foliage.rs:197`); trawa urywa się na 48 m
(`grass.rs:23`); hala fabryczna 28 m dł. — realne 60–120 (`ostrogorsk.map.ron:139`); far plane
2000 m obcina 500 m z 1500 m apronu (`projection.rs:18`); pniak po 9-metrowym drzewie 26 cm
(`battlefield.rs:295`).

## Fale PR (kolejność wg dźwigni obrazu)

- **W1 — Drzewa 1:1 (2–3 PR).** Gatunki `tree.rs` do realnych wysokości (sosna ~28 m, dąb ~22,
  topola ~28, wierzba ~18, owocowe zostają); flora CC0 przeskalowana przy imporcie (bramka
  0,3–25 m już na to pozwala — assety siedzą na 30% sufitu); daleki LOD ≥ bliskiego (koniec
  kurczenia); pasy TreeLine w górę RAZEM z boxami LOS (gameplay!); pomiar tri/klatki (one-look
  MX330@60). Locki: wysokość gatunku, LOD nie maleje, bless goldenów map świadomy.
- **W2 — Kamera (1 PR, WERDYKT USERA na probe'ach).** FOV 62°→~45–50° pion; ewentualnie boom
  i wysokość oka. Zmienia odbiór całej gry — probe render przed/po do decyzji okiem.
- **W3 — Horyzont i głębia (2–3 PR).** Wzgórza zamykające ×3–5; horizon dla Prochorowki (step:
  daleki płaski horyzont + szyki drzew na obwodzie); backdrop setki drzew zamiast 180; mgła
  wysokościowa/perspektywa powietrzna projektowana POD capem fairness (0.35@400 m zostaje).
- **W4 — Wariancja instancji (2 PR).** Deterministyczny jitter scale/yaw per instancję w
  `Row`/`Fixed` (seed z pozycji); lustrzany bliźniak z własną skalą; budynki: rotacje spoza
  0°/90° + trzecia forma w TownGrid. Bless goldenów map deliberatny.
- **W5 — Landmarki i pion (2 PR).** Kościoły/wiatrak/silosy/latarnie do realnych wysokości;
  hala fabryczna dłuższa; **wraki do wymiarów floty** i naprawa pomylonych osi.
- **W6 — Rzeka i teren (2 PR).** Realne wcięcie doliny Bystrej (floodplain głębiej niż 1 m);
  spójna szerokość rzeki na/za granicą; autorski mikro-relief tam gdzie walka (Ridge/terrace
  z programu „Ręce do terenu" — NIE densyfikacja).

**Bramka każdej fali:** probe render do przeglądu okiem usera + pomiar klatki + testy lockujące
nowe wymiary. Zasada dokumentów obowiązuje: liczba bez testu to notatka, nie polityka.
