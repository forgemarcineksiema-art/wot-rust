# W1: Sieć i multiplayer — podkład (PL)

Artykuł-primer dla właściciela projektu: jak działa multiplayer w grach akcji, co z tego
JUŻ jest w tym repo, jakie decyzje otwiera program W1 i co rekomendować. Doktryny
wykonawcze pozostają po angielsku; to jest tekst do rozumienia, nie do maszyn.

> **STATUS (2026-07-25).** Ten primer powstał PRZED pierwszą falą W1, więc części §3, §6, §7
> i §8 opisują rzeczy, które już weszły (protokół v36-v38, PR #300). Co jest zrobione:
> koperta `SnapshotDelivery` z ACK wejść i autorytatywnym ruchem kadłuba do rekoncyliacji
> predykcji; `session_id` w każdej wiadomości, więc szybki reconnect na tym samym gnieździe
> jest nową tożsamością i spóźnione pakiety ze starej sesji nic nie psują; lekkie `InputAck`
> między dostawami snapshotów; osobny NIEZAWODNY kanał osobistej prawdy bojowej
> (`CombatEventBatch` powtarzany do `CombatEventAck`, dedup przed dźwiękiem/FX/HUD, twarde
> zerwanie sesji przy dziurze w sekwencji zamiast cichego zgubienia trafienia); filtr
> per-klient na snapshotach po obu stronach (lokalnej i `RemoteBattleServer`); bramka
> terminalna zamiast „zombie klienta". Co NIE jest zrobione: publiczna sesja
> (discovery/relay), uwierzytelnianie graczy, lag compensation zwalidowana na ludziach,
> postawa antycheatowa, operacje dedyka — i zdjęcie tożsamości właściciela z cudzych
> pocisków/trafień (patrz `docs/spotting-policy.md`). Sekcje poniżej zostają jako zapis
> rozumowania, nie jako opis stanu.

## 1. Model, który już mamy: serwer autorytatywny

Istnieje jedna prawda o bitwie: symulacja na serwerze (deterministyczny fixed tick 60 Hz,
`SimulationState`). Klient NIE wysyła „gdzie jest mój czołg" — wysyła WEJŚCIA
(`TankCommand`: gaz, skręt, wieża, ogień). Serwer aplikuje wejścia, liczy fizykę, trafienia
i spotting, i rozsyła SNAPSHOTY pełnego stanu (protokół v38: pozycje, HP, fazy coverów,
blizny, pogoda). Konsekwencje:

- **Oszust nie może kłamać o stanie** — może najwyżej wysyłać dziwne wejścia; fizyka i tak
  liczy się u nas.
- **Late join jest darmowy** — każdy snapshot niesie CAŁY stan (od v38: cały stan, który
  temu klientowi wolno widzieć — filtr spottingu działa przed wysyłką), więc spóźniony
  klient synchronizuje się pierwszą paczką (to była świadoma decyzja, nie przypadek).
- **Nie wszystko jest snapshotem** — od v38 osobista prawda bojowa (twoje trafienia, twoje
  obrażenia) jedzie osobnym niezawodnym kanałem z ACK, bo zgubiony strzał to nie „za 50 ms
  będzie nowszy": tej informacji nie da się odtworzyć z późniejszego stanu.
- **Determinizm się opłaca** — klient przewiduje własny czołg tą samą matematyką co serwer
  (`LocalPredictor`), więc korekty po snapshotach są niemal zerowe; cudze czołgi rysujemy
  ~100 ms w przeszłości, interpolując między snapshotami (płynność mimo 20 paczek/s).

Czego model NIE rozstrzyga: jak paczki fizycznie podróżują przez internet (dziś: pętla
lokalna w jednym procesie/LAN) — i to jest właśnie W1.

## 2. Transport: dlaczego nie TCP

- **TCP** gwarantuje dostarczenie i kolejność. Cena: gdy zginie paczka nr 5, paczki 6-8
  CZEKAJĄ na jej retransmisję (head-of-line blocking). W grze objawia się to zamrożeniem i
  skokiem — a retransmitowany stary snapshot jest już bezwartościowy, bo istnieje nowszy.
- **UDP** nie gwarantuje niczego — i dlatego nic nigdy nie czeka. Fundament każdej
  poważnej gry akcji (Quake, CS, Rocket League).

## 3. „Własna niezawodność na UDP" — co się naprawdę pisze

Dane gry dzielą się na klasy o różnych potrzebach; warstewka nad UDP to obsługa tych klas:

1. **Snapshoty (unreliable, newest-wins)**: numer sekwencyjny; odbiorca ignoruje starsze od
   ostatniego widzianego. Zguba = nic, za 50 ms leci pełniejszy nowszy. Nasz format
   „snapshot = pełny stan" jest dokładnie pod to.
2. **Wejścia (redundancja zamiast retransmisji)**: każda paczka klienta niesie KILKA
   ostatnich ticków wejść; pojedyncza zguba niczego nie psuje, bo następna paczka niesie
   powtórkę. Serwer deduplikuje po numerze ticku.
3. **Zdarzenia jednorazowe (reliable)**: wejście/wyjście z bitwy, wynik, czat — wysyłaj aż
   do potwierdzenia (ack + bitfield ostatnich ~32 paczek to klasyka).
4. **Drobiazgi praktyczne**: MTU (~1200 B bezpieczne — większe paczki fragmentują się i
   giną częściej; nasz snapshot 14 czołgów + fazy się mieści, ale trzeba to ZMIERZYĆ i
   zalockować testem), keep-alive, wykrywanie zerwania po ciszy, prosty pomiar RTT.

To wszystko umiemy napisać i przetestować sami (2-3 PR-y) — ale patrz §4.

## 4. QUIC: dorosłe UDP

QUIC to protokół zbudowany NA UDP (podstawa HTTP/3; jeździ na nim YouTube), który problemy
z §3 rozwiązał raz a dobrze, dokładając rzeczy, które i tak musielibyśmy zrobić:

- **Szyfrowanie TLS 1.3 wbudowane** — gra B2P bez szyfrowania to proszenie się o kłopoty.
- **Datagramy** (rozszerzenie unreliable) — idealne na snapshoty i wejścia: gołe paczki bez
  gwarancji, zero head-of-line.
- **Strumienie niezawodne, niezależne od siebie** — na zdarzenia i czat; zguba w jednym nie
  blokuje drugiego.
- **Ogarnięte NAT-y i wędrówka połączenia** (zmiana IP klienta nie zrywa sesji).

W Rust: **`quinn`** — dojrzała, żywa implementacja. Koszt: zależność + odrobina narzutu na
handshake/szyfrowanie (pomijalna przy 20 paczkach/s).

## 5. Biblioteki w ekosystemie Rust (i czym był Amethyst)

- **`quinn`** — QUIC wprost; rekomendacja bazowa.
- **`renet`** — gotowa warstwa „gra na UDP" (kanały reliable/unreliable, popularna w środo-
  wisku Bevy), żywa; sensowna alternatywa, jeśli nie chcemy QUIC.
