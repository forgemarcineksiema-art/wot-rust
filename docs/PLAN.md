# Armored Vehicle Forge: Długoterminowy Plan Technologii Pojazdów

## Summary

Budujemy **Armored Vehicle Forge**: proceduralny system authoringu i bake assetów, którego celem jest jakość wizualna w duchu **World of Tanks beta**, ale bez ręcznego modelowania każdego pojazdu od zera. Proceduralność ma być źródłem prawdy i narzędziem produkcji modeli, a runtime ma renderować gotowe, zoptymalizowane assety.

Decyzje zablokowane:
- Model pipeline: **procedural source + baked assety + runtime warianty**.
- Pierwszy benchmark jakości: **T-54/T-55 family**.
- Renderer target: **PBR-lite + baked maps**.
- Nazwa technologii: **Armored Vehicle Forge**, nie `VehicleBlueprint v2`.

Realna ambicja: obecny system 3/10 pod cel WoT-beta. Forge ma podnieść go do poziomu, gdzie jeden pojazd benchmarkowy ma rozpoznawalną, photo-backed bryłę, UV, normal/AO/cavity maps, sensowne materiały, LOD-y i screenshot/regression review.

## Docelowa Abstrakcja

Armored Vehicle Forge będzie mieć 6 warstw:

1. **Reference Layer**
   - Zbiera źródła: zdjęcia, rzuty boczne/front/top, dane wymiarowe, notatki interpretacyjne.
   - Dla każdego pojazdu trzyma ratio targets: długość/szerokość/wysokość, track height, turret width, gun protrusion, wheel count, mantlet size.
   - Output: `ReferencePack`, czyli dowód, skąd pochodzą proporcje.

2. **Semantic Vehicle Model**
   - Zastępuje ideę jednego płaskiego `VehicleBlueprint`.
   - Pojazd jest grafem części: hull plates, lower tub, sponsons, fenders, track runs, road wheels, turret shell, turret cheeks, mantlet, gun, cupola, hatches, hooks, welds.
   - Każda część ma: local frame, materiał, gameplay role, source note, LOD policy.

3. **Forge Geometry Kernel**
   - Obecne `extrude/revolve/chamfered_prism` zostają jako fundament, ale trzeba dodać mocniejsze operatory:
     - `PlateBuilder` dla płyt pancernych z grubością, bevelami i normal seams.
     - `LoftBuilder` dla kadłubów, wież i casemate z wielu przekrojów.
     - `CastTurretBuilder` dla asymetrycznych cast turret cheeks.
     - `TrackBeltBuilder` dla prawdziwego pasa gąsienicy.
     - `WheelTrainBuilder` dla układu kół, rolek, idler/drive sprocket.
     - `DetailScatter` dla śrub, włazów, uchwytów, spawów i panel cuts.
   - Kernel ma generować nie tylko vertex positions, ale też UV islands, tangents, material IDs i bake metadata.

4. **Bake Artifact Layer**
   - Forge generuje asset, a nie tylko mesh w pamięci.
   - Docelowy layout:
     - `manifest.json`: pojazd, wariant, LOD-y, materiały, source hash.
     - `meshes.bin`: vertex/index buffers.
     - `albedo.png`, `normal.png`, `ao_roughness.png`, opcjonalnie `cavity.png`.
     - `review/*.png`: front, rear, profile, top, battle-oblique.
   - Na starcie można bake’ować w pamięci, ale format artefaktu ma być zaprojektowany od początku.

5. **PBR-lite Vehicle Renderer**
   - Nie rozszerzać starego `SceneVertex` dla wszystkiego.
   - Dodać osobny vehicle pipeline:
     - `VehicleVertex`: position, normal, tangent, uv, material_id, tint_mask.
     - `VehicleMaterialSet`: albedo, normal, AO/roughness/metalness, tint controls.
     - shader z normal mapping, AO/cavity, roughness specular, sun + sky fill.
   - Terrain i proste scene meshe mogą zostać na obecnym lekkim pipeline.

6. **Runtime Variation Layer**
   - Runtime nie generuje pełnego czołgu.
   - Runtime może dokładać:
     - decals po trafieniach,
     - błoto/kurz/śnieg,
     - camo/team markings,
     - uszkodzone moduły,
     - zerwane gąsienice,
     - wyposażenie opcjonalne.
   - To jest etap po stabilnym baked benchmarku.

## Key Implementation Changes

### 1. Wprowadzić Armored Vehicle Forge jako nowy poziom architektury

- Dodać nowy crate: `crates/vehicle_forge`.
- `vehicle_forge` ma zależeć od `vehicle_geometry`, ale nie od `renderer_wgpu`.
- `vehicle_geometry` zostaje low-level mesh/kernel crate.
- Przepisać dokumentację tak, żeby `VehicleBlueprint` był traktowany jako stary prototyp, nie docelowy model.
- Nowe podstawowe typy:
  - `ReferencePack`
  - `VehicleForgeRecipe`
  - `ForgePartGraph`
  - `ForgePart`
  - `BakeProfile`
  - `ForgeArtifact`
  - `ReviewCameraSet`

