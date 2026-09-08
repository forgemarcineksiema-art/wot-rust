# Pamięć i Q8c — 2026-09-08

## Pamięć: atrybucja przed cięciem

Punkt wyjścia właściciela: Bystra, jazda, working set 1,08 GB,
private 1,83 GB, GPU 873 MiB. To trzy różne liczniki; nie należy ich sumować.
Working set opisuje aktualnie rezydentne strony, private obejmuje prywatne
zobowiązanie pamięci procesu. Licznik GPU z `nvidia-smi` nie jest księgą alokacji
samego klienta. Poniżej rozmiary **payloadów wynikające z kodu**, bez narzutu
alokatora i sterownika, a nie próba przypisania każdej strony procesu do obiektu.

| Składnik flory | CPU przed | CPU po | GPU przed i po |
| --- | ---: | ---: | ---: |
| Atlas koloru i normalnych, 2 × 4096² RGBA8, pełne mipmapy | 170,667 MiB | 0 | 170,667 MiB |
| Tablice kory, 6 warstw × 2 × 1024×2048 RGBA8, pełne mipmapy | 128,000 MiB | 0 | 128,000 MiB |
| Cache zdekodowanych źródeł: 4 gatunki × pary klastrów, impostorów i kory | 128,000 MiB | 128,000 MiB | — |
| **Suma powyższych** | **426,667 MiB** | **128,000 MiB** | **298,667 MiB** |

Pierwsze dwa wiersze to kopie zachowywane przez `FoliageAtlas`, niezależne od
krótkotrwałych buforów przekazanych przez klienta. Rozmiary dokładne:
178 956 968 + 134 217 744 = **313 174 712 bajtów usuniętych stałych payloadów CPU**.
Wycofana sosna i wierzba nadal mają warstwy kory (kopie dębu); cache źródła dębu
jest wspólny. Nie zmieniono numeracji warstw ani materiałów.

Źródła: `scene_build/src/foliage_atlas_paint.rs`,
`world_forge/src/tree/{leaf_atlas,authored}.rs`,
`renderer_wgpu/src/scene_renderer/foliage_atlas.rs`.
Stare „820 MB flory” nie jest aktualną atrybucją tych tekstur. Poza tabelą pozostają
m.in. geometria, mapy i ich kopie, bufory klatek, cache geometrii, dane osadzone
w obrazie programu, sterownik oraz wolne strony zachowane przez alokator.

## Cięcie

`FoliageAtlas` zachowuje widoki tekstur i samplery GPU. Ustawienie kory przesyła
wyłącznie korę i składa bind group z istniejącymi liśćmi; ustawienie liści zachowuje
już przesłaną korę. Odpadają stałe kopie CPU oraz ponowny upload całego atlasu
przy ustawieniu kory. Teksele, formaty, mipmapy, samplery i shadery pozostają identyczne.
Log `RUST_LOG=info` podaje `texture memory`, kategorię, `gpu_payload_bytes`
i `retained_cpu_payload_bytes` przy ustawieniu tekstur. Są to rozmiary danych,
nie pomiar rezydencji sterownika.

## Q8c

`HeightMap` ma leniwą piramidę maksimów niezmiennych próbek (kolejne poziomy 2×2).
Klon mapy dzieli ten cache; serializacja i porównanie mapy pomijają go. Cache
kosztuje około 1/3 rozmiaru próbek, bez powielania poziomu bazowego.

Przed dokładnym marszem sprawdzane jest maksimum nad prostokątem odcinka.
Niepewny długi odcinek można podzielić na połówki. Dopiero dowód, że wszystkie
części leżą ponad maksimum z zapasem na zaokrąglenia, omija marsz. Każda
niepewność wraca do starego kernela. Promienie przy krawędziach, styczne, teren
z rantem krateru i zmiennoprzecinkowe przypadki brzegowe nie otrzymują
przybliżonej decyzji. Nie zmienia to kroku LOS ani geometrii widoczności.

Test porównuje wynik z niezmienionym marszem dla 24 000 przypadków (różne
wymiary, również niepotęgowe, promienie poza mapą, kratery i trzy luzy), osobno
sprawdza styczność, odcinek zerowej długości i tożsamość serializacji przed/po zbudowaniu cache.
Test GPU sprawdza zachowanie uchwytów liści przy zmianie kory i odwrotnie.

## Walidacja

Testy jednostkowe terenu: 36/36. Preflight: zielony (81 testów quality).
`verify-pr.ps1 -Crates terrain,renderer_wgpu,sim,battle_host`: zielony;
clippy całego workspace'u z `-D warnings`, testy quality/tools i wskazanych
crate'ów, w tym replaye, długie bitwy botów i test zachowania tekstur GPU.
Build release klienta i istniejącego przykładu `tick_sections`: zielony.
Właściciel 2026-09-08
zrezygnował z dodatkowych przejazdów A/B i zlecił commit, push, PR i merge.
Nie przypisujemy tej zmianie zmierzonego spadku working set/private ani konkretnej
oszczędności ms na tik. Liczba usuniętych bajtów opisuje własność danych w kodzie.

