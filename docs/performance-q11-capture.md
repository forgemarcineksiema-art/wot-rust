# Q11 — zapis narastającej utraty płynności

Data: 2026-09-09. Status: **IN PROGRESS**. Zakres: pierwszy etap Q11, instrument pomiarowy.
Polityka: [one look](one-look-policy.md). Kryteria: [plan płynności](sustained-performance-plan.md).

## Doprecyzowanie problemu

Właściciel potwierdził, że spadki FPS są odczuwalne przy każdej czynności. Celowanie było
przykładem skutku. Najpierw rejestrujemy zwykłą bitwę, a potem wybieramy testy przyczyn
z danych. Nie zakładamy, że wywołuje je celownik, liczba kraterów albo temperatura.

## Zmiana instrumentu

- Do 120 000 kolejnych klatek w uprzednio zaalokowanej pamięci, bez nadpisywania początku.
  Wystarcza na 15 minut przy 120 FPS albo ponad 30 minut przy 60 FPS. Po przekroczeniu
  limitu raport podaje liczbę niezachowanych klatek; taki zapis jest niekompletny.
- Wszystkie zacięcia zachowane w tym samym zakresie, także po dawnym limicie 1000 wpisów.
  Średnie faz uwzględniają również klatki dłuższe niż sekundę. Pierwszy interwał i przejścia
  nadal wymagają klasyfikacji; nie wolno automatycznie odrzucać zacięć aktywnej walki.
- Czas całkowity biegnie od końca jednej próby renderowania do końca następnej. Stary kod
  przekazywał odstęp między początkami renderowania razem z fazami kończącymi się później,
  co mogło błędnie przypisać długą pracę do krótkiego interwału.
- GPU readback ma osobną fazę CPU `gpu_readback`. `WOT_FRAME_GPU=0` wyłącza profiler
  GPU i odczyty; domyślnie nadal próba co 60 klatek. Raport podaje próby i zwrócone wyniki.
  Brak wyniku nie jest próbką o zerowym koszcie.
- Próbki zawierają liczby kraterów, aktywnych cząsteczek, instancji pojazdów i sceny oraz
  wierzchołków FX. Są to liczniki pracy, nie księga pamięci ani automatyczna diagnoza.
- Usunięto osobny `tracing::info!` dla każdego zacięcia. Zapis plików odbywa się dopiero
  przy normalnym wyjściu; awaria procesu przed wyjściem może utracić buforowany zapis.
- Raport obejmuje całość zachowanego przebiegu i okna 60-sekundowe według czasu zakończenia.
  CSV umożliwia policzenie innych okien i sprawdzenie wszystkich faz.

Nie zmieniono fizyki, reguł widoczności, grafiki ani częstotliwości symulacji. Poprawa
instrumentu nie jest dowodem poprawy FPS. Pomiar CPU końca renderowania nie jest fizycznym
momentem wyświetlenia obrazu, a timestamp GPU opisany jako `observed_at_s` jest chwilą
odbioru próbki; bez dodatkowej korelacji nie przypisujemy go do konkretnego wejścia myszy.

## Użycie

Najpierw ukończyć build release i dać maszynie odpocząć. Uruchomić wskazaną, zahashowaną
binarkę; w manifeście zapisać commit oraz ewentualny dirty diff. Przykładowe zmienne:

```powershell
$env:WOT_MAP = 'bystra-valley'
$env:WOT_FRAME_LOG = 'output/perf/q11-session.txt'
$env:WOT_AUTOBATTLE = '1'
$env:WOT_AUTODRIVE = '1'
$env:WOT_EXIT_AFTER_S = '900'
$env:WOT_FRAME_GPU = '1'
```

Utworzyć wcześniej katalog wyniku. Ścieżka binarki zależy od `CARGO_TARGET_DIR`; samo
ustawienie zmiennych niczego nie uruchamia. Obserwacja z autodrive ma być tak nazwana:
nie udaje ręcznej rozgrywki i nie gwarantuje 900 sekund aktywnej bitwy, gdy mecz skończy
się wcześniej. Z `WOT_FRAME_GPU=0` można kontrolować wpływ blokujących odczytów.

