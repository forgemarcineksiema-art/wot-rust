# Ammunition — reference dossier

Research pass for Battle First 2.3 ("author the ammunition"), 2026-08-02. Same rubric as
`docs/vehicles/*.md`: every row carries a source, and **confidence** is how much the sources agree
(high = multiple independent, medium = single good source, low = derived or estimated).

Keyed by GUN, not by vehicle, because guns are shared — the 8.8 cm KwK 43 and the Pak 43/3 are the
same weapon, and `modules::catalog_*` is per gun.

**AUTHORED into the catalog 2026-08-02.** `GunSpec::ammo_options()` no longer multiplies one shell
by another: it collects the stock round, the special round the gun actually fielded (if any) and
the authored HE. Rows marked **GAP** are honest holes — where one blocked a number the game needs,
the value below is a stated BALANCE DECISION and says so in the catalog comment beside it.

**How HE damage was priced.** From the shell, as the decision required: filler mass with cube-root
(blast) scaling, anchored on the 85 mm O-365K's 0.741 kg of TNT at 300 HP. That is why the 8.8 cm
lands on the same 300 — it fires a 9.4 kg shell with 0.870 kg of filler, near enough the same
round — and why the 122 mm reaches 510 rather than the 546 its AP alpha used to grant it. HE
penetration is caliber/3, which is what put a 122 mm shell above an 84 mm one again.

---

## The finding that matters most: HE velocity is not 0.70 of AP

`ammo_options()` derives the HE round at `× 0.70` muzzle velocity. Measured against the real
rounds, that is roughly right for the German guns and badly wrong for the Soviet ones, because
Soviet tank HE was a full-charge round:

| gun | real HE velocity | derived (AP × 0.70) | error |
|---|---:|---:|---:|
| 85 mm ZiS-S-53 | **793 m/s** | 554 m/s | −30 % |
| 100 mm D-10T | **900 m/s** | 626 m/s | −30 % |
| 7.5 cm KwK 42 | 700 m/s | 654 m/s | −7 % |
| 8.8 cm KwK 43 | 750 m/s | 700 m/s | −7 % |

The 100 mm case is the sharp one: the real HE round leaves the muzzle **faster than the AP round**
(900 against 895), and the game flies it at 626. That is not a balance nicety — flight time and
drop are what a player leads and holds for, so a third of the velocity is a third of the lead.

---

## 7.5 cm KwK 42 L/70 — Panther II

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| Pzgr 39/42 | APCBC-HE | 6.8 kg | 925 m/s | 138 mm @ 100 m | — | panther1944.de; Jentz penetration table | high |
| Pzgr 40/42 | APCR (tungsten) | 4.75 kg | 1 120 m/s | **194 mm @ 100 m** | — | panther1944.de | high |
| Sprgr 42 | HE | 5.74 kg | 700 m/s | — | **GAP** | panther1944.de | medium |

Produced 1943 only — the tungsten shortage ended Pzgr 40 production, which is a fact about
availability rather than about the gun.

## 8.8 cm KwK 36 L/56 — Tiger I

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| Pzgr 39 | APCBC-HE | 10.2 kg | 773 m/s | 165 mm @ 100 m | — | catalog (already authored) | medium |
| Pzgr 40 | APCR (tungsten) | 7.3 kg | **GAP** | **GAP** | — | existence attested | low |
| Sprgr Patr L/4.5 | HE | 9.4 kg | 750 m/s | — | **0.870 kg** | wwiitanks.co.uk; ww2data (1.9 lb) | medium |

The L/4.5 HE round is shared with the KwK 43 and the Flak family — one shell, several guns, which
is exactly the kind of fact a per-gun multiplier cannot express.

## 8.8 cm KwK 43 L/71 — Tiger II · 8.8 cm Pak 43/3 L/71 — same weapon

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| Pzgr 39/43 | APCBC-HE | 10.4 kg | 1 000 m/s | 202 mm @ 100 m | 59 g Amatol | Jentz; en-wiki | high |
| Pzgr 40/43 | APCR (tungsten) | 7.3 kg | 1 130 m/s | **GAP @ 100 m** | — | en-wiki (Pak 43 1 030 m/s variant) | medium |
| Gr 39/43 HL | HEAT | — | — | 90 mm, flat with range | en-wiki | medium |
| Sprgr Patr L/4.5 | HE | 9.4 kg | 750 m/s | — | 0.870 kg | wwiitanks.co.uk | medium |

The HEAT round is worth noting for the doctrine argument: 90 mm flat with range, against an APCBC
that starts at 202 and bleeds. A real sidegrade, and nothing a multiplier would ever produce.

## 12.8 cm Pak 80 L/55 — Jagdtiger

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| Pzgr 43 | APCBC-HE | 28.3 kg | 920 m/s | 223 mm @ 100 m | — | Jagdtiger dossier; en-wiki (950 m/s) | medium |
| Sprgr | HE | 28.0 kg | **GAP** | — | **GAP** | en-wiki | low |
| — | APCR | **never fielded** | | | | tungsten shortage | high |

