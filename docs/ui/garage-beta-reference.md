# Garage — World of Tanks Closed/Open Beta Reference (2010–2011)

Design-reference note for the garage / out-of-battle screen. This is **historical
research**, not a spec: it records what the original WoT beta garage looked like and
which of its ideas are worth borrowing for our (intentionally *not* 1:1) tank game.

## Timeline (sources disagree — both recorded)

- **RU closed beta:** 30 January 2010 (per Wikipedia).
- **RU open beta:** 24 June 2010, ~7 maps, 60+ Soviet/German vehicles (per Wikipedia).
- **International closed beta:** announced/screened 12–13 July 2010 (Worthplaying gallery).
- **Full release:** 12 April 2011.

The exact closed-vs-open boundary differs between the RU and international tracks, so
treat the 2010–2011 window as one continuous "beta-era garage" for design purposes.

## What the beta garage actually looked like

- **Single static 3D scene.** The owned tank sat in an enclosed industrial **hangar**
  (concrete floor, metal walls, roof skylights) on a turntable-style spot. The camera
  orbited the vehicle; the world behind it was a fixed prop set, not a live map.
- **Vertical tech trees.** The single most era-defining UI trait: research trees scrolled
  **top-to-bottom**, not left-to-right as after release. A community mod later existed
  purely to restore the vertical layout. (rykoszet.info)
- **No carousel.** The modern horizontal tank carousel did not exist yet; vehicle choice
  ran through the tree / a list rather than a scrubbable strip of tanks.
- **Two nations at first.** August 2010: only USSR and Germany; tier X was Maus and IS-7.
  USA (T30) arrived late 2010.
- **More modules on low tiers.** Tier I vehicles offered a *wider* choice of guns and
  engines than the released game — early design leaned into customization, then trimmed it.
- **Garage feature set:** module fitting (turret, gun, hull/chassis, engine, radio),
  ammunition selection, crew with leveling, "endless" tank storage, and the Battle button.
- **Matchmaking context:** spread of roughly **±4 tiers**, so the garage was where you
  prepared for very lopsided fights — a different framing than today's tight brackets.

## Archival visual sources (verified pointers, not embedded)

Text articles describe the era but the *look* is best confirmed from images/video:

- **Worthplaying** — closed beta screenshot gallery, July 2010:
  https://worthplaying.com/article/2010/7/13/news/75509-world-of-tanks-rolls-out-closed-beta-test-screens/
- **MobyGames** — WoT (Windows, 2011) screenshots incl. an A-20 in a premium-account
  garage: https://www.mobygames.com/game/52092/world-of-tanks/screenshots/windows/
  (Note: returns HTTP 403 to automated fetch — open in a browser.)
- **YouTube** — search "World of Tanks closed beta 2010 gameplay" for live hangar footage
  (not linked to a stable URL here; archival channels rotate).

## Takeaways for our garage

1. **Static 3D hangar + orbit camera** is a cheap-but-atmospheric pattern that maps
   directly onto our existing wgpu renderer: one vehicle model + one lit interior, no map
   streaming. Strong candidate for the first out-of-battle screen. See [renderer milestone].
2. **Vertical tech tree** is the signature beta-era flourish. Worth a deliberate decision:
   adopt it for retro identity, or go horizontal/carousel (which won out for good ergonomic
   reasons). This is a *choose-on-purpose* fork, not a default.
3. **Generous low-tier module choice** signals that early WoT valued customization depth —
   a useful contrast point when we decide how much our game diverges from 1:1 WoT.

## Sources

- World of Tanks – zamknięte beta testy (rykoszet.info):
  https://rykoszet.info/2017/05/22/world-of-tanks-zamkniete-beta-testy/
- World of Tanks na przestrzeni lat (rykoszet.info):
  https://rykoszet.info/2018/08/11/world-of-tanks-na-przestrzeni-lat/
- World of Tanks Rolls Out Closed Beta Test – Screens (Worthplaying):
  https://worthplaying.com/article/2010/7/13/news/75509-world-of-tanks-rolls-out-closed-beta-test-screens/
- World of Tanks – Wikipedia: https://en.wikipedia.org/wiki/World_of_Tanks
- World of Tanks – PCGamingWiki: https://www.pcgamingwiki.com/wiki/World_of_Tanks