Wynik dla `q11-session.txt`: raport TXT, `q11-session.frames.csv` i `q11-session.gpu.csv`.
Termikę oraz pamięć procesu zbiera osobno `scripts/perf/cold-run.ps1`. Raport musi wskazać
rzeczywisty viewport i temperatury; nie traktować chłodniejszego przebiegu jako zysku kodu.

## Weryfikacja i wynik

Instrument przeszedł `scripts/preflight.ps1`, `scripts/verify-pr.ps1 -Crates client`
oraz build release. Pełnego `scripts/verify.ps1` nie wykonano w tym etapie.

### Dwa krótkie przebiegi 2026-09-09

Oba uruchomiono z tej samej binarki release `output/perf/q11-client.exe`, SHA-256
`17302F3FEF32A45899B58C0B7767ACC9BB7DCF797756439F9ABA0AD394D42D37`.
Baza to `ddfa1306` (dokumenty na master `c7490826`) plus niezatwierdzone zmiany
instrumentu. Manifest, diff i kopia nowego modułu znajdują się w `output/perf/`.
Mapa Bystra Valley, AI 15v15, autodrive, rzeczywisty viewport 1920×1009.

| Zapis | GPU profiler | Czas zapisu | p50 / p95 / p99 interwału |
|---|---|---|---|
| `q11-session` | włączony, 39 odczytów | 67,6 s | 20,44 / 36,02 / 75,90 ms |
| `q11-no-gpu` | wyłączony, 0 odczytów | 56,3 s | 18,85 / 34,26 / 42,92 ms |

Statystyki zawierają pierwszy interwał rozruchu (odpowiednio 10,65 i 6,95 s).
Oba zapisy są częściowe, bez utraconych próbek. Pierwszy właściciel zamknął ręcznie,
ponieważ gra była niegrywalna. W drugim zgłosił wyraźnie płynniejszy obraz i odczyty
licznika około 70 FPS, spadające do 45–50 podczas intensywnej wymiany ognia.
To obserwacja właściciela; licznik HUD i percentyle interwałów nie są tą samą miarą.

W pierwszym zapisie wystąpiły interwały aktywnej gry 300–675 ms, często z dużym czasem
w wywołaniu hosta symulacji. Są to czasy ścienne, nie dowód samego kosztu obliczeń CPU.
Liczba kraterów pozostawała zerowa. W drugim najdłuższy interwał po pierwszych dwóch
klatkach miał 61,75 ms; większość zacięć miała największy koszt w fazie renderowania.
GPU w obu przebiegach osiągnęło około 74°C; poprawy odczuwanej płynności nie tłumaczy
więc samo nieosiągnięcie tej temperatury. Rozkład zegarów i przebieg bitwy były różne.

Profiler używa blokującego `device.poll(wait_indefinitely())`; jego wyłączenie jest
istotnym tropem, ale te dwa przebiegi nie izolują wszystkich zmiennych ani nie dowodzą,
że profiler wywołał wszystkie wcześniejsze zacięcia. Pierwszy zapis zawiera również
zdarzenie utraty urządzenia audio. Do dalszej oceny grywalności używać
`WOT_FRAME_GPU=0`; próbki GPU zbierać osobno i jawnie oznaczać ingerencję pomiaru.

### Osobna poprawka terminu wybudzenia

Po drugim przebiegu znaleziono dodawanie pełnego oczekiwania do czasu PO wykonaniu
akcji symulacji. Termin wybudzenia jest teraz zakotwiczony w chwili, z której pochodzi
czas przekazany do `AboutToWait`; praca zużywa pozostały budżet zamiast przesuwać termin.
Test sprawdza 5 ms pracy w budżecie 16,67 ms oraz przekroczenie terminu.
Ta poprawka NIE znajdowała się w żadnym z powyższych przebiegów i nie wyjaśnia różnicy.
Po poprawce `scripts/verify-pr.ps1 -Crates client` przeszedł (w tym test terminu wybudzenia;
633 testy jednostkowe klienta zaliczone, 1 istniejący test wizualny pominięty).
Build release z tą poprawką i porównanie sprzętowe pozostają do wykonania;
nie przypisujemy jej jeszcze zysku FPS ani temperatury.

Q11 pozostaje otwarte. Wymagane są powtarzalne porównania oraz pełna bitwa na rozgrzanym
laptopie. Stabilne 60 FPS i ograniczenie zbędnego obciążenia nie zostały jeszcze osiągnięte.
