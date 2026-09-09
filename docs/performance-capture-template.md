# Raport płynności — szablon

Skopiować do osobnego raportu dla danego eksperymentu. Nie nadpisywać tego szablonu wynikiem.
Kryteria: [plan płynności](sustained-performance-plan.md). Obraz: [one look](one-look-policy.md).
Niewykonany pomiar oznaczać **NIEZMIERZONE**, nigdy zerem lub PASS.

## Wynik

- Data, autor, wiersz Q11–Q16:
- Status: PASS / FAIL / PARTIAL / NIEZMIERZONE.
- Problem gracza i konkretny trigger:
- Zmiana oraz zmierzony wpływ na komfort:
- Kryteria zaliczone, niezaliczone i brakujące:
- Pozostałe ograniczenia / następny krok:

## Tożsamość i warunki

| Pole | Wartość |
| --- | --- |
| Commit A / B, dirty diff lub jego brak | |
| Pełna ścieżka i SHA-256 binarek; release/toolchain | |
| CPU, GPU, RAM/VRAM, OS, sterownik | |
| Zasilanie, tryb energetyczny, inne obciążenia | |
| Monitor Hz, tryb okna, skala systemu | |
| Viewport px, rozdzielczość świata i HUD-u | |
| Profil, MSAA, wszystkie WOT_* nadpisujące obraz/pomiar | |
| Present mode, ogranicznik, żądana latencja kolejki | |
| Mapa, pogoda/czas, format, pojazd gracza i skład | |
| Tryb AI lokalny / host zdalny i jego sprzęt/obciążenie | |
| Seed/recording/stan świata, kamera i sekwencja wejścia | |
| Start/koniec temperatur, zegary, throttle reasons | |
| Rozgrzewanie, stabilność termiczna i timeout chłodzenia | |
| Liczba powtórzeń, kolejność A/B i faktyczna długość walki | |

## Wyniki

Raportować osobno cały mecz, każde 60-sekundowe okno oraz nazwane zdarzenia celowania,
ostrzału i zniszczeń. Zestawić wszystkie powtórzenia, nie tylko najlepszy wynik.

| Miara | A | B | Różnica / rozrzut |
| --- | --- | --- | --- |
| Liczba klatek, aktywny czas, FPS = klatki / czas | | | |
| Odstęp klatek p50 / p95 / p99 / max | | | |
| Liczba i udział klatek >17,2 / >25 / >50 / >100 ms | | | |
| CPU: ticks/sight/host, scene, HUD, upload, render, wait | | | |
| Tiki na klatkę, czas na tik, wiek wejścia i snapshotu | | | |
| GPU: frame i każdy pass; liczba i kadencja próbek | | | |
| End-to-end input p50 / p95 / max i metoda | | | |
| Working set / private CPU; GPU payload / użycie sterownika | | | |
| Kratery, gruzy, obrażenia, efekty, obiekty i geometria | | | |
| Upload bajty/klatkę, zmienione fragmenty, długości kolejek | | | |
| Identyczne akcje/kadry: świeży vs późny stan, rozgrzany sprzęt | | | |

Nie sumować nakładających się faz CPU/GPU ani working set/private/GPU.
Opisać opóźnienie próbkowania GPU, granice czasowe i sposób liczenia percentyli.
Lista wyłączonych okresów wraz z przyczynami i surowymi danymi; zacięcia podczas walki
pozostają w statystyce. Oddzielić ładowanie od pierwszej sterowalnej klatki i aktywnej walki.

## Dowody, obraz i weryfikacja

- Surowe CSV, frame log, termika, recording, analiza i manifest: ścieżki oraz SHA-256.
- Dostępność artefaktów: repo / archiwum; lokalne `output/` samo nie jest trwałym załącznikiem.
- Kadr i nagranie: jazda, obrót, wejście w lunetę, prowadzenie dalekiego celu, strzał i zniszczenie.
- Ocena: sylwetki, roślinność, osłony, kontrast, migotanie/smugi, HUD, aktualność celownika.
- Rozdzielenie przyczyn: termika / wiek świata / kadr / CPU / GPU / kolejka / nieustalone.
- Wykonane testy/bramki i logi; pominięte kontrole nazwane wprost.
- Decyzja o wierszu w `program.md`; kto i na jakich dowodach stwierdził spełnienie kryteriów.
