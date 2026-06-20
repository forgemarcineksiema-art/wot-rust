# T‑54 Forge — Plan Etapów 2–4

## Podsumowanie

Cel: zmienić obecny hybrydowy T‑54 ze sprawdzonego spike’a w pierwszy produkcyjny benchmark Forge: parametryczny, semantyczny, wiarygodny wizualnie, bake’owany do artifactu i renderowany przez runtime z pełnym zestawem map PBR-lite.

Etap 1 pozostaje granicą techniczną: kontrakt lufy, jarzma i mesh topology jest już zamknięty. Etapy 2–4 dotyczą wyłącznie T‑54‑3 obr. 1951; pozostałe pojazdy pozostają na aktualnym fallbacku.

## Etap 2 — Jedno źródło parametrów i semantyczny graph części

- Rozszerzyć `VehicleBlueprint` o pełne, autoratywne dane wizualne T‑54: podział kadłuba na płyty, punkty układu jezdnego, wymiary socketu/jarzma, przekroje wieży, deck, błotniki i fittingi.
- Usunąć równoległe stałe z `vehicle_build`, `solid`, `sdf_mesh` i `revolve`; każdy generator ma otrzymywać dane z blueprintu oraz loadoutu.
- Ustanowić `ForgePartGraph` jako wspólny semantyczny opis dla T‑54 i `vehicle_build`: części mają identyfikator, parent frame, materiał, gameplay role, bounds, źródło referencji i politykę LOD.
- Zachować publiczne `t54_description()` i `t54_from_modules()`, ale uczynić je adapterami z blueprintu/part graph do generatorów hybrydowych.
- Dodać API budowania pojedynczych części oraz grup `Hull`, `Turret`, `Gun`, aby później runtime mógł osobno obsługiwać zawieszenie, jarzmo, wyposażenie i uszkodzenia.
- Dodać testy: blueprint jest jedynym źródłem wymiarów; każdy wymagany element T‑54 istnieje w graphie; frames są wyprowadzane z części; zmiana modułu działa bez rozjazdu z mount frames; bounds kadłuba i wieży mieszczą się w gameplay hitboxach.

## Etap 3 — Realne formy T‑54 i proceduralny zestaw operatorów

- Rozbić obecny pojedynczy convex hull na semantyczne płyty: lower tub, upper/lower glacis, sponsony, boki, rufa, engine deck, błotniki oraz osłony gąsienic.
- Rozbudować `solid` o kompozycję płyt z grubością, bevelami i kontrolowanymi seamami; nadal bez ogólnego CSG.
- Rozbudować SDF dla wieży o sekcje odlewu, asymetryczne policzki, zwężony tył, dach, recessed mantlet socket oraz kontrolowany profil dolnej krawędzi.
- Rozbudować `revolve`/track o odrębne road wheels, idler, sprocket, charakterystyczną pierwszą przerwę kół T‑54, segmentowane cues ogniw i czytelny top/bottom run pasa.
- Dodać fittingi T‑54: cupola, włazy, uchwyty, spawy, zaczepy i panele decku — jako osobne semantyczne części, nie anonimowy greeble.
- Wprowadzić LOD0/1/2 per część: LOD0 zachowuje charakterystyczne formy, LOD1 upraszcza fittings i track links, LOD2 zachowuje tylko silhouette/mount frames.
- Dodać jakościowe testy i review: brak degeneratów/manifold breaks, outward winding, geometry budgets, hitbox honesty, charakterystyczny układ jezdny, dokładne kąty pancerza, turret/gun transform chain oraz screenshoty front/profile/top/battle-oblique dla wszystkich LOD.

## Etap 4 — Produkcyjny Forge, artifact, PBR-lite i runtime

- Wprowadzić jedną funkcję źródła bake’a dla pojazdu i profilu LOD. Dla T‑54 ma ona używać hybrydowego part graph/build path; pozostałe pojazdy pozostają na `vehicle_geometry::bake_vehicle`.
- Przepiąć `ForgeArtifact::bake`, ratio report, review images, artifact freshness validation i klientowy fallback na wspólne źródło bake’a, aby Forge i gra nigdy nie renderowały różnych T‑54.
- Zachować deterministyczny source hash, ale oprzeć go na artifactowym bake’u T‑54 oraz pełnym graphie części, blueprint data i profilu LOD.
- Rozszerzyć vertex/artifact format o UV, tangent i material ID dla pojazdów Forge. Nie zmieniać formatu terrain ani prostych `SceneVertex`.
- Bake’ować dla T‑54 wymagane mapy: `albedo.png`, `normal.png`, `ao_roughness.png`, opcjonalnie `cavity.png`; normal map ma wynikać z wysokiej jakości proceduralnej geometrii, a nie z runtime generation.
- Uaktualnić vehicle PBR pipeline i loader artifactów tak, by ładowały oraz walidowały wszystkie mapy, z neutralnym fallbackiem tylko dla brakującego optional cavity.
- Dodać runtime variation bez rekonstrukcji geometrii: tint, camo, dirt/snow, decals, uszkodzenia modułów oraz osobny stan gąsienic/zawieszenia.
- Dodać end-to-end testy: artifact T‑54 przechodzi ratio report, write/read roundtrip zachowuje mesh/mounts/maps/hash, loader akceptuje aktualny artifact i odrzuca stary, renderer rejestruje trzy mount-aware submeshe, screenshot regression przechodzi dla LOD0/1/2.

## Weryfikacja i porządek repozytorium

- Każdy etap pracuje test-first: test kontraktu → potwierdzona porażka → minimalna implementacja → test green.
- Po każdym etapie: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, `cargo check --workspace --all-targets`, `cargo bench --workspace --no-run`.
- Na tej maszynie pełne komendy należy uruchamiać z `-j 1`, jeśli Windows ponownie zgłosi `os error 1455` związany z plikiem stronicowania.
- Każdy etap kończy się jednym celowym commitem, czystym `git status`, push na gałąź `codex/...` i jednym PR-em. Po merge usunąć lokalny worktree, nieużywane gałęzie lokalne oraz zdalne gałęzie PR.
- Przed rozpoczęciem Etapu 2 wykonać GitHub/local audit: potwierdzić, że merge Etapu 1 jest na `master`, brak niezatwierdzonych zmian i nie ma osieroconych worktree.

## Założenia

- T‑54 jest jedynym produkcyjnym benchmarkiem tych etapów.
- Docelowy styl to czytelny, stylizowany realizm WoT-beta, nie muzealny scan ani asset DCC/AAA.
- Pełne mapy PBR-lite są wymagane w Etapie 4.
- Generatory pozostają renderer-free; renderer konsumuje wyłącznie bake artifacts.
