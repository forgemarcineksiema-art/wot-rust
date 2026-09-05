# Game design document — the top-level design of the game

**Status.** The owner's design document (v0.1, September 2026) is the top-level design of this
game. It is reproduced below VERBATIM, in the owner's words and language. It was written
alongside the repository rather than from it, so it describes about three fifths of what already
ships, a quarter of what the Inny Poziom register (`docs/inny-poziom-program.md`) has open, and a
handful of things the repository had already decided the other way, with a measurement. Those are
reconciled in the table below (2026-09-02, the owner: "zrób to, co proponujesz"); **where the
table and the text below disagree, the table wins**, and a future decision either cites a chapter
of the text or adds a dated row to the table. Nothing else in `docs/` outranks this file.

## How the document maps onto the work

| Chapter | Where it lives in the repository |
|---|---|
| 0–2 Cel, zasady, produkt | `docs/ROADMAP.md`, `docs/product-program.md` territory; the six rules are the register's three rules plus the product ones |
| 3.1 Strzał | standing: `game_core::armor` (one penetration resolver, A1), Honest Steel interiors, `docs/ammunition.md`, `docs/aiming-model-policy.md` |
| 3.2 Widzenie | register lane **V** (V1–V4, W2) — the optical-density march is not built yet |
| 3.3 Ruch, 4 Fizyka, 7 Movement | register lanes **G** (G7 first: the sprung hull), `docs/vehicle-movement-policy.md`; the ground-material table is `terrain::RoadSurface` + the grip model |
| 3.4 Bitwa | standing: 7v7, one life, the modes and the timer are product rows (R lane) |
| 3.5 Progresja, 15.6 Drzewko | product decisions; the tree and the loadout editor stand; class bands and the crew counter are R-lane rows after W2 |
| 5 Feeling strzału, 12 VFX | register lane **S** (S1 lights, S2 scope feedback, S3 momentum — done, S4 mechanics, S5 being hit, S7–S12) |
| 6 Celownik | standing: A1–A12 (one resolver, refusals, arc feedback, the scope frame, the time-optimal turret, per-gun elevation, the stabilizer); dispersion in `sim::aim_dispersion` |
| 8 Kamera | register lane **C** (C1–C4) and `docs/battle-camera-policy.md`; see the reconciliation on sniper motion |
| 9 Render, 10 Art direction | `docs/art-direction-policy.md` (the seven rules and their locks), `docs/art-direction-program.md` (the defect register), lanes **O**, **Q**, **N**, **T** |
| 11 Audio | `crates/runtime/audio` (pure DSP, deterministic); S4 owes the mechanical layer; crew voices are an R-lane row |
| 13 Świat | lanes **Z** (destruction — Z1–Z3 done), **B** (buildings, W5), **T** (terrain), **F** (flora), **H** (water); `docs/map-forge-policy.md`, `docs/maps/*.md` |
| 15 UI/UX | `docs/interface-program.md` (2026-09-05): lanes **F** (foundation), **H** (battle HUD), **P** (product shell), **G** (garage); it absorbs lane **U** (U1–U11), V3, L1 and L2 of the second pass; L3 (the replay viewer) stays there |
| 16 Sieć | `docs/netcode-*.md`, wire v49, the netcode register; identity/Steam is the open block |
| 17 Boty | standing: the route brain, cover scoring, PvE-first is the population decision |
| 18 Narzędzia | Map Forge, the editor, the probes, `map-atlas`, replays as fixtures |
| 19–21 Launch, sekwencja, ryzyka | the waves W1–W5 of the register follow chapter 20's order (shot → movement → vision → the map with bots) |

## Reconciliation (2026-09-02)

The document was written without the repository's measurements in hand. These are the places
where it and the repository disagreed, and what was decided.