### 2. Zbudować T-54/T-55 jako benchmark jakości

Pierwszy cel nie brzmi „wszystkie pojazdy trochę lepsze”. Pierwszy cel brzmi: **T-54/T-55 family wygląda jak prawdziwy pojazd z WoT-beta-like asset pipeline**.

T-54/T-55 benchmark musi mieć:
- 5/6-road-wheel decision jawnie udokumentowaną i testowaną.
- Lower hull tub, upper sponsons, fenders, track run.
- Cast turret z asymetryczną masą frontu/cheeków, nie tylko revolve dome.
- Mantlet socket + moving gun mask.
- Cupola, hatch cues, gun evacuator, barrel taper.
- LOD0 near/garage model: target 8k-18k tris.
- LOD1 battle near: 3k-8k tris.
- LOD2 distance: 800-2k tris.
- Collision/hitbox proxy nadal zgodny z gameplay.

### 3. Rozwinąć kernel proceduralny

Minimalny zestaw operatorów do pierwszego benchmarku:
- `plate_box`: płyty z grubością, bevelami, normal seams.
- `loft_sections`: loft między przekrojami hull/turret/casemate.
- `cast_shell`: wygładzony, organiczny shell wieży z hard mantlet seam.
- `track_belt`: top/bottom run, rounded ends, track shoes.
- `wheel_train`: road wheels, idler, drive sprocket, rollers.
- `uv_unwrap_basic`: stabilne UV per part, atlas slots.
- `tangent_generate`: tangenty dla normal mapping.
- `bake_cavity_ao`: deterministic low-cost AO/cavity approximation.

### 4. Zbudować vehicle PBR-lite renderer

Zmiany publicznych interfejsów:
- Dodać `renderer_api::VehicleVertex`.
- Dodać `renderer_api::VehicleMeshAsset`.
- Dodać `renderer_api::VehicleMaterialDescriptor`.
- Dodać osobny path w `renderer_wgpu`: vehicle pipeline z teksturami i normal map.
- `client` ma dostać `VehicleAssetCatalog`, który ładuje/registruje baked vehicle assets, zamiast traktować wszystko jako zwykły `SceneVertex`.

Renderer acceptance:
- Normal map widocznie łapie turret cheeks, mantlet, welds/panel edges.
- AO/cavity przyciemnia track recess, under-fender, turret ring, mantlet socket.
- Armor tint nadal działa jako warstwa, ale nie niszczy materiału.
- Barrel, rubber, tracks i cast/rolled armor mają różne response w shaderze.

### 5. Dodać Forge tooling

Dodać komendy w `tools`:
- `forge-vehicle --vehicle t54-1951 --profile lod0 --out target/forge/t54_1951`
- `forge-lineup --out target/forge_review`
- `forge-report --vehicle t54-1951 --out target/forge/t54_1951/report.md`

Każdy bake generuje:
- artefakty renderowe,
- screenshoty kontrolne,
- raport proporcji,
- hash źródeł,
- listę przekroczonych budżetów.

### 6. Utrzymać gameplay honesty

Nie wolno dopuścić, żeby ładny model rozjechał gameplay:
- Hull + turret/casemate visual bounds muszą nadal mieścić się w hitbox/turret plan.
- Mantlet/gun może wystawać, ale ma mieć osobny role.
- Mount frames muszą pochodzić z semantycznych części, nie z ręcznie wpisanych magic values.
- Casemate vehicles nadal ignorują turret yaw.
- Renderer pose chain nadal: hull origin -> turret ring -> trunnion -> muzzle.

## Long-Term Milestones

### Milestone 0: Lock The Philosophy

Checklist:
- [ ] Dokument `Armored Vehicle Forge` opisuje model authoring+bake+runtime variation.
- [ ] Stary `VehicleBlueprint` opisany jako prototypowy stepping stone.
- [ ] T-54/T-55 family wybrana jako quality benchmark.
- [ ] Obecne screenshoty zachowane jako baseline porównawczy.

Acceptance:
- Każdy kolejny task można ocenić pytaniem: „czy przybliża T-54/T-55 do Forge benchmarku?”

### Milestone 1: Reference Pack And Ratio Tests

Checklist:
- [ ] Utworzyć `ReferencePack` dla T-54/T-55.
- [ ] Dodać photo-derived ratio tests.
- [ ] Testować: road wheel count, hull length/height, track height, turret width, turret height, gun protrusion, cupola position.
- [ ] Raportować różnice procentowe, nie tylko pass/fail.

Acceptance:
- Testy potrafią powiedzieć: „ten model jest proporcjonalnie zły”, nawet jeśli mesh jest technicznie poprawny.

### Milestone 2: Semantic Part Graph

Checklist:
- [ ] Dodać `ForgePartGraph`.
- [ ] Przenieść T-54/T-55 z płaskich constants do części: hull, track, wheels, turret, mantlet, gun, fittings.
- [ ] Każda część ma bounds, material role, local frame, source note.
- [ ] Mount frames wynikają z grafu części.

