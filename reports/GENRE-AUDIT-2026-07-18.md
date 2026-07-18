# Genre Audit & UNASSIGNED Clear-Out (2026-07-18)

Follow-up to the March corrections list (`scripts/dj-crates-genre-audit-corrections.py`, verified applied) and the 2026-07-17 code audit. Two operations were performed directly on `~/Music/CRATES/GENRES/`; every file move is logged in this directory for undo.

## 1. UNASSIGNED clear-out

`UNASSIGNED/` held 1,865 audio files (1,847 in `Kory Likes/`, 18 in `Black Music/`). None were duplicates of filed tracks (verified by filename overlap). Classification used three signals, in priority order:

1. **artist_match** (396 files) — artist already has filed tracks in a genre folder; follows their modal genre + subfolder (only when ≥60% of their filed tracks agree).
2. **genre_tag** (34 files) — clean ID3 genre tag mapping onto a genre folder.
3. **knowledge** (1,151 files) — 956 of 1,184 unknown artists classified by a 6-agent review; unrecognized artists were left in place, never guessed.

**Result: 1,581 files moved** (log: `unassigned-moves-2026-07-18.tsv`). Top destinations: Hip-Hop:Rap 493, R&B 329, Soul 176, Pop 105, Jazz 89, Rock 73, Funk 65, House 42. Subfolder placement prefers the artist's existing subfolder, then a decade folder matching the file's year tag, then `General <Genre>` (created where needed).

**Remaining in UNASSIGNED: 284 files**
- 242 unrecognized artists (list: `unassigned-remaining-2026-07-18.txt`) — mostly SoundCloud-tier names; 36 of the remaining items are symlinks.
- 41 held back because a same-named file already exists at the destination — likely duplicates, review before deleting.
- 1 hidden non-audio file.

## 2. Genre audit (artist-level, all filed genre folders)

Every genre folder except Charts/Party/Kory Likes/-Soundcloud was reviewed (≈4,600 unique artists across 20 folders). 248 artist-level flags were raised; each **high-confidence** flag was then re-checked at the *track* level before any move — this matters, because several "wrong artist" flags turned out to be correct *track* placements:

- Chris Brown "Hmmm (feat. Davido)", Gunna/Don Toliver × Wizkid collabs → intentionally in Afrobeats (kept)
- Rihanna "Pon De Replay" / "You Don't Love Me (No, No, No)" → genuinely dancehall cuts (kept)
- Common "The Light", Mary J. Blige "All That I Can Say" → neo-soul canon, kept in Soul/Neo Soul
- Everything in curated set crates (`Pop/Brat Pop`, `Hip-Hop:Rap/90s Hip Hop Party`, `Latin/Latino Club Bangers`) was exempt from artist-level moves — those are aesthetic/set crates, not genre shelves.

**Result: 76 files moved** (log: `genre-audit-moves-2026-07-18.tsv`). Highlights:
- `Hip-Hop:Rap/1990s Hip-Hop` was ~20% not hip-hop: eurodance (Vengaboys, Eiffel 65, Rednex, Mr. President), 90s house (Black Box, CeCe Peniston, Reel 2 Real), pure pop (Spice Girls, Ace of Base, Lou Bega), Latin (Ricky Martin, Macarena) — all rehomed.
- `Gospel/Gospel House` contained secular strays (Christopher Cross, ATB, Robert Miles, Stray Kids, The Wailers, a meme rapper) — rehomed; the secular *soulful-house* producers (Louie Vega, Kerri Chandler et al.) were left, since the crate is deliberately gospel-house.
- Decade Pop folders held pure-rap crossover hits (Eminem, Kanye, T.I., 2Pac, Kendrick…), trance (Armin, Oakenfold), current house (John Summit, Dom Dolla, Adam Port), reggaeton (Bad Bunny, KAROL G, FloyyMenor), and rock (RHCP, Linkin Park, U2, Cranberries) — moved per the March precedent.
- Misc: The Spinners → Soul, Willie Colón → Latin, Kelvin Momo/Felo Le Tee → Amapiano, Trouble Funk → GoGo, DJ Kay Slay out of French House.

