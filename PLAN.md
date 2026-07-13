# Honest Steel T‑54 — realistyczna perforacja i pełne wnętrze

## Podsumowanie

- Usunąć obecny efekt czarnego koła i pięciu jasnych odprysków. Nie będzie poprawiany ani maskowany — zostanie zastąpiony fizyczną aperturą, zdeformowaną stalą i rzeczywistą głębią wnętrza.
- Zakres obejmuje wyłącznie T‑54‑3 obr. 1951, w rzeczywistej skali balistycznej. T‑55 pozostaje poza implementacją.
- Zbudować kompletne, muzealnie wierne wnętrze bez postaci załogi. Podstawą będą instrukcje i katalog części T‑54; oficjalny zestaw [MiniArt 37007](https://miniart-models.com/product/37007-t-54-3-soviet-medium-tank-mod-1951/) posłuży wyłącznie do kontroli przestrzennego montażu. Sprzeczności rozstrzyga dokumentacja okresowa.
- Symulacja pozostaje autorytatywna i deterministyczna. Render, kolizja apertury, replay i późne dołączenie korzystają z tego samego opisu uszkodzenia.

## Model, pancerz i wnętrze

- Forge będzie generował powiązane warstwy: zewnętrzny damageable skin, wewnętrzną powierzchnię każdej płyty, boczne przekroje grubości oraz niedamageowalne spawy, włazy, peryskopy i osprzęt. Każdy trójkąt stalowy otrzyma stabilny `ArmorSurfaceId`, materiał, grubość i ramę `Hull`, `Turret` albo `Mantlet`.
- Wymodelować pełny przedział kierowcy, bojowy, wieżowy, silnikowy i transmisyjny: D‑10T z zamkiem i odrzutnikiem, mechanizmy podniesienia i obrotu, optykę, siedzenia, pulpity, radiostację, pełne zasobniki amunicji, zbiorniki, V‑54, chłodzenie, sprzęgło, przekładnię, final drives, wałki skrętne, okablowanie i przewody.
- Każdy krytyczny element wnętrza otrzyma wspólny `DamageComponentId`: ta sama autorska transformacja zasila mesh wizualny oraz deterministyczne bryły trafień. Zachować dotychczasowe `ModuleSlot` jako agregaty gameplayowe, a szczegółowe komponenty mapować na działo, wieżę, silnik, ammo rack, radio lub zawieszenie.
- Dodać materiały `InteriorPaint`, `MachinedSteel`, `BareFractureSteel`, `OpticalGlass`, `Insulation` i `AmmunitionMetal`. Odsłonięty przekrój nie przejmuje kamuflażu ani team tint.
- Budżety wnętrza: maksymalnie 32 tys. trójkątów LOD0, 12 tys. LOD1 i 2,5 tys. LOD2. LOD wybierać według ekranowej średnicy największego widocznego otworu: ponad 48 px — LOD0, 12–48 px — LOD1, 2–12 px — LOD2, poniżej 2 px — parallax depth impostor.
- Zaktualizować dokumentację T‑54 i Honest Steel: wcześniejszy „16-segmentowy tunel” oraz założenie o niepełnym wnętrzu przestają obowiązywać.

## Perforacja, fizyka i render

- Rozszerzyć `ArmorBreach` o `breach_id`, `shell_type`, `created_tick`, energię uderzenia, energię resztkową, kąt, kaliber/średnicę rdzenia, deterministyczny `fracture_seed`, lokalną bazę powierzchni oraz osobne zwarte deskryptory konturu zewnętrznego i wewnętrznego.
- Kontur generować deterministycznie z typu pocisku, materiału, kąta i energii. Maksymalnie 12 perforacji na pojazd; nakładające się otwory łączyć jako rzeczywistą sumę apertur, bez zastępowania ich większym kołem.
- Zastąpić usuwanie całych trójkątów lokalnym constrained remeshingiem: wyciąć kontur w istniejących trójkątach, zachować fragmenty poza nim, odtworzyć powierzchnię krzywą przez interpolację barycentryczną i połączyć z odpowiadającym konturem wewnętrznym. Wynikiem ma być zamknięty manifold z przekrojem stali, nieregularną krawędzią i bez niebieskiej/pustej szczeliny.
- AP tworzy kalibrowy otwór wejściowy, wewnętrzny plug i szerszy stożek spallu; APCR — mniejszą perforację rdzenia i kruche wykruszenie; HEAT — wąski kanał strugi z lokalnym nadtopieniem i osadem; HE — fizyczne wgniecenie lub rozerwanie, lecz otwór tylko po rzeczywistej penetracji.
- Wyjście na przeciwległej płycie powstaje wyłącznie wtedy, gdy pocisk po przejściu wnętrza i modułów przebije ją od środka. Tworzy osobną perforację typu `Egress`, większe wyrwanie i płatki skierowane na zewnątrz. Pocisk zatrzymany wewnątrz nie tworzy fałszywego otworu wyjściowego.
- Autorytatywna kolizja odejmuje ten sam nieregularny kontur. Przejście jest dozwolone dopiero po zawężeniu apertury o fizyczny promień pocisku; obrzeże zawsze ponownie zderza się ze stalą.
- Dodać natychmiastowe wycinanie analityczne w shaderze pojazdu i shadow passie, aby już w pierwszej klatce było widać wnętrze i światło przez otwór. Asynchroniczny remesh zastępuje maskę po ukończeniu, bez zmiany konturu.
- Decale mogą przedstawiać wyłącznie subtelną utratę farby, sadzę i przebarwienie cieplne. Usunąć czarny stempel, `splash_angles()` i pięć białych smug. Świecenie ma wynikać z energii: krótkie i słabe dla AP/APCR, lokalne dla HEAT, bez trwałych białych krawędzi.
- Trafienia wnętrza rozwiązywać po uporządkowanych przecięciach z bryłami komponentów. Każdy element pochłania energię, generuje deterministyczny spall i może odsłonić własny wariant `Damaged`, `Destroyed` albo `Burning`.
- Konsekwencje obejmują zablokowanie lub opadnięcie działa, zatrzymanie wieży, utratę napędu przez silnik lub transmisję, pożar paliwa, uszkodzenie konkretnych zasobników, pogorszenie radia oraz niezależne uszkodzenia lewej i prawej strony zawieszenia i pasa.
- Wprowadzić protokół v26: pełny stan perforacji i kraterów, rewizję damage mesha, szczegółowe komponenty trafione jednym pociskiem, ich stany, pożary oraz stan termiczny wyliczany z `created_tick`. Replay i late join muszą odtwarzać identyczny rezultat.

## Testy i bramki akceptacyjne

- Forge: damageable skin i powierzchnie wewnętrzne bez degeneratów, samoprzecięć, otwartych krawędzi i błędnego windingu; maksymalna różnica render–balistyka 15 mm globalnie i 5 mm w sąsiedztwie perforacji.
- Remesh: otwory na płaskiej płycie, spawie, policzku wieży, podcięciu i ruchomym jarzmie; brak usuniętych całych dużych trójkątów, pełny manifold i stabilny hash.
- Balistyka: AP, APCR, HEAT oraz HE pod kątem prostym i skośnym; penetracja, zatrzymanie, rykoszet, wewnętrzny spall i rzeczywisty przestrzał przez drugą płytę.
- Apertura: mniejszy pocisk przechodzi przez istniejący otwór, większy oraz trafiający w obrzeże uderza w stal; wynik serwera odpowiada konturowi renderowanemu.
- Wnętrze: kontrolne przekroje z przodu, boków i góry; pozycje komponentów zgodne z dokumentacją, bez przenikania przez wewnętrzny pancerz. Każdy wizualny moduł krytyczny ma odpowiadającą bryłę trafienia.
- Prezentacja: zbliżenia wejścia, przekroju i wyjścia przy 0, 0,25, 2 i 30 sekundach; żadnego czarnego dysku, białej gwiazdy ani płaskiej powierzchni zamykającej otwór na dowolnym LOD.
- Wydajność: przebudowa wyłącznie na workerze, p95 poniżej 8 ms na uszkodzony frame mesha, praca głównego wątku poniżej 0,5 ms i najwyżej jeden upload damage mesha na klatkę.
- Końcowe bramki: protocol goldens v26, replaye, late join, combat hot path, FX i interior LOD budgets, screenshot review, `rustfmt`, `clippy -D warnings` oraz pełne `./scripts/verify.ps1`.

## Założenia

- T‑55 nie otrzymuje wnętrza, geometrii ani kodu specjalizującego go jako kopię T‑54.
- Załoga, gore i gameplay obrażeń ludzi pozostają poza zakresem; modelowane są puste stanowiska i wyposażenie.
- Nie powstaje system voxelowy ani ogólna destrukcja dowolnego pojazdu. Rozwiązanie jest produkcyjne, ale benchmark i kompletne dane dotyczą wyłącznie T‑54‑3 obr. 1951.
- Istniejąca niedokończona implementacja zostanie refaktoryzowana i wykorzystana tam, gdzie zachowuje poprawną prawdę gameplayową; błędna warstwa wizualna zostanie usunięta, a nie przykryta kolejnym efektem.