N9 nadal wymaga drugiego systemu. Ten zakres nie instaluje WSL i nie zamyka N9.

## Co dalej: płynność całej gry

Poniższe rozpoznanie wynika z kodu i pomiarów zapisanych wcześniej w `program.md`,
nie z nowej sesji pomiarowej. Rozdzielamy stały koszt klatki, pojedyncze zacięcia,
opóźnienie sterowania i oczekiwanie na bitwę.

| Priorytet | Miejsce | Dowód i następny konkretny krok |
| --- | --- | --- |
| 1 — zacięcia po strzale | Zniszczenia statyków | `app/render.rs::rebuild_cover_scene_if_dirty` odbiera zmienione fragmenty z workera, ale na głównym wątku ponownie składa wszystkie `statics_buckets` i wywołuje `set_terrain` dla całości. Dokończyć fragmentaryczność aż do GPU: trwałe zasoby per fragment i podmiana tylko zmienionych. To także usuwa potrzebę trzymania pełnej kopii scalonej obok fragmentów. |
| 1 — responsywność garażu i ładowania | Przygotowanie pojazdów i odbiór mapy | `render_garage` wywołuje `prebake_next_playable_vehicle` na wątku klatki; „jeden pojazd na klatkę” nie ogranicza czasu tej klatki. `take_prebaked_world` używa blokującego `recv`, a brak gotowego świata kończy się bake'em w miejscu. Zastąpić to jawnym stanem ładowania z nieblokującym odbiorem, pracą CPU poza wątkiem okna i budżetowanym uploadem. Pierwszeństwo mają pojazd gracza i skład bieżącej bitwy; reszta katalogu nie powinna wydłużać pierwszej gry. |
| 2 — FPS w walce | Bliskie podwozie i wnętrze | Zapisany zimny pomiar Q9: flota 15v15 podnosi klatkę GPU z ok. 15,6 do 19,9 ms; bliskie podwozie kosztuje ok. 2,5× dalekie na kadłub. Rozdzielić geometrię, materiał i drugi przebieg wnętrza, usunąć niepotrzebną pracę bez zmiany sylwetki i gąsienic. Samo ograniczenie botów nie usuwa tego kosztu GPU. |
| 2 — koszt hosta | Widoczność, AI i fizyka 30 kadłubów | Liczba potencjalnych par PRZECIWNIKÓW rośnie z 98 w 7v7 do 450 w 15v15 (4,59×), zanim zadziała zasięg i przeszkody. Sojusznicy w `compute_observer_masks` omijają LOS — opis „30×29 marszów” zawyża faktyczną liczbę. Q8c dotyczy terenu, nie całego hosta. Następnie mierzyć koszt wyboru kandydatów i spatial broadphase przeszkód; reuse masek tylko dla tej samej wersji stanu. |
| 3 — długie bitwy | Ostrzał, kurz, dym, kratery i ślady | Oddzielić pociski w locie od trafień i przebudowy obrazu. Teren już wysyła łaty, ale zmiana łąki może nadal wymienić całą siatkę `dressing`. Kilka trafień naraz może skumulować upload, efekty i pracę workera. Potrzebny scenariusz salwy oraz dojrzałej bitwy, pomiar p99 i najgorszej klatki; potem lokalne aktualizacje łąki i ograniczenie kosztu przezroczystych warstw. |
| 3 — stabilny rytm i sterowanie | Główny wątek klienta | `loop_step` wykonuje lokalnego hosta i predykcję przed rysowaniem. Gdy klatka trwa dłużej, nadrabia więcej tików, co dodatkowo wydłuża następną klatkę. Osobny worker hosta może poprawić rytm, ale nie zmniejsza kosztu obliczeń i wymaga kontroli opóźnienia wejścia, kolejki snapshotów i deterministycznego czasu. Najpierw tańsze usunięcie pracy i blokad. |

AI nie przelicza wszystkich kosztownych decyzji co tik: wybór celu ma kadencję
6 tików, przejęcie celu i rozwiązanie celowania po 3, rozłożone przez ID kadłuba
(`battle_host/src/bots.rs`). Dalsze obniżanie częstotliwości bez pomiaru mogłoby
pogorszyć reakcje botów. W 15v15 trzeba osobno wycenić obecność kadłubów,
ich ruch, ich mózgi, strzelanie i widoczność na ekranie.

Mapa ma już spekulacyjny bake w tle (`app/mod.rs::bake_world_for`), więc propozycja
„przenieść ładowanie mapy do wątku” sama nie rozwiązuje problemu. Pozostały odbiór
pracy, zimne cache, przygotowanie całego katalogu i upload. Docelowe czasy:
od uruchomienia do reagującego menu, od kliknięcia BITWA do pierwszej sterowalnej
klatki, a potem p50/p95/p99, liczba klatek >25/>50/>100 ms i opóźnienie wejścia.
Średni FPS nie opisuje tych czterech doświadczeń.