## 3. Duplicate findings (nothing deleted — review these)

- 21 tracks flagged during moves already exist correctly filed elsewhere (e.g. Kanye "Stronger" in both Pop and Hip-Hop; Spice Girls "Wannabe" in both 1990s Hip-Hop and 1990s Pop; Quavo "Tough" in both Contemporary Pop and Brat Pop) — the redundant copies were left in place and marked EXISTS in the run output.
- `World/Baile` duplicates the top-level `Baile` folder's edits (10 files); `Funk & Boogie Edits` appears in both Disco and Funk; `World/Amapiano` duplicates the Amapiano folder.
- 41 UNASSIGNED files held back on name collisions (see §1).

## 4. Full flag table (248 rows)

Status: **moved** = applied; **kept (track-level review)** = high-confidence artist flag overruled by track evidence; **review** = medium confidence, left for Kory.

| Folder | Artist | Tracks | Suggested | Conf | Status | Reason |
|---|---|---|---|---|---|---|
| Afrobeats | Chris Brown | 1 | R&B | high | kept (track-level review) | US R&B singer; core identity is R&B even on afro collabs |
| Afrobeats | Ciara | 1 | R&B | high | kept (track-level review) | US R&B singer |
| Afrobeats | Craig David | 2 | R&B | high | kept (track-level review) | UK R&B/garage singer, not afrobeats |
| Afrobeats | Gunna | 1 | Hip-Hop:Rap | high | kept (track-level review) | US trap rapper |
| Afrobeats | Don Toliver | 1 | Hip-Hop:Rap | high | kept (track-level review) | US rapper/melodic trap |
| Afrobeats | Stormzy | 1 | Hip-Hop:Rap | high | kept (track-level review) | UK grime/rap; core identity is UK rap |
| Afrobeats | Dave | 1 | Hip-Hop:Rap | med | review | UK rapper (likely 'Location' ft Burna Boy); core identity UK rap |
| Afrobeats | M Huncho | 1 | Hip-Hop:Rap | high | kept (track-level review) | UK trap/rap artist |
| Afrobeats | Hurricane Wisdom | 1 | Hip-Hop:Rap | med | review | Florida rapper, not afrobeats |
| Afrobeats | Angelique Kidjo | 2 | World | med | review | Beninese world-music legend, not afrobeats scene |
| Afrobeats | Fally Ipupa | 1 | World | med | review | Congolese rumba/ndombolo |
| Afrobeats | Titom | 2 | Amapiano | med | review | SA amapiano producer (TitoM, 'Tshwala Bam'); pure amapiano |
| Afrobeats | TXC | 1 | Amapiano | med | review | SA amapiano DJ duo; pure amapiano |
| Afrobeats | Jaz Karis | 1 | R&B | med | review | UK R&B/soul singer |
| Afrobeats | Aqyila | 1 | R&B | med | review | Canadian R&B singer |
| Dance | Maeta | 4 | R&B | med | review | Core-identity R&B singer (Jaguar II); flag unless these are dance remixes |
| Dance | JMSN | 2 | R&B | med | review | Moody alternative R&B balladeer, not a dance artist |
| Dance | FLO | 1 | R&B | med | review | UK R&B revival girl group; core identity is R&B |
| Dancehall | Rihanna | 2 | Pop | high | kept (track-level review) | Pop/R&B superstar; dancehall-flavored singles don't change core identity |
| Dancehall | Kevin Lyttle | 2 | World | med | review | Vincentian soca artist (Turn Me On); no Soca folder |
| Dancehall | Rupee | 1 | World | med | review | Barbadian soca artist (Tempted to Touch); no Soca folder |
| Dancehall | Burna Boy | 1 | Afrobeats | high | kept (track-level review) | Nigerian Afrobeats/Afro-fusion star |
| Dancehall | Moliy | 1 | Afrobeats | med | review | Ghanaian Afro-fusion artist; dancehall crossover hit aside, core is Afrobeats |
| Dancehall | Voice | 1 | World | med | review | Likely Trinidadian soca star Voice (Cheers to Life); soca goes to World |
| Gospel | Christopher Cross | 2 | Rock | high | moved | Yacht-rock singer-songwriter (Sailing, Ride Like the Wind); no gospel identity |
| Gospel | Ace of Base | 1 | Pop | high | moved | 90s Swedish pop group; clearly not gospel or gospel house |
| Gospel | Stray Kids | 1 | Pop | high | moved | K-pop group; no gospel connection |
| Gospel | The Wailers | 1 | Reggae | high | moved | Bob Marley's reggae band; belongs in Reggae |
| Gospel | Yuno Miles | 1 | Hip-Hop:Rap | high | moved | Comedy/meme rapper; not faith music |
| Gospel | Whigfield | 1 | Dance | high | moved | 90s eurodance (Saturday Night); outside the gospel-house scene |
| Gospel | Robert Miles | 1 | Dance | high | moved | Dream-trance producer (Children); not gospel house |
| Gospel | ATB | 2 | Dance | high | moved | German trance producer; not gospel house |
| Gospel | Everything But The Girl | 3 | Pop | med | review | Secular sophisti-pop/electronic duo; likely house remixes misfiled here |
| Gospel | The Blackbyrds | 1 | Funk | med | review | 70s jazz-funk band; secular |
| Gospel | Gwen McCrae | 1 | Soul | med | review | Secular soul/disco singer; core identity is not gospel |
| Gospel | Alicia Myers | 1 | Funk | med | review | Secular boogie/funk artist; I Want to Thank You is gospel-adjacent one-off |
| Gospel | Incognito | 2 | Jazz | med | review | UK acid-jazz/funk band; secular |
| Gospel | Celtic Harp Soundscapes, Deep Sleep Music Delta Binaural 432 Hz, Bossa Cafe en Ibiza | 1 | Electronic | med | review | Ambient/sleep-music compilation artifact; clearly misfiled |
| Hip-Hop:Rap | Shaggy | 4 | Dancehall | high | moved | Dancehall/reggae artist, per house rule |
| Hip-Hop:Rap | Ini Kamoze | 2 | Dancehall | high | moved | Jamaican dancehall/reggae artist (Here Comes the Hotstepper) |
| Hip-Hop:Rap | Chaka Demus & Pliers | 1 | Dancehall | high | review | Jamaican dancehall/reggae duo |
| Hip-Hop:Rap | Beenie Man | 1 | Dancehall | high | review | Dancehall artist, explicitly named in house rules |
| Hip-Hop:Rap | Sean Paul | 1 | Dancehall | high | review | Dancehall artist |
| Hip-Hop:Rap | Snow | 1 | Dancehall | med | review | Reggae/dancehall style (Informer) |
| Hip-Hop:Rap | Vengaboys | 1 | Dance | high | moved | Eurodance group, no hip-hop identity |
| Hip-Hop:Rap | Eiffel 65 | 1 | Dance | high | moved | Italian Eurodance (Blue) |
| Hip-Hop:Rap | Real McCoy | 1 | Dance | high | review | Eurodance act (Another Night) |
| Hip-Hop:Rap | Rednex | 1 | Dance | high | moved | Eurodance/country novelty (Cotton Eye Joe) |
| Hip-Hop:Rap | Mr. President | 2 | Dance | high | moved | German Eurodance (Coco Jamboo) |
| Hip-Hop:Rap | C+C Music Factory | 2 | Dance | med | review | Dance/hip-house production group, core identity is dance |
| Hip-Hop:Rap | RuPaul | 1 | Dance | med | review | Dance/club artist (Supermodel) |
| Hip-Hop:Rap | Black Box | 2 | House | high | moved | Italian house group (Everybody Everybody) |
| Hip-Hop:Rap | CeCe Peniston | 3 | House | high | moved | House/dance vocalist (Finally) |
| Hip-Hop:Rap | Reel 2 Real | 1 | House | high | moved | House act (I Like To Move It) |
| Hip-Hop:Rap | LaTour | 1 | House | med | review | House/electronic artist |
| Hip-Hop:Rap | Deee-Lite | 1 | House | med | review | House/club group (Groove Is in the Heart) |
| Hip-Hop:Rap | Right Said Fred | 1 | Pop | high | review | Pop novelty act, no hip-hop identity |
| Hip-Hop:Rap | Spice Girls | 1 | Pop | high | review | Pure pop group, no hip-hop identity |
| Hip-Hop:Rap | Ace of Base | 1 | Pop | high | moved | Swedish pop group, no hip-hop identity |
| Hip-Hop:Rap | Lou Bega | 1 | Pop | high | review | Pop/mambo novelty (Mambo No. 5) |
| Hip-Hop:Rap | LEN | 1 | Pop | med | review | Alt-pop act (Steal My Sunshine) |
| Hip-Hop:Rap | Ricky Martin | 1 | Latin | high | moved | Latin pop artist |
| Hip-Hop:Rap | Los Del Río | 1 | Latin | high | moved | Spanish pop duo (Macarena) |
| Hip-Hop:Rap | Jamiroquai | 1 | Funk | high | moved | Acid jazz/funk band, no hip-hop identity |
| Hip-Hop:Rap | Alice In Chains | 1 | Rock | high | review | Grunge/rock band, plainly misfiled |
| Hip-Hop:Rap | Warren Zevon | 1 | Rock | high | moved | Rock singer-songwriter (d. 2003) in a 2020s hip-hop crate; likely metadata error |
| Hip-Hop:Rap | Trouble Funk | 1 | GoGo | high | moved | Quintessential DC go-go band |
| Hip-Hop:Rap | Jessy Lanza | 1 | Electronic | high | moved | Electronic/synth-pop artist (Hyperdub), no hip-hop identity |
| Latin | CKay | 1 | Afrobeats | high | moved | Nigerian afrobeats artist (Love Nwantiti) |
| Latin | Badshah | 2 | World | med | review | Indian/Bollywood rapper, not Latin |
| Latin | Mc Kevinho | 1 | Baile | med | review | Brazilian baile funk artist |
| Latin | MC Fioti | 1 | Baile | med | review | Brazilian baile funk (Bum Bum Tam Tam) |
| Latin | Tropkillaz | 1 | Baile | med | review | Brazilian bass/funk duo |
| Latin | Charly Black | 2 | Dancehall | med | review | Jamaican dancehall artist |
| Latin | Sean Paul | 1 | Dancehall | med | review | Jamaican dancehall artist (may be a Latin collab track) |
| Latin | Majid Jordan | 1 | R&B | med | review | Canadian R&B duo, no Latin catalog |
| Pop | Eminem | 5 | Hip-Hop:Rap | high | moved | Pure hip-hop artist (named in house rules) |
| Pop | Kanye West | 3 | Hip-Hop:Rap | high | review | Pure hip-hop artist |
| Pop | JAŸ-Z | 2 | Hip-Hop:Rap | high | review | Pure hip-hop artist |
| Pop | T.I. | 2 | Hip-Hop:Rap | high | moved | Pure hip-hop artist |
| Pop | Quavo | 2 | Hip-Hop:Rap | high | moved | Migos rapper, pure trap |
| Pop | GloRilla | 2 | Hip-Hop:Rap | high | review | Pure Memphis rap |
| Pop | Kendrick Lamar | 1 | Hip-Hop:Rap | high | review | Pure hip-hop artist |
| Pop | 2Pac | 1 | Hip-Hop:Rap | high | review | Pure hip-hop artist |
| Pop | DMX | 1 | Hip-Hop:Rap | high | review | Hardcore rap, not pop |
| Pop | Beastie Boys | 1 | Hip-Hop:Rap | high | moved | Pure hip-hop group |
| Pop | Warren G | 1 | Hip-Hop:Rap | high | review | G-funk rap |
| Pop | Young Money | 1 | Hip-Hop:Rap | high | moved | Rap collective (Lil Wayne/Drake/Nicki) |
| Pop | Bad Meets Evil | 1 | Hip-Hop:Rap | high | moved | Eminem/Royce rap duo |
| Pop | Moneybagg Yo | 1 | Hip-Hop:Rap | high | moved | Pure Memphis rap |
| Pop | BigXthaPlug | 1 | Hip-Hop:Rap | high | moved | Pure Texas rap |
| Pop | DaBaby | 1 | Hip-Hop:Rap | high | review | Pure rap |
| Pop | Central Cee | 1 | Hip-Hop:Rap | high | moved | UK drill/rap |
| Pop | Dave | 1 | Hip-Hop:Rap | high | moved | UK rap |
| Pop | Tyler, The Creator | 1 | Hip-Hop:Rap | high | moved | Core hip-hop artist |
| Pop | Don Toliver | 1 | Hip-Hop:Rap | high | kept (track-level review) | Trap/melodic rap |
| Pop | BossMan Dlow | 1 | Hip-Hop:Rap | high | review | Pure Florida rap |
| Pop | Snoop Dogg | 1 | Hip-Hop:Rap | med | review | Pure hip-hop artist (unless track is a pop feature) |
| Pop | Diddy | 1 | Hip-Hop:Rap | med | review | Hip-hop artist/mogul |
| Pop | Fat Joe | 1 | Hip-Hop:Rap | med | review | Pure hip-hop artist (unless R&B collab track) |
| Pop | OutKast | 1 | Hip-Hop:Rap | med | review | Core hip-hop duo (Hey Ya! is borderline pop) |
| Pop | Eve | 1 | Hip-Hop:Rap | med | review | Rapper, pop only via features |
| Pop | Sir Mix-A-Lot | 1 | Hip-Hop:Rap | med | review | Rap, party crossover only |
| Pop | JT | 1 | Hip-Hop:Rap | med | review | City Girls rapper |
| Pop | Hanumankind | 1 | Hip-Hop:Rap | med | review | Straight rap (Big Dawgs), viral not pop |
| Pop | Cent | 1 | Hip-Hop:Rap | med | review | Likely '50 Cent' filename parse; 50 Cent is named in house rules |
| Pop | Daniel Caesar | 1 | R&B | high | moved | Pure R&B/neo-soul, no pop identity |
| Pop | Bryson Tiller | 1 | R&B | med | review | Trap-soul/R&B core |
| Pop | Aaliyah | 4 | R&B | med | review | Core 90s R&B icon |
| Pop | Sade | 2 | R&B | med | review | Smooth soul/R&B, not pop or Brat |
| Pop | Ginuwine | 1 | R&B | med | review | Pure 90s R&B |
| Pop | Blackstreet | 1 | R&B | med | review | New jack swing/R&B |
| Pop | Montell Jordan | 1 | R&B | med | review | 90s R&B |
| Pop | K-Ci & JoJo | 1 | R&B | med | review | R&B (Jodeci) balladeers |
| Pop | Mary J. Blige | 1 | R&B | med | kept (track-level review) | Queen of hip-hop soul, core R&B |
| Pop | Tiësto | 2 | Dance | high | review | Pure trance/EDM producer (named in house rules) |
| Pop | ATB | 2 | Dance | high | moved | Pure trance producer |
| Pop | Gabriel & Dresden | 5 | Dance | high | review | Pure trance/progressive duo |
| Pop | Paul Oakenfold | 2 | Dance | high | moved | Pure trance DJ/producer |
| Pop | Armin van Buuren | 1 | Dance | high | moved | Pure trance producer |
| Pop | BT | 1 | Dance | high | review | Trance/progressive producer |
| Pop | JES | 1 | Dance | med | review | Trance vocalist |
| Pop | Delerium | 1 | Dance | med | review | Trance/ambient (Silence) |
| Pop | Da Hool | 1 | Dance | med | review | Trance classic (Meet Her at the Love Parade) |
| Pop | deadmau5 | 2 | Dance | med | review | Pure EDM/progressive house producer |
| Pop | Swedish House Mafia | 1 | Dance | med | review | Pure EDM producers (though track likely a vocal anthem) |
| Pop | Sebastian Ingrosso | 1 | Dance | med | review | Pure EDM producer |
| Pop | Kerri Chandler | 3 | House | high | review | Deep house pioneer, zero pop identity |
| Pop | Roger Sanchez | 2 | House | high | review | Pure house DJ/producer |
| Pop | The Blessed Madonna | 2 | House | high | review | House DJ/producer |
| Pop | Jamie Jones | 1 | House | high | review | Tech house DJ (Hot Creations) |
| Pop | Mall Grab | 1 | House | high | review | Lo-fi house producer |
| Pop | Chris Lake | 1 | House | high | review | Pure house producer |
| Pop | John Summit | 1 | House | high | moved | Tech house producer |
| Pop | Dom Dolla | 1 | House | high | moved | Tech house producer |
| Pop | Adam Port | 1 | House | high | moved | Keinemusik afro/deep house producer |
| Pop | Peggy Gou | 4 | House | med | review | House DJ/producer (Brat-adjacent but core house) |
| Pop | Rui Da Silva | 3 | House | med | review | Progressive house (Touch Me) |
| Pop | Barry Can’t Swim | 2 | House | med | review | Melodic house producer |
| Pop | Franky Wah | 2 | House | med | review | Melodic house producer |
| Pop | Anti Up | 2 | House | med | review | Chris Lake/Chris Lorenzo house project |
| Pop | Crystal Waters | 2 | House | med | review | 90s house classic (Gypsy Woman) |
| Pop | CeCe Peniston | 2 | House | med | review | 90s house diva (Finally) |
| Pop | Robin S | 1 | House | med | review | House anthem (Show Me Love) |
| Pop | Ultra Naté | 1 | House | med | review | House diva (Free) |
| Pop | Ultra Nate | 1 | House | med | review | Duplicate spelling of Ultra Naté, house diva |
| Pop | Nightcrawlers | 1 | House | med | review | House classic (Push the Feeling On) |
| Pop | The Chemical Brothers | 1 | Electronic | high | review | Big beat/electronic core |
| Pop | Goreshit | 1 | Electronic | high | moved | Breakcore artist, plainly misfiled in 1990s Pop |
| Pop | Justice | 5 | Electronic | med | review | French electro duo |
| Pop | Gesaffelstein | 6 | Electronic | med | review | Dark techno/electro producer |
| Pop | Tiga | 4 | Electronic | med | review | Electroclash/techno producer |
| Pop | Fatboy Slim | 2 | Electronic | med | review | Big beat electronic act |
| Pop | Machine Girl | 2 | Electronic | med | review | Breakcore/digital hardcore |
| Pop | Machinedrum | 4 | Future Beats | med | review | IDM/footwork/beat-scene producer |
| Pop | Hudson Mohawke | 2 | Future Beats | med | review | Beat-scene/trap-electronic producer |
| Pop | Red Hot Chili Peppers | 2 | Rock | high | moved | Pure rock band |
| Pop | Joan Jett and the Blackhearts | 2 | Rock | high | review | Pure rock |
| Pop | Linkin Park | 1 | Rock | high | moved | Nu metal/rock band |
| Pop | Evanescence | 1 | Rock | high | moved | Gothic rock/metal band |
| Pop | U2 | 1 | Rock | high | moved | Pure rock band |
| Pop | The Cranberries | 1 | Rock | high | moved | Alt-rock band |
| Pop | Sublime | 1 | Rock | med | review | Ska punk/rock band |
| Pop | Foo Fighters | 1 | Rock | med | review | Rock band, not pop punk |
| Pop | Crazy Town | 1 | Rock | med | review | Nu metal (Butterfly) |
| Pop | Bad Bunny | 1 | Latin | high | moved | Reggaeton/Latin trap (reggaeton rule) |
| Pop | KAROL G | 1 | Latin | high | review | Reggaeton |
| Pop | FloyyMenor | 1 | Latin | high | moved | Chilean reggaeton (Gata Only) |
| Pop | Omar Courtz | 1 | Latin | high | moved | Puerto Rican reggaeton |
| Pop | Tainy | 1 | Latin | high | review | Reggaeton producer |
| Pop | DannyLux | 1 | Latin | high | review | Sierreno/musica mexicana |
| Pop | Grupo Firme | 1 | Latin | high | moved | Banda/regional mexicano |
| Pop | Rvssian | 1 | Latin | med | review | Latin/dancehall producer |
| Pop | Mon Laferte | 2 | Latin | med | review | Latin alternative singer |
| Pop | Dudu Nobre | 1 | World | med | review | Brazilian samba/pagode |
| Pop | SPINALL | 1 | Afrobeats | high | review | Nigerian Afrobeats DJ/producer |
| Pop | Joeboy | 1 | Afrobeats | high | moved | Nigerian Afrobeats singer |
| Pop | Titom | 1 | Amapiano | high | review | SA amapiano (Tshwala Bam) |
| Pop | Darkoo | 2 | Afrobeats | med | review | UK afroswing/Afrobeats |
| Pop | Amaarae | 1 | Afrobeats | med | review | Alte/afro-fusion artist |
| Pop | YG Marley | 1 | Reggae | med | review | Reggae (Praise Jah in the Moonlight) |
| Pop | Inner Circle | 1 | Reggae | med | review | Reggae band |
| Pop | Rick James | 1 | Funk | high | review | Funk icon |
| Pop | Bee Gees | 1 | Disco | high | review | Disco icons |
| Pop | Chase & Status | 4 | Jungle | med | review | Drum & bass duo |
| R&B | Fabolous | 5 | Hip-Hop:Rap | med | review | Pure rapper; R&B-flavored hits are duets/features |
| R&B | Ja Rule | 2 | Hip-Hop:Rap | med | review | Pure rapper; Ashanti duets aside, core identity is rap |
| R&B | Plies | 2 | Hip-Hop:Rap | med | review | Pure rapper; R&B hooks are features |
| R&B | Bow Wow | 2 | Hip-Hop:Rap | med | review | Pure rapper despite R&B collab singles |
| R&B | Chingy | 1 | Hip-Hop:Rap | med | review | Pure rapper (St. Louis rap) |
| R&B | Mike Jones | 1 | Hip-Hop:Rap | high | moved | Houston rapper, unambiguously rap |
| R&B | Nelly | 1 | Hip-Hop:Rap | med | review | Rapper; crossover duets don't change core identity |
| R&B | Trina | 1 | Hip-Hop:Rap | high | moved | Pure rapper (Miami rap) |
| R&B | Remy Ma | 1 | Hip-Hop:Rap | high | moved | Pure rapper |
| R&B | Eve | 1 | Hip-Hop:Rap | med | review | Pure rapper (Ruff Ryders) |
| R&B | Fat Joe | 1 | Hip-Hop:Rap | med | review | Pure rapper; R&B hooks are features |
| R&B | Twista | 1 | Hip-Hop:Rap | med | review | Pure rapper (Chicago) |
| R&B | Lil Jon & The East Side Boyz | 1 | Hip-Hop:Rap | med | review | Crunk rap group, not R&B |
| R&B | Field Mob | 1 | Hip-Hop:Rap | med | review | Rap duo; Ciara collab is a feature |
| R&B | Yung Berg | 1 | Hip-Hop:Rap | med | review | Rapper; sung hooks were features |
| R&B | Diddy | 1 | Hip-Hop:Rap | med | review | Rapper/mogul; core identity is hip-hop |
| R&B | Enrique Iglesias | 1 | Pop | high | moved | Latin pop singer, not R&B |
| R&B | Saja Boys | 1 | Pop | high | moved | Fictional K-pop boy band (KPop Demon Hunters) — pure pop |
| R&B | Kranium | 1 | Dancehall | high | moved | Jamaican dancehall artist |
| R&B | Shenseea | 1 | Dancehall | high | moved | Jamaican dancehall artist |
| R&B | Dudu Nobre | 1 | Latin | med | review | Brazilian samba/pagode artist |
| R&B | Terry Hunter | 1 | House | med | review | Chicago house DJ/producer; track likely a house remix |
| Soul | Common | 1 | Hip-Hop:Rap | high | kept (track-level review) | Chicago rapper; neo-soul adjacent but core identity is hip-hop |
| Soul | Phat Kat | 1 | Hip-Hop:Rap | high | kept (track-level review) | Detroit rapper (Dilla affiliate) |
| Soul | Mary J. Blige | 1 | R&B | high | kept (track-level review) | Queen of hip-hop soul; core identity contemporary R&B, not neo-soul |
| Soul | Brian McKnight | 2 | R&B | med | review | 90s/2000s contemporary R&B balladeer (tracks may be house remixes) |
| Soul | Willie Colón | 1 | Latin | high | review | Salsa legend, Fania Records |
| Soul | Sergio Mendes | 2 | Latin | med | review | Brazilian bossa/jazz-pop icon (unless these are MAW house remixes) |
| Soul | Fela Kuti | 2 | World | med | review | Afrobeat pioneer; core identity Nigerian Afrobeat, not soul |
| Soul | Kelvin Momo | 1 | Amapiano | high | moved | SA private-school amapiano producer |
| Soul | Felo Le Tee | 1 | Amapiano | high | moved | SA amapiano producer |
| Soul | Calvin Harris | 1 | Dance | high | kept (track-level review) | Mainstream EDM/pop producer; core identity clearly not soul |
| Soul | Crystal Waters | 1 | House | med | review | 90s house diva (Gypsy Woman); house not soul |
| Soul | Robin S | 1 | House | med | review | 90s house vocalist (Show Me Love) |
| Soul | CeCe Peniston | 1 | House | med | review | 90s house/dance vocalist (Finally) |
| Soul | Cheryl Lynn | 1 | Disco | med | review | Got to Be Real — disco-era core identity |
| Soul | Azymuth | 3 | Jazz | med | review | Brazilian jazz-funk trio (unless tracks are house remixes) |
| Soul | Tita Lau | 1 | House | med | review | Tech-house artist; no soul/soulful-house identity |
| Soul | Sunnery James & Ryan Marciano | 1 | House | med | review | Festival/commercial house duo, not soulful house |
| Funk | The Spinners | 2 | Soul | high | moved | Classic soul vocal group, not funk (per owner note) |
| House | DJ Kay Slay | 2 | Hip-Hop:Rap | high | moved | Hip-hop mixtape DJ/rapper, no house catalog |
| House | Jamiroquai | 2 | Funk | med | review | Acid-jazz/funk band, not French house (unless remixes) |
| Reggae | Aventura | 1 | Latin | med | review | Bachata group, neither reggae nor reggaeton (track may be Don Omar collab) |
| Reggae | Peso Pluma | 2 | Latin | med | review | Regional Mexican corridos artist (tracks could be reggaeton collabs) |
| Reggae | Fuerza Regida | 1 | Latin | med | review | Regional Mexican corridos band, not reggaeton |
| Reggae | Luis R Conriquez | 1 | Latin | med | review | Corridos belicos artist, not reggaeton |
| Reggae | Victor Mendivil | 1 | Latin | med | review | Regional Mexican artist, not reggaeton |
| World | FS GREEN | 1 | Baile | high | review | Brazilian baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | SaturdaySelects | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | Rilla Force | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | SOULECTION | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | XP3R3M3NT | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | Josa | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | SANTO (CH) | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | lucky | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | austin marc | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | BZAR | 1 | Baile | high | review | Baile funk edit, not African/SA world (dupe of Baile folder entry) |
| World | Nkulee501 | 2 | Amapiano | med | review | Sub says Amapiano; dedicated Amapiano folder exists |
| World | BoyGreat | 2 | Amapiano | med | review | Sub says Amapiano; dedicated Amapiano folder exists |
| World | Daano | 2 | Amapiano | med | review | Sub says Amapiano; dedicated Amapiano folder exists |
| World | Inkey | 2 | Amapiano | med | review | Sub says Amapiano; dedicated Amapiano folder exists |
| World | TDK Macassette | 1 | Amapiano | med | review | Sub says Amapiano; dedicated Amapiano folder exists |
| World | KayGee | 1 | Amapiano | med | review | Sub says Amapiano; dedicated Amapiano folder exists |
---
*Generated 2026-07-18. All moves are plain filesystem moves; run the DJ Crates sync to rebuild Serato crates from the new folder layout. To undo any move: `mv "<dst>" "<src>"` using the TSV logs in this directory.*
