# Stabilna płynność przez całą bitwę

Data: 2026-09-09. Status: **OPEN — ogólna utrata płynności narastająca w trakcie bitwy**.
Autorytet: [GDD, decyzja 36](game-design.md), [one look](one-look-policy.md).
Identyfikatory i kolejność prac należą do [program.md](program.md), wiersze Q11–Q16.
Ten dokument jest planem wykonania i odbioru tych wierszy, nie drugą kolejką projektu.

## Problem i cel

Właściciel zgłasza nieprzewidywalne spadki FPS narastające w trakcie bitwy i odczuwalne
przy każdej czynności. Doprecyzowanie z 2026-09-09: celowanie było tylko przykładem
skutku, nie warunkiem wystąpienia ani wskazaniem przyczyny. Najpierw obserwujemy zwykłą
bitwę i ustalamy korelacje kosztów; kontrolowane testy kandydatów wybieramy na podstawie
danych. Jest to problem blokujący komfortową grę, a nie końcowy szlif grafiki.

Celem jest stabilne 60 FPS na rozgrzanym MX330 przez całą bitwę, przy zachowaniu one look.
120 FPS i więcej na mocniejszym sprzęcie jest następnym etapem, z osobnymi pomiarami CPU,
GPU, prezentacji i wejścia. Nie podnosimy wymagań minimalnych, aby zamknąć ten problem.

## Dowody i ich ograniczenia

Rozpoznanie kodu: baza `c7490826`. Poniższe wyniki pochodzą z lokalnych artefaktów
z **2026-09-08**, ponownie odczytanych 2026-09-09. Nie uruchomiono nowej sesji pomiarowej.
Dokładny commit i hash historycznego pliku wykonywalnego nie są potwierdzone w tych
zestawieniach; nie przypisujemy wyników bieżącemu HEAD.
Historyczny instrument zapisał 4832 klatki i 483 próbki GPU; bieżący kod ma inne limity
i kadencję opisane w sekcji narzędzi. Nie zakładamy, że dzisiejszy `WOT_FRAME_LOG`
odtwarza ten sam format i komplet danych.

| Zapis | Wynik | Co to oznacza |
| --- | --- | --- |
| `output/perf/ab-20260908-070516/summary.txt`, Bystra, krótki probe po schłodzeniu | GPU p50: scena 15,61 ms; 7v7 17,94 ms; 15v15 19,92 ms | Nawet krótki pomiar floty przekracza 16,67 ms; probe nie mierzy pełnej aktywnej bitwy |
| `output/perf/c-record.txt`, Bystra AI, autodrive, viewport 1920×1009, 180,1 s | 4832 klatki; odstęp p50 30,45 ms, p95 115,53 ms, p99 148,12 ms; średnia 483 próbek GPU 32,44 ms, w tym scene_pass 23,87 ms | Istnieją zarówno wysokie koszty GPU, jak i duże zacięcia; CPU render może zawierać oczekiwanie |
| `output/perf/c-record.gpu.csv`, średnie próbek według `at_s` | [10,30): 19,68 ms (94 próbki); [60,90): 34,11 ms (76); [120,180): 38,37 ms (145) | Czas GPU rośnie w przejeździe; to zmieniająca się scena i temperatura, nie kontrolowane A/B |
| `output/perf/c-record-20260908-214855.thermal.txt` | 74°C i maska 0x20 w wielu próbkach, np. 30/40/50/60 s | Wystąpiło SW Thermal Slowdown; sam spadek zegara bez flagi i kontekstu obciążenia nie wystarcza do tej diagnozy |

Przedziały CSV są lewostronnie domknięte, prawostronnie otwarte. Średnie GPU są średnimi
próbek, nie średnim FPS ani czasem wszystkich klatek. Nie sumujemy czasów CPU i GPU,
bo mogą pracować współbieżnie; nie przypisujemy kosztu CPU konkretnej próbce GPU bez
sprawdzenia opóźnienia zapytań i zgodności identyfikatorów klatek.