| # | The document says | The repository had decided | Decision |
|---|---|---|---|
| 1 | Rapier for hulls, collisions and props; "determinizm niewymagany" (§4) | its own 2.5-D physics with a deterministic 60 Hz sim and replay-exact locks; the audit found a physics engine touches none of the driving complaints | **Own sim, deterministic.** The document contradicts itself here: §16's replays from the server's input log exist only on a deterministic sim. G7 (the sprung hull in the sim) delivers "45 ton" |
| 2 | Min spec GTX 1060 / RX 580 at 1080p 60 FPS (§2) | one look, MX330 @ 60 FPS, no quality options | **MX330 stays the floor** (the owner, 2026-09-02: "mój laptop ma MX330; gra musi być nieziemsko zoptymalizowana"). The GTX 1060 is the audience's machine, not a licence to spend: the lever is optimisation, and every renderer row still lands with an MX330 measurement |
| 3 | Maps 600 × 600 m on a 0.5 m heightfield (§3.4, §13.2) | 1000 × 1000 m at 5 m samples, five shipped maps, the Terrain Atlas program | **The 1000 m format stays**; resolution rises where it is measured (T1: the 2.5 m ring). A new map may be authored at 600 m as an experiment against the "open book" finding |
| 4 | Damage ±10 % (§3.1) | deterministic damage; the interior already makes 300 damage unequal | **0 %.** Randomness without information is against rule 1; revisit only if a playtest shows the HP exchange counted to the point |
| 5 | Dispersion: truncated Gaussian, σ = r/3 (§6) | centre-biased `r·u²` with a 0.15 floor, measured in A2 | **Keep `r·u²`**; same intent, measured curve; not touched without a playtest |
| 6 | The sniper reticle trembles with the engine and the terrain; the camera kick is stronger in sniper (§5, §8) | the owner's decision of the same morning (S2): in the scope the picture under the player's hand never moves | **The scope is rigid.** "Stań, potem strzelaj" is expressed by the dispersion bloom, never by camera motion |
| 7 | The garage and menus on egui (§15.8) | egui only in the editor; the client's UI is `ui_kit` (one draw call, one look) | **`ui_kit`** (lane U) |
| 8 | Network 30 Hz, ~100 ms interpolation (§16) | a 60 Hz sim, 20 Hz snapshots, a ~50 ms window, the player's own gun locally predicted with no delay | **Keep the repository's numbers**; they are measured and better |
| 9 (2026-09-02) | §13.3 "Pogoda jako wariant mapy: deszcz obcina zasięg widzenia (modyfikator jawny w HUD), zmienia μ"; §4 "Deszcz mnoży μ"; §1.4 "mgła, dym, deszcz działają symetrycznie i są pokazane jako liczba" | the sim has no weather input at all: spotting, physics and `game_core` carry no weather, fog density and rain live in the client's look (`scene_build/weather.rs`, `camera.weather_params`); the only gameplay-relevant effect is whether the PLAYER can still read the picture through a profile (the 35 % contrast rule at view range, `weather.rs:104-118`) | **Owner, 2026-09-02: weather must not spoil legibility and must not change the game.** Weather is presentation only: no view-range cut, no μ multiplier, no HUD modifier (there is nothing to show). Kept as picture: rain on the sniper glass, wet hulls, puddles, fog. Locks: `quality` rules that `sim`/`physics`/`game_core` never depend on look or weather types; every weather profile of every map passes the legibility floor at `max(TankSpec::view_range_m)` (V0's fog item is fixed by density, never by moving the threshold). The same message ranks terrain, trees and all flora as GAMEPLAY: they must read well and naturally, ground and grass at real quality, maps redesigned where needed — the F/T/H rows of W4 move ahead of W3 (see `inny-poziom-program.md`, "Order after the owner's directive") |
| 10 (2026-09-02) | Content: "własny pipeline proceduralny na kernelach GPU (… roślinność …)" (header); §13 flora as a procedural species | procedural trees (Drzewa 3.0) whose frames the owner judged after F7: "these trees, these leaves, this vegetation look tragic, nowhere near real models" | **Route 2 — a tree is DATA** (the owner, 2026-09-02 ~21:00): skeleton grown offline in Blender (Sapling), leaf clusters rendered there, CC0 bark tiles with a licence file beside them, embedded and hash-locked; the runtime (LOD ladder, wind, impostor, honesty boxes) stays procedural. No imported tree MODELS. `docs/map-forge-policy.md` rule 10; `inny-poziom-program.md` F8 |
| 11 (2026-09-03) | the document names no species ("roślinność", header; §13 flora as volumes with optical density) | a willow grown by route 2 (F8) | **NO WILLOW** (the owner, 2026-09-03): `Willow` keeps its enum identity (append-only) and is never planted — `map_forge::RETIRED_KINDS`; assets, rows and horizon entries removed; not to be proposed again |
| 12 (2026-09-04) | the document names no species (as above) | a pine on the ladder | **NO PINE** (the owner, 2026-09-04): `Pine` retired the same way as the willow (PR #687); the living species are oak, poplar, fruit tree and bush |
| 13 (2026-09-05) | §10 "UI: ciemny, płaski, jeden kolor akcentu, kontrast, zero gradientów, cyfry tabelaryczne"; `docs/art-direction-policy.md` "The UI is instrument, not decoration" | exactly that look, shipped and locked: flat graphite plates, one amber accent (`ui_kit/src/theme.rs`) | **Revoked by the owner** ("Jak obecnie jest tak płasko, z jednym akcentem, kolorem — no to jest do dupy"; "przy projektowaniu tego nie ma blokad … czytelnie, intuicyjnie, a także klimatycznie"): the interface's look is **steel, enamel and instrument glass** — depth, material, glass, a warm lamp, a full semantic palette — `docs/interface-program.md` Part I. Kept: tabular digits, contrast floors, no weather on the HUD (row 9) |
| 14 (2026-09-05) | §15.8 "tekst przez glyphon", "garaż, menu, ustawienia na egui z własnym motywem", "budżet 0,5 ms" | row 7: `ui_kit`, one draw call, no egui | **`ui_kit` for everything**: its own SDF glyph atlas with a pair of OFL fonts, one pass; neither glyphon nor egui is adopted; the 0.5 ms is adopted as the TARGET of the HUD pass budget (F1/H26), the FLOOR being the first MX330 measurement |
| 15 (2026-09-05) | §15.1 "Liczby obrażeń pływające, kolor po typie" | S7: the damage number is demoted, the world promoted; A6: a zero is a word | **Both**: the numbers float at S7's size and life, coloured by OUTCOME family (pen / held / module / fire), never by shell type — the shell type is the hit log's job (H8, H9) |
| 16 (2026-09-05) | §15.1 keys 1:1 with WoT: "Shift snajper, T cel, Z komendy, M minimapa, R/F tempomat" | Shift = sniper HOLD, V = toggle, Space = fire, Ctrl = brake; T/Z/M/R/F unbound; "ŻADNEGO aim-assistu" (the owner, 2026-09-02) | **WoT 1:1 defaults** (Shift toggles sniper with hold as a setting, Space is the handbrake, fire on the left button, Ctrl frees the cursor, T/Z/M/R/F as the document says); **T marks the target and pings "attack this" but NEVER lays the gun** — the no-aim-assist decision outranks the key's WoT meaning; every key rebindable (P7) |
| 17 (2026-09-05) | §15.1 team lists and markers with names; "nie ma statystyk innych graczy w bitwie" | no identity on the wire (the protocol carries no `String`); the snapshot filter strips unseen enemies | every hull is **vehicle · seat** (`T-54 · C`) until Steam identity lands; unseen enemies are known only through a roster manifest that withholds positions (W-1); HP bars are battle state, not player statistics — allowed by the same paragraph |
| 18 (2026-09-05) | §15.1 minimap: "okrąg zasięgu widzenia i rysowania" | no draw-distance mechanic exists — visible ⇔ replicated | the view-range circle and the "seen-from" circle only; there is no draw circle to draw honestly (H15) |
| 19 (2026-09-05) | §15.2 after death: "kamera po sojusznikach albo wyjście do garażu z czołgiem zablokowanym do końca bitwy i możliwością wjechania innym" | one battle per session (the garage's BATTLE abandons and redeploys); death orbits the own wreck with the HUD blank | the intel HUD stays and the camera spectates allies from the wire (H19); the destroyed hull is locked in the garage and says so (G14); a second concurrent battle from the garage is a product and netcode row, not an interface one |
| 20 (2026-09-05) | §15.3 "kto cię wykrył … przez krzak o gęstości 0,4", "replay pod jednym przyciskiem"; §15.6 "ile XP brakuje", "kolumny po klasach 1–4" | V1 (density) not built; L3 (the viewer) is W5; no XP, no research, no economy; class bands are an R-lane row after W2 | the observer is named only from `BattleEnded.spotting_log` after the battle (W-7); density appears the day V1 lands; the replay button is disabled and says why (P10); a tree node says what it is and what follows it, no XP and no locks (G12); columns keyed by tier until the bands land |

The document's largest honest gap is not in this table: "content robi pipeline" (30–40 tanks
at launch) against a fleet with one vehicle of eight at the benchmark bar, and a benchmark that
cost a whole program. That is lane **K** (Kuźnia 2.0), and it is the risk above netcode.

---

# Dokument projektowy — gra czołgowa (nazwa robocza)

Silnik: własny, Rust + wgpu + Bevy ECS + Rapier. Content: własny pipeline proceduralny na kernelach GPU (czołgi, teren, roślinność, budynki, materiały).

Wersja 0.1 — wrzesień 2026. Dokument jest zbiorem decyzji, nie opcji. Każda z nich ma jedno uzasadnienie i jest do zmiany tylko wtedy, gdy playtest ją obali.

---

## 0. Cel i pozycjonowanie

**Cel:** gra, którą weteran World of Tanks poleca drugiemu weteranowi zamiast "wracaj do WoT". Nie wyciąganie graczy Wargamingowi na ich boisku (F2P, 600 czołgów, 10 regionów, 15v15). Armored Warfare próbowało tego z całym Obsidianem i pieniędzmi Mail.ru i praktycznie umarło.

**Wzorzec:** BattleBit — trzech ludzi, "Battlefield bez ściemy", ponad milion kopii w pierwszych tygodniach. Z drugą połową lekcji: populacja BattleBita zjechała do zera w rok, bo trzech ludzi nie utrzymało tempa contentu. Dlatego cały plan jest zbudowany tak, żeby content robił pipeline, nie ręka.

**Rdzeń WoT, który zostaje:** precyzyjny środek między arcade a symulacją — pula HP, jedno życie, trzecia osoba plus snajper, drzewko, spotting jako zasób. Wszystko, co robi z WoT WoT, a nie War Thunder.

**Realny sukces:** kilka tysięcy graczy w EU prime time, 100–300 tys. sprzedanych kopii, gra, z której da się żyć, nazywana "tym, czym WoT powinien był zostać".

**Realna porażka:** nie brak graczy, tylko 18 miesięcy dłubania w silniku, bo netcode i boty są nudne, a rendering jest ciekawy. Sekwencja w rozdziale 20 jest ułożona tak, żeby ten scenariusz był trudniejszy do zrealizowania niż tamten.

---

## 1. Zasady nadrzędne

1. **Nie ma tabelek, jest geometria.** WoT to warstwy ukrytych tabel (normalizacja, RNG penetracji, punkty spottingu, kamuflaż, opór terenu) z modelem pancerza jako fasadą. Tu model jest grą, a każda mechanika to pytanie zadane modelowi. Gracz nie uczy się reguł z wiki, bo reguły są tym, co widzi.
2. **Wszystko zachowuje się tak, jak wygląda.** Drewniany płot pęka, kamienny mur nie. Krzak chowa, drzewo blokuje. Woda spowalnia. Żadnych niewidzialnych ścian poza granicą mapy. Immersję łamie głównie "dlaczego to nie zadziałało".
3. **Co wpływa na grę, jest deterministyczne, wypieczone i identyczne u wszystkich.** Krzak, dom, krater — serwera. Trawa, ptaki, kurz — klienta i skalowalne. Tryb ziemniaka usuwa trawę, nigdy krzak.
4. **Immersja nigdy kosztem czytelności.** Mgła, dym, deszcz działają symetrycznie i są pokazane w interfejsie jako liczba.
5. **Uczciwość jako feature.** Gra mówi, co się stało i dlaczego. Log trafień, inspektor pancerza, oś czasu spottingu, replay. To, do czego WoT-owcy używają trzech narzędzi zewnętrznych, tu jest ekranem w grze.
6. **Silnik jest zamrożony.** Każda godzina w feature silnika spoza ścieżki krytycznej to godzina zabrana grze.

---

## 2. Decyzje produktowe

| Decyzja | Wartość | Uzasadnienie |
|---|---|---|
| Model | Płatna, ~20 €, Steam Early Access | F2P to konkurowanie z WoT na osi, gdzie oni są najsilniejsi, i wymusza "premium coś", które rozbija pitch. Cena wejścia to najtańszy anti-cheat. |
| Monetyzacja | Zero złotej amunicji, czołgów premium ze statystykami, premium time, kredytów. Kosmetyki może, później. Kolejne ery jako płatne rozszerzenia. | Cały pitch to "uczciwy WoT". |
| Era na start | WWII (1939–45), jedno pasmo | Early (1916–39) i Cold War (1945–91) później jako rozszerzenia. |
| Content na start | 30–40 czołgów, 4 mapy | Wykonalne solo wyłącznie dzięki pipeline'owi. |
| Format bitwy | 7v7, jedno życie, 7 minut | Decyzja populacyjna: mecz potrzebuje 14 ludzi zamiast 30, z botami — 7. Przy 200 online w EU prime time to 15–30 bitew równolegle i kolejka poniżej minuty. |
| Archetypy | Lekki, średni, ciężki, niszczyciel czołgów | Artylerii nie ma. |
| Matchmaking | Klasa 1–4 w paśmie, spread ±1 | |
| Region | Jeden serwer EU (Hetzner DE/FI) | Koncentracja populacji. |
| Min spec | 1080p 60 fps na GTX 1060 / RX 580 | Publiczność WoT, zwłaszcza w Polsce, siedzi na starym sprzęcie. |
| Lokalizacja | PL/EN/DE/RU od startu | |
| Ludzie na ekranie | Nigdy | Abstrakcja gatunku, ton, rating. |

---

## 3. Rdzeń rozgrywki

### 3.1 Strzał

Pocisk to promień przez model materiałowy czołgu (z pipeline'u kerneli). Dla każdego trafienia liczona jest grubość materiału wzdłuż promienia od wejścia do wyjścia — efektywna grubość z kątem wychodzi z geometrii, bez osobnej tabeli normalizacji.

| Typ | Zachowanie |
|---|---|
| AP | Całka grubości vs penetracja. Overmatch: kaliber > 3× grubość nominalna przebija niezależnie od kąta (bez tego płyta pod 89° jest nieśmiertelna). |
| APCR | Wyższa penetracja, gorzej znosi kąt (mnożnik na całkę przy skośnym wejściu), spada z dystansem. |
| HEAT | Zero spadku z dystansem. Strumień traci pen liniowo na każdy mm powietrza po pierwszym kontakcie z materiałem — ekrany i fartuchy działają jak w rzeczywistości, bez przypadku specjalnego w kodzie. |
| HE | Detonacja na pierwszym kontakcie, obrażenia w funkcji grubości w punkcie, splash. |

**RNG:**
- Penetracja **deterministyczna**. Zero ±25%. 150 pen vs 149 efektywnej — wchodzi, zawsze.
- Rozrzut w celowniku **zostaje** — jest skill-compatible, uczysz się, kiedy strzelać.
- Obrażenia **±10%** — tyle, żeby wymiana HP nie była policzalna co do punktu, za mało, żeby ktoś zrobił 100 zamiast 300.

**Post-pen:** pocisk, który przebił, leci dalej przez wnętrze jako promień, tracąc energię, i uszkadza to, co przetnie: magazyn amunicji, silnik, zbiorniki, załogę. Wnętrze to takie same wolumeny jak pancerz. Tu dokładna replika przestaje być fetyszem — wiedza, gdzie w danym czołgu leży amunicja, jest umiejętnością, którą gra nagradza. Pula HP zostaje, ale przez wnętrze nie każde 300 dmg jest równe.

### 3.2 Widzenie

Spotting to ukryty rdzeń WoT — cały design map i klasa lekkich stoją na tym, że widoczność jest zasobem. Idea zostaje, tabela wylatuje.

- Krzak, korona drzewa, dym z wraku, żywopłot — wolumeny z gęstością optyczną.
- Linia wzroku od obserwatora do punktów spottingu celu całkuje przesłonięcie tak samo, jak pocisk całkuje pancerz.
- Zasięg widzenia i kamuflaż czołgu to dwa skalary przesuwające próg.
- Deterministyczne i jawne w UI: gracz widzi budżet widoczności i co go zjada.
- Szósty zmysł dla wszystkich od pierwszej bitwy.
- Auto-spot 50 m zostaje.
- Trawa nigdy nie wpływa na widzenie. Mechanika spottingu jest niezależna od oświetlenia (cienie to percepcja gracza, nie mechanika).

### 3.3 Ruch (design)

- Czołg ma ważyć. Kołysanie kadłuba z zawieszenia mówi "45 ton" bez HUD-u.
- Opór terenu z materiału podłoża: błoto i piach spowalniają naprawdę i widocznie.
- Skręt to cecha czołgu, nie ustawienie: hamulcowy, przekładniowy, podwójny dyferencjał z obrotem w miejscu. Sherman i Tiger różnią się w ręku.
- Depresja i elewacja działa dokładne co do stopnia — to mechanika zamieniająca rzeźbę terenu w taktykę.
- Jeśli fizyka pozwala wjechać na skałę, wjeżdżasz. Jeśli to psuje mapę, naprawiana jest mapa.

### 3.4 Bitwa

- 7v7, jedno życie (nienegocjowalne — stąd napięcie i to, że pozycja ma znaczenie), 7 minut.
- Tryby: standard (dwie bazy) i jeden z ruchomym celem, żeby kampienie miało cenę.
- Artyleria: nie ma. Jej funkcja (karanie stagnacji) rozwiązana timerem i trybem z celem.
- Mapy 600×600 m, teren pierwszy. Każda ma: grzbiet do hull-down, korytarz flankujący, strefę kontroli wizji, otwarte pole, którego przekroczenie boli. Żadnych korytarzy.

### 3.5 Progresja

- Poziomo w paśmie WWII, klasa 1–4 (z grubsza 1939–40, 41–42, 43–44, 45).
- Drzewko zostaje (hak kolekcjonerski), ale całe pasmo to ~100 godzin, nie tysiąc.
- Załoga: jeden licznik na czołg, cap po ~10 godzinach, drobne bonusy.
- Wyposażenie: 3 sloty z jednego zestawu, swap za darmo.
- Materiały eksploatacyjne darmowe.
- Kredytów nie ma. Jest XP i są czołgi.

---

## 4. Fizyka

- Rapier na kadłuby, kolizje czołg–czołg i propsy. Własne zawieszenie na raycastach po wierzchu (nie kontroler pojazdu Rapiera, który zakłada Ackermanna).
- Fixed step 60 Hz, sieć co drugi krok, render interpoluje. Determinizm niewymagany.
- Kadłub: jedna bryła sztywna, compound collider (kilka OBB lub convex hull z siatki). Nigdy pełny model w broadphase.
- **Napęd:** krzywa momentu × przełożenie, automatyczna skrzynia z punktami zmiany biegów. Daje "wolno rusza, potem dociąga" bez tuningu; moc/tona staje się statystyką, którą czuć.
- **Trakcja:** siła na koło ograniczona μ·N, μ z materiału podłoża; opór toczenia jako drugi współczynnik. Skręt przez różnicę prędkości gąsienic → moment obrotowy. Z tego wynika limit podjazdu tan(θ) < μ i zjeżdżanie na lodzie.
- **Zawieszenie:** sprężyna/tłumik per koło, limity skoku. Kołysanie przy hamowaniu i przyspieszaniu. Wysokość przeszkody, którą czołg przekracza, wynika ze skoku i punktu startu raycastu — nie z listy.
- **Taranowanie:** obrażenia z względnej energii kinetycznej ważonej stosunkiem mas. Ciężki przepycha średniego i to boli tylko średniego.
- **Propsy:** dwie klasy — bryły z masą (płot, drzewko; próg masy decyduje, czy lekki je ruszy) i obiekty stanowe (mur, dom; stany, nie symulacja).
- **Pociski:** swept raycast, nie CCD Rapiera. Grawitacja tak, opór powietrza nie.
- **Gąsienice:** zerwana jedna → obrót w miejscu; dwie → unieruchomienie; timer naprawy.
- **Woda:** bród spowalnia, głęboka topi po timerze jawnym w HUD.

### Tabela materiałów podłoża (wartości startowe, w danych)

| Materiał | μ | Opór toczenia |
|---|---|---|
| Droga | 1,0 | niski |
| Trawa | 0,9 | niski |
| Piach | 0,7 | średni |
| Błoto | 0,6 | wysoki |
| Śnieg | 0,5 | średni |
| Lód | 0,2 | niski |

Deszcz mnoży μ. Wartości do balansu w plikach danych, nie w kodzie.

---

## 5. Feeling strzału

Suma pięciu rzeczy w tej samej klatce po kliknięciu, lokalnie, zanim serwer odpowie:

1. **Odrzut kadłuba** — impuls na bryłę proporcjonalny do pędu pocisku. Tiger siada na zawieszeniu, Pz II drga.
2. **Cofnięcie lufy w jarzmie** — 2–3 klatki w tył, powolny powrót.
3. **Kick kamery** — kierunkowy, skalowany kalibrem; w snajperze mocniejszy, potem osiada.
4. **Błysk i pierścień kurzu** podnoszony z ziemi wokół czołgu.
5. **Dźwięk trójwarstwowy** — mechanika zamka, huk zależny od kalibru, ogon zależny od otoczenia (pole vs miasto).

Do tego:
- Smuga — czas lotu widoczny; 600 m/s i 1000 m/s to różne pociski.
- Trafienie: osobne dźwięki i VFX dla przebicia, rykoszetu, modułu. Na własnym czołgu: wskaźnik kierunku, mignięcie HP, szarpnięcie kamery skalowane obrażeniami.
- Deterministyczny decal w miejscu trafienia (rozdz. 13.1).
- Wynik (pen/bounce) przychodzi po RTT, ale klient już pokazał wszystko lokalne.
- Dźwięk odległych strzałów z opóźnieniem z prędkości dźwięku: 600 m to 1,8 s. Realny sygnał taktyczny.
- Wypalony ślad wylotowy na ziemi przed lufą po strzale ze stojącej pozycji.

---

## 6. Celownik i balistyka

- **Predykcja wieży:** klient przewiduje obrót wieży i działa tą samą symulacją co serwer (prędkość z przyspieszeniem, limity depresji zależne od kąta wieży), serwer koryguje. Celownik zbiega się zamiast rozjeżdżać — dokładnie tam, gdzie WoT ma problem z "serwerowym celownikiem".
- **Rozrzut:** promień = bazowy/100 m × dystans × mnożniki (ruch, obrót kadłuba, obrót wieży, po strzale); zbiega wykładniczo z czasem celowania. Rozkład: ucięty Gauss, σ = r/3.
- **Deterministyczny rozrzut:** losowanie z PRNG seedowanego tickiem i ID gracza. Klient i serwer liczą ten sam odchył, więc przewidywana smuga leci dokładnie tam, gdzie serwerowa. Zero "moja smuga poszła gdzie indziej".
- **Balistyka:** grawitacja tak, opór powietrza nie — spadek penetracji z dystansem tabelą. Celownik kompensuje opad na dystans punktu celowania (jak WoT). Bez asysty wyprzedzenia.
- **Dwa elementy celownika:** gdzie trafia promień kamery i gdzie faktycznie patrzy lufa (mała kropka). Rozjeżdżają się przy przeszkodzie — widać.
- **Wskaźnik pen/marginal/no-pen** z tego samego raymarchu w punkcie celowania, tylko dla podświetlonych celów. W WoT to zgadywanka, tu prawda. Przewaga wiedzy przenosi się na wnętrze.
- **Jeden celownik, serwerowy** — przy serwerze autorytatywnym z rewindem nie ma dwóch prawd.
- Zoom 2/4/8× ze skalowaniem czułości, osobno na każdy poziom.

---

## 7. Movement — szczegóły

- Sterowanie: W/S gaz, A/D skręt, tempomat R/F (jak WoT), auto skrzynia.
- Modele skrętu per czołg:
  - **Hamulcowy** — jedna gąsienica stoi, promień skrętu rośnie z prędkością.
  - **Przekładniowy** — stałe promienie zależne od biegu.
  - **Podwójny dyferencjał** — obrót w miejscu (neutral steer).
- Animacja z fizyki, nie osobno: pozycje kół z odległości raycastów, wahacze podążają, gąsienica jako siatka na splajnie po kołach z przesuwem UV według prędkości.
- Pył za gąsienicami z prędkości i materiału. Błoto akumuluje się na gąsienicach (shader).
- Trawa i krzaki uginają się pod kadłubem (warstwa dynamiczna terenu).
- Przepychanie sojuszników, taranowanie (rozdz. 4).

---

## 8. Kamera

- **Trzecia osoba:** orbita zakotwiczona nad wieżą, spherecast do terenu i obiektów. Tłumienie na ruch kadłuba, **zero tłumienia na celowanie** — mysz 1:1. Blisko kadłuba, żeby masę było czuć w tym, jak kadłub przesłania świat.
- Ograniczona wysokość zawieszenia kamery (mniej zaglądania za grzbiet niż WoT), ale nie zabrana całkiem — to część feelu.
- **Snajper:** kamera przy działnie, zoom 2/4/8× ze zmianą FOV. Celownik drży subtelnie z obrotami silnika i terenem, uspokaja się po zatrzymaniu (uczy "stań, potem strzelaj").
- Free look pod klawiszem. Kamera obserwatora po śmierci z widokiem przez sojuszników.
- Bliskie wybuchy trzęsą ekranem z opadaniem po odległości.

---

## 9. Render

- wgpu, **clustered forward**, nie deferred — MSAA 4× i sensowna przezroczystość dla roślinności.
- HDR, PBR, jedno światło kierunkowe + kilka lokalnych (błysk, ogień).
- Cienie: 4 kaskady 2048, PCF.
- **Teren:** quadtree z geomorfingiem, splat 8 materiałów, **height-blend** zamiast liniowego mieszania (jedna zmiana, dwa razy lepszy wygląd), triplanar tylko na stromych zboczach, makro-szum przeciw kafelkowaniu.
- **Czołgi:** siatki z pipeline'u z LOD-ami. Zapytania gameplayowe (pancerz, wnętrze) w compute, nie w renderze.
- **GPU-driven:** indirect draw, culling frustum + HZB. Koszt siedzi w roślinności, nie w 14 czołgach.
- Trawa instancjonowana z LOD, alpha-to-coverage; drzewa: pełna siatka → uproszczona → impostor oktaedryczny.
- Post: GTAO, subtelny bloom, AgX tonemapping, LUT, mgła wysokościowa. Motion blur wyłączony. TAA jako opcja, nie domyślnie.
- Niebo fizyczne z ustaloną porą dnia na mapę. Zmierzch tak, noc nie.
- Bez streamingu — mapa 600 m mieści się w pamięci.
- Budżet klatki 16 ms na min specu. HUD ≤ 0,5 ms.

---

## 10. Grafika / art direction

- **Kierunek:** realistyczne, ale czytelne — WoT, nie War Thunder. Czołg na 400 m musi być do zobaczenia: otoczenie odbarwione, czołgi z wyraźnym albedo i czytelną krawędzią.
- **Materiały proceduralne:** krzywizna z modelu daje starcie krawędzi, zapadnięcia dają brud i AO — weathering bez malowania.
- Kamuflaże z szumu per nacja. Oznaczenia taktyczne, numery, godła jednostek proceduralnie — czołg jest czyjś.
- Drzewa (space colonization), budynki modułowe ze stanami zniszczenia, krzaki jako klastry billboardów.
- Ludzka skala jako odniesienie: płoty, drzwi, słupy telegraficzne — bez nich czołg nie ma rozmiaru.
- UI: ciemny, płaski, jeden kolor akcentu, kontrast, zero gradientów, cyfry tabelaryczne.

---

## 11. Audio

- Własny mikser na cpal albo kira.
- **Silnik jako żywa maszyna:** crossfade pętli po RPM, bieg jałowy, dławienie pod górkę, zmiana biegów; benzyniak vs diesel jako różne barwy.
- Gąsienice klekoczą inaczej na bruku, w błocie, na śniegu.
- Obrót wieży ma dźwięk napędu; w czołgach z ręcznym obrotem słychać korbę (immersja + informacja o czołgu).
- Zamek, wyrzut łuski, brzęk o podłogę.
- **Snajper = "jesteś w środku":** zewnętrze stłumione, ambient wnętrza (silnik przez kadłub, wentylacja, metal). Wyjście do trzeciej osoby otwiera dźwięk.
- Świst pocisków nad głową, trzask bliskiego chybienia, jęk rykoszetu.
- Opóźnienie po odległości, okluzja przez raycast do terenu (za wzgórzem stłumione).
- **Załoga** mówi w języku nacji przez filtr radiowy z paskiem tekstu. 10–15 kwestii na nację: wykryty, trafieni, magazyn, gąsienica, pożar. Filtr ukrywa jakość nagrania — native speakerzy ze społeczności zamiast studia.
- **Konsekwencje słyszalne:** uszkodzony silnik = niższe obroty, zerwana gąsienica zmienia dźwięk jazdy, ranny ładowniczy = wolniejsze przeładowanie, trafienie w kadłub = dzwon stali, pożar trzeszczy, magazyn = osobny głuchy odgłos.
- Tło: odległy pomruk ostrzału, samolot wysoko, ptaki podrywające się po strzale w pobliżu.
- **Muzyki w bitwie nie ma.** Muzyka w garażu i na ekranie wyników.

---

## 12. VFX

- Cząstki na compute: kurz, dym, ogień, błoto spod gąsienic, bryzg ziemi, iskry.
- VFX trafienia z materiału: bryzg ziemi (gleba), iskry (kamień), pył (cegła), plusk (woda), puch (śnieg).
- Błysk wylotowy, pierścień kurzu, smuga, ogień z uszkodzonego silnika (czarny dym), pył opadający z sufitu w snajperze po trafieniu.
- **Śmierć czołgu** nie jest jedna: detonacja amunicji odrzuca wieżę (ląduje jako prop), pożar dopala, zwykłe zero HP to mniejsza eksplozja. Wrak dymi do końca bitwy, widoczny z daleka.
- Deszcz: krople na szkle snajpera.

---

## 13. Świat

### 13.1 Ślady po strzałach

**Na czołgu:** decal wynika z danych serwera (punkt, normalna, wektor wejścia, typ, wynik) — identyczny u wszystkich i w replayu. Cztery rodzaje:
- dziura po przebiciu z wywiniętym metalem,
- wgniecenie po nieprzebiciu,
- rysa po rykoszecie wydłużona w kierunku odbicia (z wektora wejścia i normalnej),
- osmalenie po HE.

Implementacja: lista trafień per czołg (do 64), ewaluowana w fragment shaderze przez siatkę komórek w przestrzeni lokalnej czołgu — bez UV. Najstarsze zlewają się w ogólne zużycie. Decal jest informacją: przeciwnik widzi rysę tam, gdzie się odbił.

**Na terenie:** HE robi krater — radialne obniżenie w warstwie dynamicznej, materiał "spalona ziemia" w splacie, przeliczenie normalnych. Płytki, 0,3–0,5 m: czuć na zawieszeniu, nie daje osłony (głęboki zamieniłby spam HE w inżynierię terenu). Chybienia AP to małe wyrwy. Wszystko zostaje do końca bitwy.

**Na budynkach:** część systemu stanów (13.4).

### 13.2 Teren

- Heightmapa 0,5 m na mapę 600×600. Fizyka próbkuje biliniowo bezpośrednio (własne, spójne z renderem).
- Warstwy: wysokość, splat 8 materiałów, wilgotność, **warstwa dynamiczna** (koleiny, kratery, ugięcie trawy).
- Materiał w punkcie z argmax splatu → μ i opór z tabeli (rozdz. 4).
- Koleiny malowane z punktów kontaktu gąsienic; w błocie prawdziwe, z minimalnym przesunięciem wysokości.
- **Generacja:** erozja hydrauliczna i termiczna na GPU (grzbiety i doliny robią hull-down naturalnie) + warstwa autorska: prymitywy taktyczne jako pędzle — grzbiet, niecka, korytarz, nasyp. Makro jest ręczne, mikro i tekstura z pipeline'u.
- Drogi jako splajny spłaszczające teren i malujące materiał; wsie na spłaszczonych platformach.
- Powyżej progu nachylenia: skała i zakaz podjazdu z fizyki, nie z niewidzialnej ściany.
- Poza granicą grywalną świat trwa wizualnie kilkaset metrów w niskiej rozdzielczości; granica to czerwona linia i twardy stop — reguła, nie udawana ściana. Za horyzontem 2–3 km terenu, las z impostorów, kolumny dymu.

### 13.3 Drzewa i roślinność

- Drzewa z pipeline'u, łańcuch LOD (pełna siatka → uproszczona → impostor).
- Pień nieprzezroczysty, korona z gęstością optyczną.
- **Powalenie:** po przekroczeniu progu siły zawias u podstawy, fizyczny upadek, potem statyk. Pień leży, korona staje się krzakiem, zostaje pniak. Padające drzewo widać z daleka — zdradza pozycję, to sygnał z WoT, który zostaje.
- **Trawa nigdy nie wpływa na widzenie. Krzaki i drzewa zawsze** — i renderują się identycznie na każdych ustawieniach.
- Rozmieszczenie proceduralne z masek (nachylenie, wysokość, wilgotność), **wypiekane z ustalonym seedem w pliku mapy** — każdy krzak jest elementem gry. Krzaki taktyczne stawiane ręcznie i zamykane.
- Warianty sezonowe (lato, jesień, zima) z tego samego generatora. Śnieg z widocznymi śladami.
- Pogoda jako wariant mapy: deszcz obcina zasięg widzenia (modyfikator jawny w HUD), zmienia μ.

### 13.4 Budynki

- Gramatyka modułowa: obrys → kondygnacje → ściany z otworami → dach. Styl per region: polska wieś, niemieckie miasteczko, normandzki kamień.
- **Zniszczenie per segment ściany**, nie per budynek. Cztery stany: cały, uszkodzony, ruina, gruz.
- Materiał decyduje: drewno pada od taranu, cegła od kilku HE, kamień tylko od bezpośredniego dużego HE.
- Gruz: niska osłona, przejezdny wolno.
- **Ściana to grubość materiału w tej samej tabeli co ekran pancerza:** AP przechodzi przez drewnianą stodołę z utratą pen, staje na cegle. Okna i drzwi to otwory w proxy kolizji i wzroku — linia wzroku przez okno działa bez przypadku specjalnego.
- Do środka czołg nie wjeżdża. Kościół i wieża jako punkty orientacyjne.
- Ryzyko balansowe (ciężki z HE przebuduje sobie wieś) akceptowane — jest uczciwe i emergentne, a limit daje czas: czołg, który dwie minuty rozbiera dom, nie robi nic innego.
- Navmesh kafelkowy, przeliczany lokalnie, gdy dom zamienia się w gruz.

### 13.5 Elementy mapy

Pola z rzędami upraw (otwarty teren), żywopłoty i bocage (blokery z gęstością), kamienne murki, nasypy kolejowe (osłona), mosty (niezniszczalne przewężenia), brody (alternatywa), rzeki z głębokością. Wraki "z poprzednich bitew" jako scenografia i osłona — spalone, bez wież, rdza — wizualnie odrębne od żywych. Nazwy miejscowości, drogowskazy w języku kraju, rekwizyty z epoki, zero fantazji.

### 13.6 Plik mapy

Jeden plik: heightfield, splat, warstwa dynamiczna początkowa, lista propów z seedami, wolumeny gęstości, navmesh, spawny, cele. Serwer i klient ładują to samo i hashują — mod usuwający krzaki nie połączy się z serwerem.

---

## 14. Immersja — podsumowanie decyzji

- Świat zachowuje się tak, jak wygląda (zasada 2).
- Dźwięk = 60% immersji za 5% kosztu (rozdz. 11).
- Konsekwencje czuć zanim spojrzysz na ikonę.
- **Ślady:** koleiny, powalone drzewa, rozbite mury, kratery, decale, dym z wraków. Po pięciu minutach mapa wygląda jak po pięciu minutach bitwy.
- Dym z płonącego wraku blokuje linię wzroku — kolejny wolumen gęstości; z tego wychodzi taktyka (tymczasowa osłona w korytarzu, po obu stronach tak samo).
- Skala i ciężar: ludzka skala w rekwizytach, kamera blisko kadłuba, drżenie celownika z silnika.
- Pole jako miejsce: nazwy, drogowskazy, oznaczenia jednostek, wojna w tle, która nie jest mechaniką.
- Garaż: czołg z uruchomionym silnikiem, obejście dookoła, obrót wieży pod myszą.
- Granica: immersja nigdy kosztem czytelności (zasada 4).

---

## 15. UI/UX

### 15.1 HUD w bitwie

Zasada: lista modów, które WoT-owcy doinstalowują (XVM, celowniki, panele), to spec HUD-u. Układ przestrzenny **identyczny z WoT** (minimapa prawy dół, listy drużyn po bokach, panel uszkodzeń lewy dół, amunicja na dole, timer i wynik na górze) i **domyślne klawisze 1:1 z WoT** (Shift snajper, T cel, Z komendy, M minimapa, R/F tempomat). Pamięć mięśniowa jest featurem retencji.

Elementy:
- Łuk przeładowania wokół celownika.
- Log ostatnich trafień pod celownikiem: pen vs efektywna, kąt, moduł. Zwijalny, domyślnie włączony.
- Pasek sumy HP obu drużyn na górze.
- Minimapa: okrąg zasięgu widzenia i rysowania, kierunek wieży i kadłuba, ostatnie znane pozycje wygasające, pingi. Skalowalna klawiszem.
- Markery: pełne tylko dla celu, reszta wygaszona.
- Panel uszkodzeń: rzut z góry z modułami i załogą, progres naprawy.
- Wskaźnik kierunku trafienia, szósty zmysł (dźwięk + lampka).
- Liczby obrażeń pływające, kolor po typie.
- Komendy przez kółko radialne z pingami na minimapie i w 3D, limit na minutę.
- **Nie ma statystyk innych graczy w bitwie.** XVM z win rate'ami zrobił więcej toksyczności niż artyleria. Swoje widzisz, cudze tylko za zgodą właściciela.

### 15.2 Zasady UX bitwy

- Dwie sekundy na odczyt każdego elementu.
- Wszystko skalowalne i przesuwalne w edytorze HUD w grze. Trzy presety: minimalny, standard, pełny.
- Palety dla daltonistów od pierwszego dnia.
- Zero pop-upów w bitwie. Cyfry tabelaryczne.
- Po śmierci: kamera po sojusznikach albo wyjście do garażu z czołgiem zablokowanym do końca bitwy i możliwością wjechania innym.

### 15.3 Ekran po bitwie

Nie tylko obrażenia, asysty, spotting — **oś czasu bitwy**: kto cię wykrył i kiedy, każdy twój strzał z wynikiem, każde trafienie w ciebie z punktem i przebiciem. "Wykrył cię lekki z 380 m przez krzak o gęstości 0,4" uczy mechaniki widzenia w tydzień. Replay pod jednym przyciskiem.

### 15.4 Garaż

Czołg na środku, karuzela na dole z filtrami (klasa, nacja, klasa 1–4). **Jeden przycisk: Bitwa.** Zakładki: drzewko, inspektor pancerza, replaye, statystyki, ustawienia. Nie ma sklepu, promocji, czerwonych kropek, dziennych nagród, timera battle passa. Garaż bez ekranu ładowania, w tym samym rendererze.

Statystyki czołgu: prawdziwe liczby, nie paski 7/10. Pozycja względem średniej klasy, wartości pochodne (efektywny przód pod 0°, moc/tona, rozrzut w ruchu). Porównanie dwóch czołgów obok siebie.

### 15.5 Inspektor pancerza

Ten sam raymarch co w bitwie: klik w punkt → grubość nominalna, efektywna, kąt. Warstwy: pancerz, ekrany, moduły, załoga. Tryb "strzel do mnie": pocisk + dystans + kąt czołgu → cały kadłub koloruje się mapą przebić; obracasz i widzisz, przy jakim kącie sidescrape zaczyna działać. To jest feature, który sprzedaje grę na YouTube.

### 15.6 Drzewko

Poziome, kolumny po klasach 1–4, wiersze po archetypach. Każdy węzeł mówi, co odblokowuje i ile XP brakuje. Zero mgły — całe pasmo widoczne od początku.

### 15.7 Onboarding

15-minutowy samouczek PvE, który uczy prawdziwych umiejętności: ustawianie kadłuba pod kątem, hull-down, sidescrape, widzenie przez krzak — każde jako mini-scenariusz z botem.

### 15.8 Technicznie

- HUD rysowany własnym kodem: atlas sprite'ów, tekst przez glyphon, jedno przejście, budżet 0,5 ms.
- Garaż, menu, ustawienia na egui z własnym motywem.
- Klawisze w pełni rebindowalne, czułość osobno na każdy zoom, skalowanie UI niezależne od rozdzielczości. Pad później.

### 15.9 Społeczne

Pluton 2–3 osoby, znajomi przez Steam, czat drużynowy i plutonowy. Bez wbudowanego voice (Steam voice w plutonie). Klany po Early Access.

---

## 16. Sieć i serwer

- **Serwer autorytatywny**, klient wysyła tylko input. Tick 30 Hz (co drugi krok fizyki).
- Predykcja własnego kadłuba i wieży, interpolacja reszty ~100 ms.
- Pociski symulowane na serwerze swept raycastem z **rewindem celu do czasu strzału** (lag compensation) — leczy "trafiłem, a nie weszło".
- Deterministyczny rozrzut (rozdz. 6) → przewidywana smuga = serwerowa.
- UDP z kanałem reliable. Bevy headless na serwerze bez wgpu.
- Jeden rdzeń uciągnie kilkanaście meczów 7v7. Hetzner DE/FI. Koszt hostingu przy grze płatnej pomijalny.
- Konta: Steam auth. Matchmaking: prosty, ±1. Replay z serwerowego logu inputów.
- Anti-cheat: serwer autorytatywny na wszystko (ruch, strzał, widzenie), hash pliku mapy, VAC. Bez kernel anti-cheata.

---

## 17. Boty i PvE

- Gra startuje jako **PvE/co-op z PvP jako trybem, który zapala się, gdy jest populacja.** Boty na ścieżce krytycznej, nie w backlogu.
- Navmesh kafelkowy, utility AI, ocena osłon: bot rozumie hull-down i depresję działa (ocena pozycji względem zagrożenia z tego samego modelu terenu i pancerza). To jest lepsze niż połowa graczy WoT.
- W PvP boty dopełniają do progu, jawnie oznaczone.
- PvE: scenariusze, samouczek, tryb treningowy.

---

## 18. Narzędzia i workflow

Solo narzędzia są ważniejsze niż feature'y.

- **Edytor map w silniku:** pędzle prymitywów taktycznych, scatter z maskami, malowanie propów, przycisk "wjedź z botami". Od edycji do jazdy poniżej minuty. Cztery mapy na start = dwadzieścia iteracji każdej.
- Hot reload materiałów i parametrów.
- Profiler.
- **Telemetria z bitew:** heatmapy śmierci i strzałów wracają do edytora jako warstwa.
- Dane czołgów w RON/TOML — balans liczbami, nie kodem.
- Replay jako narzędzie debugowe.

---

## 19. Populacja i launch

- **Problem pustego serwera rozwiązany przed premierą:** PvE-first, boty, 7v7, jeden serwer.
- **Koncentracja:** EU, Polska jako boisko domowe (jedna z największych społeczności WoT w Europie, wspólny język). Pierwszy tysiąc buduję tam.
- Na początku ogłaszane **okna gry** (wieczory 19–23 CET) zamiast udawania, że serwer żyje 24/7.
- **Devlog** od dnia, kiedy jest co pokazać. Hak: "jeden człowiek, Rust, czołgi generowane proceduralnie" — sprzedaje się jednocześnie na r/WorldofTanks, r/rust i HN. Trzy publiczności, jedna historia.
- Strona Steam, wishlisty.
- Zamknięte playtesty z polskimi weteranami WoT — powiedzą w pięć minut, czy feel jest.

---

## 20. Sekwencja

**Kolejność budowy gry:** strzał → ruch → widzenie → jedna mapa z botami → reszta. Jeśli po dwóch miesiącach strzelanie do stojącego Panzera IV z dokładnym wnętrzem nie jest samo w sobie przyjemne, coś jest źle w fundamencie i żadna liczba czołgów tego nie naprawi.

**Test na dziś:** czy mogę usiąść, wjechać na mapę, strzelić do drugiego czołgu — i czy to jest przyjemniejsze niż w WoT? Jeśli nie, wszystko inne czeka.

| Etap | Zakres | Czas |
|---|---|---|
| 1. Vertical slice | 1 mapa, 8 czołgów, 7v7 z botami, feel dopięty | 3–4 mies. |
| 2. Netcode i serwer | Autorytatywny, predykcja, rewind, matchmaking | 3–4 mies. |
| 3. Steam + devlog | Strona, wishlisty, publiczność | równolegle od etapu 1 |
| 4. Playtesty | Zamknięte, weterani WoT z Polski | po etapie 2 |
| 5. Early Access | 30–40 czołgów, 4 mapy, PvE + PvP | 12–18 mies. uczciwie, czyli pewnie 24 |

---

## 21. Ryzyka

| Ryzyko | Mitygacja |
|---|---|
| Dłubanie w silniku zamiast w grze | Zamrożenie silnika (zasada 6), sekwencja z testem "czy to jest przyjemne" |
| Brak populacji na starcie | PvE-first, boty, 7v7, koncentracja EU/PL, okna gry |
| Tempo contentu solo (lekcja BattleBita) | Pipeline robi content; era jako rozszerzenie, nie ciągły grind |
| Balans zniszczalnych budynków (HE przebudowuje wieś) | Limit czasowy naturalny; telemetria; segmenty kamienne odporne |
| Netcode jako największa dziura | Czołgi to wdzięczny przypadek (wolne, 30 Hz wystarczy); etap 2 przed czymkolwiek innym |
| Głos załogi w wielu językach | 10–15 kwestii, filtr radiowy, native speakerzy ze społeczności |
| Stary sprzęt publiczności | Min spec jako decyzja strategiczna, tryb ziemniaka od dnia pierwszego |

---

## Załącznik A — parametry startowe

| Parametr | Wartość |
|---|---|
| Fizyka | 60 Hz fixed |
| Sieć | 30 Hz, interpolacja 100 ms |
| Bitwa | 7v7, 7 min, jedno życie |
| Mapa | 600×600 m, heightmapa 0,5 m |
| Rozrzut | ucięty Gauss, σ = r/3 |
| Penetracja | deterministyczna |
| Obrażenia | ±10% |
| Overmatch | kaliber > 3× grubość nominalna |
| Auto-spot | 50 m |
| Krater HE | 0,3–0,5 m głębokości |
| Decale per czołg | do 64 |
| Cienie | 4 kaskady 2048, PCF |
| MSAA | 4× |
| Min spec | GTX 1060 / RX 580, 1080p 60 fps |
| HUD budżet | 0,5 ms |
| Załoga cap | ~10 h |
| Pasmo WWII | ~100 h |
| Klasy w paśmie | 1–4, MM ±1 |
| Zoom snajpera | 2/4/8× |
| Klany | po EA |
