# One look — jeden standard obrazu na wszystkich wspieranych PC

Data: 2026-09-09. Status: **przyjęty kierunek produktu; wydajność wymaga potwierdzenia**.
Decyzja właściciela: na jego laptopie wystarczy stabilne 60 FPS; mocniejsze komputery
mają wykorzystywać zapas do większej płynności przy tym samym zamierzonym wyglądzie gry.
Autorytet: [GDD, decyzja 36](game-design.md). Kolejkę prac prowadzi
[program główny](program.md); pomiary i odbiór opisuje [plan płynności](sustained-performance-plan.md).

## Obietnica dla gracza

Każdy gracz otrzymuje tę samą estetykę, informacje potrzebne do walki i zasady widoczności.
Jakość odczuwana obejmuje stabilność obrazu w ruchu, reakcję sterowania, czytelne celowanie
i działanie do końca bitwy. Dobry pierwszy kadr ani wysoki średni FPS nie zastępują tych cech.

Podstawą jest 60 FPS na rozgrzanym laptopie referencyjnym z MX330. Na mocniejszym sprzęcie
celem jest 120 FPS i więcej, stosownie do możliwości całego komputera i monitora.
To cele do zweryfikowania, nie deklaracja obecnie osiągniętej wydajności.
Nie obiecujemy obsługi każdego historycznego PC; publikujemy zmierzone konfiguracje wspierane.

## Co musi pozostać wspólne

- Kierunek artystyczny, paleta, materiały, światło, pogoda i hierarchia czytelności sceny.
- Obecność i położenie przeszkód, osłon, roślinności wpływającej na widoczność i wraków.
- Sylwetki pojazdów i istotne części konstrukcji, zwłaszcza podczas celowania z powiększeniem.
- Reguły wykrywania, optyki, kolizji i toru pocisku oraz zgodność obrazu z tymi regułami.
- Dostępność informacji bojowych, kontrast i czytelność HUD-u; ustawienia dostępności pozostają.
- Zamierzony charakter ruchu, animacji, efektów i ich informacji o zdarzeniach.

Nie wprowadzamy presetów Low/Medium/Ultra rozdzielających graczy na różne światy.
Nie pozwalamy wyłączyć krzaków, przeszkód lub cieni dla przewagi w dostrzeganiu celu.
Zmiana pogody jest zmianą autorskiego wariantu sceny, nigdy ukrytym trybem wydajności.

## Co może się różnić

| Właściwość | Zasada |
| --- | --- |
| FPS i odświeżanie | Większa wydajność daje częstsze klatki; cel 120+ nie podnosi automatycznie częstotliwości symulacji |
| Rozdzielczość wyjściowa | Zależna od monitora i wyboru gracza; rzeczywisty viewport zapisujemy w każdym pomiarze |
| Rozdzielczość świata | Może być skalowana dopiero po wdrożeniu i odbiorze czytelności w ruchu; dziś brak zaakceptowanego systemu dynamicznej skali |
| Sposób obliczania obrazu | Culling, współdzielenie zasobów, cache i prostsze reprezentacje mogą usuwać zbędną pracę, zachowując obietnicę wizualną |
| LOD | Wspólna reguła oparta na znaczeniu i rozmiarze obiektu na ekranie; zoom musi przywracać potrzebny szczegół; bez znikających osłon i skoków sylwetki |
| Dostępność | Skala HUD-u, palety i sterowanie dopasowane do gracza, z zachowaniem informacji bojowych |

One look nie oznacza identycznych bajtów obrazu między różnymi rozdzielczościami i GPU.
Oznacza wspólny wzorzec estetyki i czytelności. Każda uproszczona reprezentacja wymaga
porównania z nim, również podczas ruchu, na dalekim celu i w lunecie.

## Stan implementacji, sprawdzony na bazie c7490826

- `renderer_api::LightingQuality::canonical()` jest wspólnym profilem; ścieżka kanoniczna
  używa 1× MSAA oraz FXAA. `WOT_QUALITY=high` i `WOT_MSAA` są nadpisaniami deweloperskimi,
  nie potwierdzonymi profilami dla gracza. Pomiar ma ujawniać wszystkie nadpisania.
- `client::loop_policy::WinitLoopDriver::set_present_hz` dopasowuje tempo do monitora
  i ogranicza je do 120 Hz. Obsługa wyższych częstotliwości pozostaje pracą do wykonania.
- Symulacja ma stały krok 60 Hz, prezentacja interpoluje. Mocniejsza karta nie usuwa
  automatycznie ograniczeń CPU, blokad ani narastającego kosztu stanu bitwy.
- Referencyjny laptop ma i5-1035G1 i MX330; podczas rozpoznania ekran pracował w 1080p/60 Hz.
  Nie jest to dowód maksymalnych możliwości monitora ani zaliczonego 1080p/60 FPS.
- Nie wdrożono w tym zakresie dynamicznej rozdzielczości świata ani nowego upscalera.
  Oddzielenie natywnego HUD-u od skali świata jest kandydatem do testów, nie gotową funkcją.

## Jak wybieramy optymalizacje

Najpierw usuwamy niewidoczną geometrię, powtórzone obliczenia, zbędne kopie, globalne
przebudowy i blokady. Następnie wyceniamy reprezentacje świata, materiały, podwozia i efekty.
Budżet zwiększamy tylko dla konkretnego elementu z pomiarem całej klatki.

Jeżeli wymagany wygląd nie mieści się w budżecie MX330, poprawiamy wykonanie albo wspólny
projekt efektu. Nie przenosimy rozwiązania problemu na gracza suwakiem usuwającym świat.
Skalowanie rozdzielczości wymaga osobnego odbioru: bez smug za lufą, migotania roślinności,
utraconych dalekich celów, rozmytego HUD-u i gwałtownego pompowania ostrości.
Generowane klatki nie zastępują pomiaru rzeczywistego renderowania i reakcji wejścia.

## Relacja do pozostałych zasad

- [Art direction](art-direction-policy.md) określa docelowy wygląd i jego zamierzone warianty
  pogody; niniejsza polityka określa wspólność tego wyglądu między komputerami.
- [Interface policy](interface-policy.md) nadal określa materiały, układ i czytelność HUD-u.
- [Engineering rules](engineering-rules.md) zachowują deterministyczną symulację i bramki zmian.
- [Plan płynności](sustained-performance-plan.md) określa konkretne kryteria wydajności.

Zmiana dokumentacji przyjmuje kontrakt i plan. Zamknięcie prac wymaga kodu, odpowiednich
testów zachowania, porównania obrazu i pomiaru pełnej bitwy na nazwanym sprzęcie.