Artefakty `output/` są lokalne i ignorowane przez Git; powyższe zestawienie utrwala
historyczny punkt odniesienia, nie pełny pakiet odtwarzalności. Nowe raporty muszą zawierać
hash binarki, komplet surowych danych i sposób ich udostępnienia. Znaczenie 0x20:
[NVIDIA NVML — clocks event reasons](https://docs.nvidia.com/deploy/nvml-api/group__nvmlClocksEventReasons.html).

## Hipotezy do rozdzielenia

| Kandydat | Dowód z kodu lub logu | Następny pomiar / możliwa naprawa |
| --- | --- | --- |
| Ograniczenie termiczne | Powyższy log; Q9 w programie | Ten sam widok i stan przy różnych temperaturach; zmniejszyć stałe obciążenie GPU i sprawdzić wynik po rozgrzaniu |
| Przebudowa statyków | `client/src/app/render.rs::rebuild_cover_scene_if_dirty`: worker oddaje zmienione buckets, główny wątek scala wszystkie i wywołuje `set_terrain` | Liczba zmienionych fragmentów, bajty uploadu, czas scalenia i p99 po salwie; trwałe zasoby GPU per fragment |
| Droższy teren po ostrzale | `terrain/src/height_bounds.rs::clears` sprawdza listę kraterów i wraca do dokładnego kernela przy możliwym przecięciu wpływu krateru | Liczba kandydatów, odsetek fallbacków i czas LOS w świeżym/dojrzałym stanie; lokalny indeks i konserwatywne ograniczenia |
| Celowanie | `client/src/hud/reticle_sweep.rs::reticle_trace` używa autorytatywnego `trace_shell` | Osobno sight CPU, HUD/reticle CPU i GPU lunety; nie utożsamiać problemu podczas celowania z udowodnioną winą celownika |
| Nadrabianie tików | `client/src/app/loop_step.rs`: lokalny host i predykcja pracują przed rysowaniem | Tiki na klatkę, koszt na tik i wiek wejścia; usuwać pracę/blokady przed zmianą modelu wątków |
| Efekty i zasoby | Cząsteczki mają limit, ślady gąsienic limit 256; nie jest to dowód wycieku | Liczba aktywnych efektów, pokrycie ekranu, zniszczenia, geometria, kolejki, CPU/GPU pamięć; sprawdzić wzrost i stabilizację |

Kraterów mających znaczenie fizyczne nie wolno usuwać dla FPS. Optymalizacja zapytań musi
zachować zgodność wyniku z dokładnym kernelem, także na rantach i po zmianie stanu.
Q8c i Q10 już istnieją; ich zakres i ograniczenia opisuje [raport pamięci](perf-memory-q8c.md).
Obniżanie liczby botów nie zamyka problemu 15v15, a AI już rozkłada część decyzji w czasie.

## Przebieg prac Q11–Q16

1. **Q11 — odtwarzalność i atrybucja.** Zarejestrować pełną bitwę i segment utraty kontroli.
   Sprawdzić granice pomiarów: odstęp między klatkami, praca CPU, oczekiwanie i GPU.
   Dodać brakujące liczniki oraz odtwarzalne ruchy kamery/celownika. Wynikiem jest wskazanie
   dominującego kosztu wraz z dowodem, także gdy problem ma więcej niż jedną przyczynę.
2. **Q12 — zacięcia po zniszczeniach.** Dokończyć lokalne aktualizacje aż do zasobów GPU,
   mierząc skalę uploadu. Odbiór workerów i integracja nie mogą tworzyć nieograniczonej pracy
   w jednej klatce. Zachować zgodność świata i aktualność osłon; nie zamrażać grafiki zniszczeń.
3. **Q13 — koszt dojrzałej bitwy i celowania.** Usunąć potwierdzone skanowanie globalnego
   stanu z lokalnych zapytań, kontrolować koszt śladów, obrażeń, gruzu i efektów. Cache tylko
   z poprawnym kluczem wersji świata, kamery i celu; bez opóźnionej informacji o trafieniu.
4. **Q14 — stały koszt i temperatura (kontynuacja Q9).** Wycenić scene_pass, bliskie podwozie,
   wnętrza, materiały i pokrycie roślinnością. Zachować sylwetki i informacje bojowe.
   Dynamiczna skala świata z natywnym HUD-em jest opcją do osobnego wdrożenia i porównania,
   jeżeli bezstratne redukcje pracy nie wystarczą; nie jest zakładanym darmowym przyspieszeniem.
5. **Q15 — odbiór 60 FPS.** Pełne rozgrzane bitwy, powtarzalne celowanie i próby kolejnych
   meczów bez restartu procesu. Wynik obejmuje najgorszą mapę i wariant pogody.
6. **Q16 — skalowanie 120+.** Po stabilizacji minimum: pomiary na mocniejszym CPU/GPU,
   odpowiednim monitorze, kontrola ogranicznika 120 Hz, kolejek prezentacji i interpolacji.
   Nie zmieniać stałego kroku symulacji tylko po to, aby zwiększyć FPS.

Q11 może zmienić kolejność wykonania Q12–Q14 według zmierzonego wpływu. Taką zmianę zapisujemy
w programie wraz z dowodem. Żaden z tych wierszy nie jest zamknięty samym powstaniem dokumentu.

## Protokół rozdzielenia temperatury od wieku bitwy

W release, na zasilaczu, bez równoległych kompilacji i obcych obciążeń GPU:

| Próba | Co kontrolujemy | Co rozstrzyga |
| --- | --- | --- |
| Stały stan i widok, rozgrzewanie | Identyczna scena i kamera; log zegarów, temperatur, przyczyn ograniczeń | Koszt zmiany warunków termicznych bez przyrostu zniszczeń |
| Świeży i późny stan na rozgrzanym sprzęcie | Ten sam kadr, akcje, pogoda, rozdzielczość i porównywalne warunki termiczne | Koszt wieku świata |
| Celowanie w obu stanach | Powtarzalny obrót, wejście/wyjście z lunety, prowadzenie celu, strzał | CPU celownika, GPU zoomu i reakcja wejścia |
| Ostrzał i zniszczenia | Pojedyncze trafienie, salwa, zawalenie i jazda po zmienionym terenie | Przebudowy, uploady, efekty i najgorsze klatki |
| Pełna bitwa i kolejny mecz | Naturalne zmiany świata, bez restartu procesu między meczami | Rzeczywisty komfort, zaległe zasoby i stabilność po powrocie do garażu |

Potrzebny jest kontrolowany scenariusz odtworzenia stanów i kamery; istniejący recorder
`WOT_RECORD` sam nie dowodzi, że taki harness już działa. Autodrive nie zastępuje testu
celowania ani 15 minut aktywnej walki. Jeżeli mecz kończy się wcześniej, zapisujemy faktyczny
czas i dodajemy scenariusz utrzymujący wymagane obciążenie do pełnej długości.

Zimne A/B/A nadal służy wycenie pojedynczej zmiany. Długi pomiar po rozgrzaniu służy
odbiorowi produktu. Stare zalecenie pomiarów wyłącznie na zimno w historycznych Q7/Q9
nie wyklucza tego drugiego pomiaru. Nie porównujemy zimnego A z gorącym B jako zysku kodu.
Po kompilacji odpoczynek maszyny; timeout skryptu chłodzenia lub brak telemetrii trzeba
ujawnić, a nie nazwać taki przebieg kontrolowanym termicznie.

## Kryteria odbioru — cele, jeszcze niezaliczone

Poniższe progi są początkowym kontraktem inżynierskim tego planu, nie wcześniejszym pomiarem
ani dosłownymi liczbami podanymi przez właściciela. Zmiana progu wymaga uzasadnienia i wpisu
w raporcie; nie wolno go podnosić wyłącznie po to, aby wynik stał się zielony.

| Obszar | Cel odbioru Q15 |
| --- | --- |
| Konfiguracja | i5-1035G1 + MX330, release, profil kanoniczny; badać viewport 1280×720, 1600×900 i około 1080p; wybrać i jawnie zatwierdzić rozdzielczość minimum po pomiarze oraz kontroli czytelności |
| Budżet | 60 FPS = 16,67 ms; roboczy cel GPU p95 ≤13 ms daje zapas; osobno raportować CPU i kolejkę prezentacji |
| Rytm | p99 odstępów klatek ≤17,2 ms w każdym pełnym 60-sekundowym oknie aktywnej walki oraz w całym meczu; 17,2 to tolerancja pomiaru rytmu 60 Hz, nie zmiana budżetu renderera |
| Zacięcia | Klatki >25 ms ≤0,1% aktywnej walki; zero niewyjaśnionych klatek >50 ms; każda >50 ms ma atrybucję i ocenę wpływu; game-caused stall >50 ms blokuje odbiór |
| Późny stan | Dla tych samych kadrów/akcji na rozgrzanym sprzęcie p95 CPU i GPU późnego stanu nie gorsze o więcej niż 5% od świeżego; naturalną bitwę raportować osobno, bo jej scena się zmienia |
| Celowanie | Bez zamarzania, skokowego nadrabiania i utraty poleceń; cel p95 ruch myszy → widoczna reakcja ≤50 ms przy 60 Hz, mierzony metodą end-to-end; obecny frame log tego nie dowodzi |
| Obraz | Wspólny wzorzec one look także w ruchu i lunecie, na dalekim celu oraz tle roślinności; bez utraty osłon i informacji |
| Zakres | Wszystkie 5 map, 7v7 (420 s) i 15v15 (900 s), lokalne AI 15v15 i ścieżka zdalna osobno; przegląd pogód, pełny soak najdroższej pogody na każdej mapie |
| Powtarzalność | Minimum 3 przebiegi konfiguracji/scenariusza stanowiącego dowód zamknięcia; co najmniej 2 kolejne bitwy bez restartu procesu na najgorszym scenariuszu; ujawnić rozrzut, nie wybrać najlepszego przebiegu |

Brak wyniku dla części macierzy oznacza PARTIAL, a nie PASS. Zdarzenia ładowania, startu,
alt-tab i końca meczu opisujemy oddzielnie, zachowując surowe dane; nie odrzucamy długiej
klatki w środku walki dlatego, że narzędzie nazywa ją bake'em albo przekracza 1 sekundę.
Na etapie Q16 budżet 120 FPS wynosi 8,33 ms; roboczy cel GPU 6,5–7 ms. Wyższe częstotliwości
wymagają własnych progów rytmu i pomiaru na wyświetlaczu obsługującym dany tryb.

## Narzędzia i raport

Istniejące wejścia: `WOT_FRAME_LOG=<path>`, `WOT_AUTOBATTLE=1`, `WOT_AUTODRIVE=1`,
`WOT_EXIT_AFTER_S=<seconds>`, `WOT_RECORD=<path>`; definicje w `client/src/app/mod.rs`
i `client/src/app/session.rs`. Logger z bazy `c7490826` zachowywał tylko ostatnie
3600 klatek, pierwsze 1000 zacięć i próbuje pobrać GPU co 60. klatkę. Percentyle z tego
pierścienia nie opisują całej bitwy, a limit listy zacięć nie jest ich rzeczywistą liczbą.
Q11 musi zapewnić pełny zapis/agregację okien oraz raportować utracone próbki i narzut
instrumentu bez wprowadzania blokujących zapisów na wątku klatki. Statystyka faz odrzuca
w tej bazie klatki >1000 ms; odbiór aktywnej walki wymaga zachowania tych zdarzeń.

Pierwsza implementacja Q11 z 2026-09-09 zastępuje ten pierścień zapisem do 120 000 klatek,
zachowuje długie zacięcia i eksportuje CSV przy wyjściu. Rozdziela także diagnostyczny
odczyt GPU i poprawia granice pomiaru CPU. [Raport zakresu i pomiarów](performance-q11-capture.md)
podaje stan weryfikacji. Sam logger nie zamyka Q11 ani nie wskazuje przyczyny spadków gry.

GPU timestamps są próbkowane; raport musi podać rzeczywistą liczbę próbek.
`scripts/perf/cold-run.ps1` zapisuje termikę i pamięć procesu, a
`scripts/perf/ab.ps1` służy krótkim A/B probe'a. Budować przed przebiegiem i używać
jednoznacznie wskazanej binarki; dwa buildy nie mogą podmienić sobie badanego pliku.

Q11 uzupełnia brakujące liczniki, korelację i harness; powyższe zmienne nie zapewniają
automatycznie całej macierzy. End-to-end input wymaga osobnej metody, np. szybkiej kamery
obejmującej zdarzenie wejścia i ekran. Wewnętrzny czas obsługi myszy raportujemy osobno.

Każda zmiana zamykana jest [raportem według szablonu](performance-capture-template.md):
surowe dane, konfiguracja, porównanie, ograniczenia, kontrola obrazu i status kryteriów.
Kod przechodzi właściwe bramki z [engineering-rules.md](engineering-rules.md).
Testy deterministyczne chronią wyniki fizyki, własność zasobów i ograniczenie pracy;
niestabilnych progów czasu laptopa nie zastępujemy przypadkowym testem jednostkowym.