**This gun is the reason the "three slots for everyone" fallback had to go.** It fielded no
tungsten round, and the derivation hands it a fabricated APCR at 279 mm.

## 85 mm ZiS-S-53 — T-34-85

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| BR-365K | APBC-HE | 9.2 kg | 792 m/s | 125 mm @ 500 m / 30° | RDX-Al 0.16 kg | wwiitanks.co.uk; ww2data | high |
| BR-365P | APCR (tungsten) | 4.95–5.3 kg | 1 050 m/s | 136 mm @ 500 m / 30° | — | wwiitanks.co.uk; ww2data | high |
| O-365K | HE-Frag | 9.54–9.58 kg | **793 m/s** | — | **TNT 0.741–0.78 kg** | wwiitanks.co.uk; ww2data | high |

Mass disagreement between the two sources is under 1 %, which is what "high" means here.

## 100 mm D-10T / D-10T2S — T-54

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| BR-412 | APBC-HE | 15.7 kg | 895 m/s | 150 mm @ 1 000 m | TNT 0.6 kg | en-wiki D-10; ww2data | high |
| BK-5M | HEAT | — | 900 m/s | 280 mm, flat with range | — | catalog (already authored) | medium |
| OF-412 (F-412) | HE-Frag | 15.8–15.9 kg | **900 m/s** | — | **TNT 2.16 kg** | en-wiki D-10; ww2data | high |

## 122 mm D-25T — IS-3

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| BR-471B | APBC-HE | 25.0 kg | 795 m/s | 175 mm @ 100 m | RDX-Al 0.156 kg | ww2data; catalog | medium |
| OF-471 | HE-Frag | 25.53 kg | **GAP** (~800 m/s) | — | **TNT 3.605 kg** | ww2data | medium |
| — | APCR | **never fielded** | | | | — | medium |

Two-piece ammunition: this is the gun whose propellant charges ride separately, which the damage
layout already models (projectiles low in the hull, charges in the bustle).

## 84 mm 20-pounder Type A / Type B — Centurion

| round | class | mass | muzzle velocity | penetration | filler | source | confidence |
|---|---|---:|---:|---:|---:|---|---|
| APCBC | APCBC | — | 1 020 m/s | ~210 mm RHA | — | en-wiki 20-pdr | high |
| APDS | APDS (tungsten) | — | 1 465 m/s | ~300 mm RHA; 330 mm @ 910 m | — | en-wiki 20-pdr | high |
| HE | HE | **GAP** | **GAP** | — | **GAP** | en-wiki (attested, "rarely used") | low |

The APDS is already authored in the catalog. Its own source note is worth keeping: conventional
APCBC rounds "were rarely used" — the discarding-sabot round was the gun's normal load, which is
the opposite of the usual stock/special relationship and a genuine identity for this vehicle.

## 120 mm Prototype

Fictional test vehicle. No research applies; whatever it fires is a design decision, not a source.

---

## What is still missing before authoring

- **HE filler** for the 7.5 cm Sprgr 42, the 12.8 cm Sprgr and the 20-pdr HE.
- **HE muzzle velocity** for the 12.8 cm and the 20-pdr; the 122 mm OF-471 is ~800 m/s by
  convention rather than by a source that states it.
- **Pzgr 40 @ 100 m** for the KwK 36 and KwK 43 — the APCR penetration tables found so far quote
  500 m and beyond.

Six holes across twelve guns. Each is a row that says GAP rather than a number somebody invented,
which is the whole point of doing this pass before touching `ammo_options()`.

## Sources

- [panther1944.de — 7.5 cm KwK 42 ammunition](https://www.panther1944.de/index.php/en/sdkfz-171-pzkpfwg-panther/technology/75-cm-kwk-42-munition)
- [Wikipedia — 8.8 cm KwK 43](https://en.wikipedia.org/wiki/8.8_cm_KwK_43)
- [Wikipedia — 8.8 cm Pak 43](https://en.wikipedia.org/wiki/8.8_cm_Pak_43)
- [Wikipedia — 12.8 cm Pak 44](https://en.wikipedia.org/wiki/12.8_cm_Pak_44)
- [Wikipedia — D-10 tank gun](https://en.wikipedia.org/wiki/D-10_tank_gun)
- [Wikipedia — Ordnance QF 20-pounder](https://en.wikipedia.org/wiki/Ordnance_QF_20-pounder)
- [wwiitanks.co.uk — 8.8cm KwK 43 L/71 gun data](https://wwiitanks.co.uk/FORM-Gun_Data.php?I=156)
- [wwiitanks.co.uk — 85 mm ZiS-S-53 gun data](https://wwiitanks.co.uk/FORM-Gun_Data.php?I=299)
- [WW2 Equipment Data — Soviet 85mm and 100mm projectiles](http://ww2data.blogspot.com/2015/09/soviet-explosive-ordnance-85mm-and.html)
- [WW2 Equipment Data — Soviet 122mm projectiles](http://ww2data.blogspot.com/2015/10/soviet-explosive-ordnance-122mm.html)
- [WW2 Equipment Data — German 88mm projectiles](http://ww2data.blogspot.com/2017/06/german-projectiles-88mm-projectiles.html)