- **`laminar`** — historyczna warstewka niezawodności na UDP pisana dla silnika
  **Amethyst**. Amethyst (2016-2022) był pierwszym dużym, społecznościowym silnikiem gier w
  Rust — ECS (jego `specs` nauczył ekosystem wzorca, który dziś kontynuuje `bevy_ecs`,
  używany u nas w warstwie prezentacji), data-driven, ambitny — ale projekt się wypalił i
  został oficjalnie zarchiwizowany, gdy społeczność przeniosła się do Bevy. `laminar`
  osierocony razem z nim — dlatego: nie brać, mimo że koncepcyjnie robi dokładnie §3.

## 6. Zestaw narzędzi na opóźnienie (co już jest, co dojdzie)

- **Predykcja własnego czołgu** — JEST (`LocalPredictor` + korekty po snapshotach).
- **Interpolacja zdalnych** — JEST (faza w domenie ticków, odporna na jitter klatek).
- **Lag compensation przy strzale** — DO ZROBIENIA: celujesz w przeszłość (interpolacja +
  ping), więc serwer przy strzale cofa pozycje CELÓW o opóźnienie strzelca i sprawdza
  trafienie tam, gdzie cel był na jego ekranie. Decyzje z zębami: górny limit cofnięcia
  (proponuję ~200 ms — powyżej gramy „w to, co widzi serwer"), czy cofamy też fazy coverów
  (proponuję NIE — rzadkie, a tanio), test-lock: „strzał w to, co widział klient, trafia".
- **Zegary**: klient goni tick serwera z małym buforem; dryf korygowany płynnie (mamy już
  kulturę akumulatorów ticków po stronie klienta — to rozszerzenie, nie rewolucja).

## 7. Uczciwość sieciowa = anty-wallhack

**ZROBIONE (v38+).** Snapshot NIE niesie już wszystkich pozycji: `filtered_for_viewer_with_observers`
tnie go per klient (drużynowe maski spotted ∪ własne oczy, radio permitting) po obu stronach —
lokalnej i `RemoteBattleServer`. Pochodne domknięte: v44/N1 zdjęło tożsamość właściciela z cudzych
pocisków/trafień (`ShellSnapshot.owner`/`ShellImpact.owner` → `Option`, `None` dla niespotowanego
strzelca; `ShotFired` niespotowanego dropowany) — tracer/kurz zostają jako zdarzenia świata,
identyczność znika. Uczciwy dług resztkowy: `shell_id` to hash właściciela (brute-force po ~14 id
możliwy) — remap per-viewer zapisany jako przyszłość w `docs/multiplayer-production-program.md`.

## 8. Cykl życia i zgodność

Handshake już weryfikuje `map_content_hash` („obaj kompilujemy ten sam świat albo nikt nie
gra") — zostaje: wersjonowanie protokołu przy łączeniu (odrzuć starego klienta z czytelnym
komunikatem), timeouty, powrót do bitwy po zerwaniu (tożsamość sesji), podmiana gracza na
bota po rozłączeniu.

## 9. Hosting, realnie

Nasz serwer JUŻ jest headless binarką. Ścieżka najkrótsza: VPS za kilka EUR/mies., otwarty
port UDP, klienci łączą się po adresie; to wystarcza na testy z ludźmi tydzień po W1.
Dedyki „oficjalne", listy serwerów i matchmaking (OpenSkill) to W3 — świadomie osobno.

## 10. Proponowany kształt programu W1 (szkic pod przyszłą sesję)

1. PR: doktryna W1 (ten dokument → wersja EN wykonawcza + decyzje przypięte).
2. PR: transport `quinn` — połączenie, datagramy, jeden strumień reliable; test: pętla
   klient↔serwer przez prawdziwe gniazdo na localhost.
3. PR: wejścia z redundancją + dedup po ticku; test: 30% sztucznych strat pakietów, sterowanie
   pozostaje płynne (symulator strat w testach!).
4. PR: snapshoty jako datagramy newest-wins + pomiar/lock rozmiaru vs MTU.
5. PR: filtrowanie snapshotów per klient (+ dźwięki/tracery przez ten sam filtr).
6. PR: lag compensation z limitem + test-lock „trafiasz w to, co widziałeś".
7. PR: cykl życia (timeouty, bot-substytucja, wersja protokołu w handshake).
8. PR: bitwa przez internet na VPS — dowód end-to-end + liczby (RTT, straty, rozmiary).

Zasada nadrzędna bez zmian: każda obietnica z tej listy ląduje jako test, nie jako opis.