Acceptance:
- Można wygenerować raport: które części pojazdu istnieją, skąd mają proporcje i do jakiego gameplay role należą.

### Milestone 3: Geometry Operators For Real Tank Forms

Checklist:
- [ ] Plate builder z grubością i bevels.
- [ ] Multi-section loft dla kadłuba.
- [ ] Cast turret shell dla T-54/T-55.
- [ ] Track belt + real wheel train.
- [ ] Basic fittings: cupola, hatches, handles, exhaust/fuel tank cues.
- [ ] UV unwrap i tangent generation.

Acceptance:
- T-54/T-55 przestaje wyglądać jak „low-poly approximation”, zaczyna czytać się jako konkretny model pojazdu z referencji.

### Milestone 4: PBR-lite Vehicle Pipeline

Checklist:
- [ ] Dodać `VehicleVertex`.
- [ ] Dodać vehicle material textures.
- [ ] Dodać normal mapping.
- [ ] Dodać AO/roughness map.
- [ ] Dodać vehicle-specific shader path.
- [ ] Dodać screenshot regression z tym shaderem.

Acceptance:
- Ten sam mesh bez normal/AO wygląda wyraźnie gorzej niż z bake maps.
- Track recess, turret ring, mantlet socket i plate seams są czytelne bez dodawania absurdalnej liczby trójkątów.

### Milestone 5: Bake Artifact And Toolchain

Checklist:
- [ ] Forge CLI generuje artifact folder.
- [ ] Client potrafi załadować baked artifact.
- [ ] Startup path używa baked asset, a nie procedural build w każdej sesji.
- [ ] Debug path nadal potrafi bake’ować bezpośrednio z recipe.
- [ ] Hash artefaktu wykrywa zmianę źródła lub generatora.

Acceptance:
- Pojazd można wygenerować, sprawdzić, zapisać i renderować bez ręcznej ingerencji.

### Milestone 6: First Production Benchmark

Checklist:
- [ ] T-54/T-55 family ma LOD0/LOD1/LOD2.
- [ ] Screenshoty: front, rear, left/right profile, top, battle-oblique.
- [ ] Ratio report przechodzi.
- [ ] Geometry tests przechodzą.
- [ ] Renderer tests przechodzą.
- [ ] Performance budget przechodzi.
- [ ] Wizualnie baseline jest porównywalny z „early WoT-like”: nie AAA, ale konkretny, prawdziwy pojazd.

Acceptance:
- T-54/T-55 staje się quality bar dla reszty garażu.

### Milestone 7: Runtime Variation

Checklist:
- [ ] Decal layer dla trafień.
- [ ] Dirt/mud/camo overlay.
- [ ] Optional equipment attachment points.
- [ ] Damage visibility per module.
- [ ] Track damage state.

Acceptance:
- Runtime dodaje stan i warianty, ale nie odpowiada za pełne modelowanie pojazdu.

### Milestone 8: Migrate Other Vehicles

Kolejność:
1. Jagdtiger, bo testuje casemate i wielką bryłę.
2. Tiger I, bo testuje hard-surface heavy + suspension.
3. Tiger II, bo testuje sloped heavy + turret bustle.
4. Panther II, dopiero po jawnej decyzji interpretacyjnej: museum/prototype/playable planned variant.

Acceptance:
- Każdy nowy pojazd ma ReferencePack, PartGraph, ratio tests, LOD-y, screenshot review i baked material set.

## Test Plan

Core commands:
- `cargo test -p vehicle_geometry`
- `cargo test -p vehicle_forge`
- `cargo test -p renderer_api`
- `cargo test -p renderer_wgpu`
- `cargo check -p client --examples`
- `cargo run -p tools -- forge-vehicle --vehicle t54-1951 --out target/forge/t54_1951`
- `cargo run -p client --example vehicle_lineup_views -- target/vehicle_geometry_views`
- `./scripts/verify.ps1` before declaring a phase complete.

Required scenarios:
- T-54/T-55 bake is deterministic.
- LODs preserve mount frames and gameplay hitbox honesty.
- UVs stay inside atlas bounds.
- Tangents are finite and normalized enough for normal mapping.
- Renderer loads vehicle textures and falls back cleanly if a debug texture is missing.
- Screenshot set contains all required camera views.
- Casemate yaw behavior remains locked for Jagdtiger.
- Existing non-Forge vehicles keep rendering through fallback until migrated.

## Assumptions And Defaults

- Procedural source remains the source of truth; baked artifacts are generated output.
- First target is not full AAA realism; it is **WoT-beta-like readable realism**.
- We do not rename everything immediately. Existing `vehicle_geometry` remains and grows as kernel infrastructure.
- `VehicleBlueprint` is not deleted early; it is gradually superseded by Armored Vehicle Forge concepts.
- We add a separate vehicle renderer path instead of forcing terrain/simple scene meshes into a heavier vertex format.
- We prioritize one excellent benchmark family over shallow upgrades to all vehicles.
- Full runtime procedural generation of complete tanks is explicitly out of scope for the core model pipeline.
