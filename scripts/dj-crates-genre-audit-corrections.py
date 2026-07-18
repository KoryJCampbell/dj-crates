"""
DJ Crates Genre Audit - Misplaced Track Corrections
Generated: 2026-03-12
Library: /Users/koryjcampbell/Music/CRATES/GENRES/

Format: (search_genre, search_subgenre_or_None, filename_regex_pattern, correct_genre, correct_subgenre, reason)

Notes:
- Crossover artists with hip-hop features/collabs are LEFT in Hip-Hop (e.g., Beyonce ft Jay-Z, Jennifer Lopez ft Ja Rule)
- R&B/Pop artists who had legitimate hip-hop crossover singles are LEFT (e.g., Ashanti ft Ja Rule, Mary J ft 50 Cent)
- Dance classics like Shannon, Lisa Lisa, Soul II Soul are LEFT in Dance (they are dance/freestyle)
- Azealia Banks is LEFT in House (she makes house-rap)
- Bootsy Collins is LEFT in Rock/Funk Rock (funk rock is appropriate)
"""

CORRECTIONS = [
    # =========================================================================
    # HIP-HOP:RAP -> R&B (Solo R&B tracks with NO hip-hop features)
    # =========================================================================
    ("Hip-Hop:Rap", None, r"Aaliyah - Miss You", "R&B", "Contemporary R&B", "Pure R&B ballad, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Alicia Keys - Karma", "R&B", "Contemporary R&B", "R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Alicia Keys - You Don't Know My Name", "R&B", "Contemporary R&B", "R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ashanti - Foolish", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ashanti - Happy", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ashanti - I Just Wanna Love You Baby", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ashanti - Only U", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ashanti - Rock Wit U \(Awww Baby\) \(Clean\)", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ashanti - Baby \(Clean\)", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Beyonce - Irreplaceable", "R&B", "Contemporary R&B", "R&B ballad, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Beyonce - Naughty Girl", "R&B", "Contemporary R&B", "R&B/dance single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Beyonce - Ring The Alarm", "R&B", "Contemporary R&B", "R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Beyonce - In Da Club", "R&B", "Contemporary R&B", "R&B single, no rap features despite title"),
    ("Hip-Hop:Rap", None, r"Black Buddafly - Bad Girl", "R&B", "Contemporary R&B", "R&B group, no hip-hop connection"),
    ("Hip-Hop:Rap", None, r"Chris Brown - Gimme That \(Main\)", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Chris Brown - Run It \(Clean\)", "R&B", "Contemporary R&B", "Solo R&B single (solo version, non-remix)"),
    ("Hip-Hop:Rap", None, r"Chris Brown - Wall To Wall", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Chris Brown - With You", "R&B", "Contemporary R&B", "R&B ballad, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Dave Hollister - Keep Lovin", "R&B", "Contemporary R&B", "R&B singer, pure R&B track"),
    ("Hip-Hop:Rap", None, r"Destiny's Child - Independent Women", "R&B", "Contemporary R&B", "R&B group, pop/R&B single"),
    ("Hip-Hop:Rap", None, r"Destiny's Child - Nasty Girl", "R&B", "Contemporary R&B", "R&B group single"),
    ("Hip-Hop:Rap", None, r"En Vogue - Riddle", "R&B", "Contemporary R&B", "R&B group, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Fantasia - When I See U", "R&B", "Contemporary R&B", "R&B singer, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Ginuwine - Hell Yeah", "R&B", "Contemporary R&B", "R&B singer, solo R&B track"),
    ("Hip-Hop:Rap", None, r"Ginuwine - There It Is", "R&B", "Contemporary R&B", "R&B singer, solo R&B track"),
    ("Hip-Hop:Rap", None, r"Jaheim - Just In Case \(Main\)", "R&B", "Contemporary R&B", "Solo R&B track, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Jaheim - Put That Woman First", "R&B", "Contemporary R&B", "Solo R&B track, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Jaheim - The Chosen One", "R&B", "Contemporary R&B", "Solo R&B track, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Monica - All Eyes On Me", "R&B", "Contemporary R&B", "R&B singer, solo R&B track"),
    ("Hip-Hop:Rap", None, r"Nivea - Dont Mess With The Radio", "R&B", "Contemporary R&B", "R&B singer, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Omarion - Ice Box \(Clean\)", "R&B", "Contemporary R&B", "Solo R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Sean Kingston - Beautiful Girls", "R&B", "Contemporary R&B", "Pop/R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Sean Kingston - Face Drop", "R&B", "Contemporary R&B", "Pop/R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Sean Kingston - Lights", "R&B", "Contemporary R&B", "Pop/R&B single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Sean Kingston - Me Love", "R&B", "Contemporary R&B", "Pop/R&B single, no hip-hop features"),

    # =========================================================================
    # HIP-HOP:RAP -> POP (Pop artists with no hip-hop connection)
    # =========================================================================
    ("Hip-Hop:Rap", None, r"Lady Gaga - Just Dance \(Clean\)", "Pop", "Dance Pop", "Pop/dance single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Nelly Furtado - Maneater", "Pop", "General Pop", "Pop single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Shontelle - T-Shirt", "Pop", "General Pop", "Pop/R&B singer from Barbados, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Weird Al Yankovic - Couch Potato", "Pop", "General Pop", "Comedy/novelty artist, not hip-hop"),
    ("Hip-Hop:Rap", None, r"Weird Al Yankovic - White & Nerdy", "Pop", "General Pop", "Comedy/novelty artist, not hip-hop"),
    ("Hip-Hop:Rap", None, r"Brian Harvey Ft Wyclef Jean - Ole Ole Ole", "Pop", "General Pop", "UK pop artist, pop single"),
    ("Hip-Hop:Rap", None, r"Brooke Hogan.*About Us.*Pop Edit", "Pop", "General Pop", "Pop single (Pop Edit version)"),
    ("Hip-Hop:Rap", None, r"Eamon - \(How Could You\) Bring Him Home", "R&B", "General R&B", "R&B singer, not a hip-hop track"),

    # =========================================================================
    # HIP-HOP:RAP -> REGGAE/DANCEHALL (Dancehall artists, solo dancehall tracks)
    # =========================================================================
    ("Hip-Hop:Rap", None, r"Aidonia - Yeah Yeah \(Usher", "Dancehall", None, "Dancehall artist, dancehall track"),
    ("Hip-Hop:Rap", None, r"Aidonia ft Chris Martin - Summer Girl", "Dancehall", None, "Dancehall artists, dancehall track"),
    ("Hip-Hop:Rap", None, r"Baby Cham - Groundsman", "Reggae", "General Reggae", "Dancehall/reggae artist"),
    ("Hip-Hop:Rap", None, r"Baby Cham - Heading To The Top", "Reggae", "General Reggae", "Dancehall/reggae artist"),
    ("Hip-Hop:Rap", None, r"Baby Cham - She's Crazy", "Reggae", "General Reggae", "Dancehall/reggae artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man - Dancehall Queen \(Hip Hop", "Dancehall", None, "Dancehall artist - even the hip-hop remix is dancehall"),
    ("Hip-Hop:Rap", None, r"Beenie Man - Gal Dem Lover", "Dancehall", None, "Dancehall artist, dancehall track"),
    ("Hip-Hop:Rap", None, r"Beenie Man - Row The Beat", "Dancehall", None, "Dancehall artist, even hip-hop remix"),
    ("Hip-Hop:Rap", None, r"Beenie Man - Street Life", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man - Toy Friend", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man feat Mya - Girls Dem Sugar", "Dancehall", None, "Dancehall artist, dancehall track"),
    ("Hip-Hop:Rap", None, r"Beenie Man Feat\. R\. Kelly - Flex", "Dancehall", None, "Dancehall artist, dancehall track"),
    ("Hip-Hop:Rap", None, r"Beenie Man Feat\. Sean Paul", "Dancehall", None, "Dancehall artists"),
    ("Hip-Hop:Rap", None, r"Beenie Man ft Akon - Girls", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man Ft Guerilla Black - Compton", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man ft Janet Jackson - Feel It Boy", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man ft Wyclef - Love Me Now", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Beenie Man with Lil' Kim - Fresh From", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Busy Signal - Hustle", "Reggae", "General Reggae", "Reggae/dancehall artist"),
    ("Hip-Hop:Rap", None, r"Busy Signal - Smoke Some", "Reggae", "General Reggae", "Reggae/dancehall artist"),
    ("Hip-Hop:Rap", None, r"Cecile - Changes", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Elephant Man - Dancing Gym", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Harry Toddler - Soul Survivor", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Ky-Mani Marley - One Time", "Reggae", "General Reggae", "Reggae artist (son of Bob Marley)"),
    ("Hip-Hop:Rap", None, r"Leftside & Esco - Tuck In", "Dancehall", None, "Dancehall artists"),
    ("Hip-Hop:Rap", None, r"Macka Diamond - Bun Him", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Munga - Take My Place", "Dancehall", None, "Dancehall artist"),
    ("Hip-Hop:Rap", None, r"Munga Feat\. Sean Paul - Got It Made", "Dancehall", None, "Dancehall artists"),
    ("Hip-Hop:Rap", None, r"Rupee - Hurricane", "Soca", None, "Soca artist from Barbados"),
    ("Hip-Hop:Rap", None, r"Rupee - Tempted To Touch \(Boomtunes", "Soca", None, "Soca artist from Barbados"),
    ("Hip-Hop:Rap", None, r"Shaggy.*Angel", "Reggae", "General Reggae", "Reggae/dancehall artist, reggae track"),
    ("Hip-Hop:Rap", None, r"Shaggy.*Sexy Lady.*Just Blaze", "Reggae", "General Reggae", "Reggae/dancehall artist"),
    ("Hip-Hop:Rap", None, r"Shaggy - Strength Of A Woman", "Reggae", "General Reggae", "Reggae/dancehall artist"),
    ("Hip-Hop:Rap", None, r"Shaggy & Barrington Levi.*Broadway", "Reggae", "General Reggae", "Reggae artists"),
    ("Hip-Hop:Rap", None, r"Shaggy Ft Akon - What Is Love", "Reggae", "General Reggae", "Reggae/dancehall artist"),
    ("Hip-Hop:Rap", None, r"Shaggy ft Rik Rok - It Wasnt Me", "Reggae", "General Reggae", "Reggae/dancehall artist"),

    # =========================================================================
    # HIP-HOP:RAP -> Other genres
    # =========================================================================
    ("Hip-Hop:Rap", None, r"Tainted Love ft Dwele - Tainted", "R&B", "General R&B", "R&B track, not hip-hop"),
    ("Hip-Hop:Rap", None, r"Boyz II Men - Pass You By", "R&B", "Contemporary R&B", "R&B group, solo R&B track"),
    ("Hip-Hop:Rap", None, r"Pretty Ricky - Push It Baby \(Clean\)\.mp3", "R&B", "Contemporary R&B", "R&B group, solo version no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Pretty Ricky - Push It Baby \(Dirty\)\.mp3", "R&B", "Contemporary R&B", "R&B group, solo version no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Pretty Ricky - Yes Sir", "R&B", "Contemporary R&B", "R&B group"),
    ("Hip-Hop:Rap", None, r"Next - Mamacita", "R&B", "Contemporary R&B", "R&B group, R&B track"),
    ("Hip-Hop:Rap", None, r"Next - Wifey", "R&B", "Contemporary R&B", "R&B group, R&B track"),
    ("Hip-Hop:Rap", None, r"Bell Biv Devoe - Da Hot Shit", "R&B", "New Jack Swing", "New Jack Swing R&B group"),
    ("Hip-Hop:Rap", None, r"Blu Cantrell - Breathe \(No Rap", "R&B", "Contemporary R&B", "R&B singer, no-rap version"),
    ("Hip-Hop:Rap", None, r"Blu Cantrell - Hit Em Up Style \(Oops\) \(Radio", "R&B", "Contemporary R&B", "R&B singer, solo version"),
    ("Hip-Hop:Rap", None, r"Jamie Foxx - Extravaganza", "R&B", "Contemporary R&B", "R&B track, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Lloyd - Get It Shawty", "R&B", "Contemporary R&B", "R&B singer, solo R&B track"),
    ("Hip-Hop:Rap", None, r"Rihanna - Rehab \(Clean\)", "R&B", "Contemporary R&B", "R&B/pop single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Rihanna - Ride", "R&B", "Contemporary R&B", "R&B/pop single, no hip-hop features"),
    ("Hip-Hop:Rap", None, r"Amerie - One Thing", "R&B", "Contemporary R&B", "R&B singer, solo R&B track"),

    # =========================================================================
    # R&B -> Correct genre (non-R&B tracks in R&B folder)
    # =========================================================================
    ("R&B", None, r"Bon Jovi - Thank You For Loving Me", "Rock", "Pop Rock", "Rock band, not R&B"),
    ("R&B", None, r"Chris Tomlin - I Lift My Hands", "Christian", None, "Christian worship artist"),
    ("R&B", None, r"Enya - May It Be", "Pop", "General Pop", "New age/Celtic artist, not R&B"),
    ("R&B", None, r"Gloria Gaynor - I Never Knew", "Disco", None, "Disco artist"),
    ("R&B", None, r"Hilary Duff - Stranger", "Pop", "General Pop", "Pop artist, not R&B"),
    ("R&B", None, r"Josh Groban - You Raise Me Up", "Pop", "Pop Ballad", "Classical crossover/pop artist"),
    ("R&B", None, r"Backstreet Boys", "Pop", "General Pop", "Boy band, pop not R&B"),
    ("R&B", None, r"Westlife", "Pop", "General Pop", "Boy band, pop not R&B"),
    ("R&B", None, r"Bette Midler - In My Life", "Pop", "Pop Ballad", "Pop artist, not R&B"),
    ("R&B", None, r"Do \(DJ Sammy\) - Heaven", "Dance", None, "Dance/trance track"),
    ("R&B", None, r"Annie Lennox - Sing", "Pop", "General Pop", "Pop/rock artist, not R&B"),
    ("R&B", None, r"Codigo Fn - El Gallero", "Latin", None, "Regional Mexican (norteno) artist"),
    ("R&B", None, r"Duelo - A Punto De Empezar", "Latin", None, "Regional Mexican (norteno) artist"),
    ("R&B", None, r"Intocable - Nadie Es Indispensable", "Latin", None, "Regional Mexican (Tejano) group"),
    ("R&B", None, r"Irasema - Chiquitito", "Latin", None, "Regional Mexican artist"),
    ("R&B", None, r"Los Tigres Del Norte", "Latin", None, "Regional Mexican (norteno) group"),
    ("R&B", None, r"Pepe Tovar Y Sus Chacales", "Latin", None, "Regional Mexican artist"),
    ("R&B", None, r"Joe Budden - Pump It Up", "Hip-Hop:Rap", "East Coast", "Hip-hop rapper, not R&B"),
    ("R&B", None, r"Lil Flip - Sunshine", "Hip-Hop:Rap", "Southern", "Hip-hop rapper"),

    # =========================================================================
    # POP -> Correct genre (Dance/EDM tracks misplaced in Pop)
    # =========================================================================
    ("Pop", None, r"Darude - (Feel The Beat|Sandstorm)", "Dance", None, "Trance/dance producer, not pop"),
    ("Pop", None, r"Daft Punk - One More Time", "Dance", None, "Electronic/dance, already has copies in Dance"),
    ("Pop", None, r"Deadmau5.*I Remember", "Dance", None, "Electronic/dance producer"),
    ("Pop", None, r"Deep Dish - Say Hello", "Dance", None, "Progressive house duo"),
    ("Pop", None, r"Dj Tiesto - Adagio For Strings", "Trance", None, "Trance DJ"),
    ("Pop", None, r"Tiesto - Traffic", "Trance", None, "Trance DJ"),
    ("Pop", None, r"Eiffel 65 - Move Your Body", "Dance", None, "Eurodance group"),
    ("Pop", None, r"Ferry Corsten - Rock Your Body Rock", "Trance", None, "Trance producer"),
    ("Pop", None, r"Global Deejays - What A Feeling", "Dance", None, "Dance/remix group"),
    ("Pop", None, r"Ian Van Dahl - (Reason|Will I|Castles)", "Trance", None, "Trance/dance producer"),
    ("Pop", None, r"Lasgo - (Alone|Something)", "Trance", None, "Trance/dance group"),
    ("Pop", None, r"Safri Duo - Played Alive", "Dance", None, "Dance/electronic duo"),
    ("Pop", None, r"Tomcraft - Loneliness", "Dance", None, "Techno/dance producer"),
    ("Pop", None, r"Warp Brothers Vs Aquagen - Phatt Bass", "Dance", None, "Hard dance/trance producers"),

    # POP -> Correct genre (Hip-Hop tracks misplaced in Pop)
    ("Pop", None, r"50 Cent - Disco Inferno", "Hip-Hop:Rap", "Bling Era", "Hip-hop artist, hip-hop track"),
    ("Pop", None, r"50 Cent - Window Shopper", "Hip-Hop:Rap", "Bling Era", "Hip-hop artist, hip-hop track"),
    ("Pop", None, r"Eminem - (Just Lose It|Lose Yourself|The Real Slim Shady|Without Me)", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop artist"),
    ("Pop", None, r"Dr Dre feat Snoop Dogg - Still D\.R\.E", "Hip-Hop:Rap", "West Coast", "Hip-hop classic"),
    ("Pop", None, r"D12 - My Band", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop group"),
    ("Pop", None, r"Cypress Hill - \(Rock\) Superstar", "Hip-Hop:Rap", "West Coast", "Hip-hop group"),
    ("Pop", None, r"Method Man.*Redman.*How High", "Hip-Hop:Rap", "East Coast", "Hip-hop artists"),
    ("Pop", None, r"Redman.*Method Man.*How High", "Hip-Hop:Rap", "East Coast", "Hip-hop artists"),
    ("Pop", None, r"Terror Squad.*Lean Back", "Hip-Hop:Rap", "East Coast", "Hip-hop group"),
    ("Pop", None, r"Busta Rhymes feat Sean Paul - Make It Clap", "Hip-Hop:Rap", "East Coast", "Hip-hop track"),
    ("Pop", None, r"Cassidy feat R Kelly - Hotel", "Hip-Hop:Rap", "East Coast", "Hip-hop artist"),

    # =========================================================================
    # ROCK -> Correct genre (non-Rock tracks in Rock)
    # =========================================================================
    ("Rock", "Pop Rock", r"Brandy - Baby", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Brandy - I Wanna Be Down", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Brandy - Best Friend", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Brandy - Brokenhearted", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Mary J\. Blige - MJB Da MVP", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Mary J\. Blige - Ain't Really Love", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Mary J\. Blige - Enough Cryin", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Mary J\. Blige - Take Me As I Am", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Mary J\. Blige - Be Without You", "R&B", "Contemporary R&B", "R&B singer, not rock"),
    ("Rock", "Pop Rock", r"Mustard - (Whole Lotta|Parking Lot|Pure Water)", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop/rap producer, not rock"),
    ("Rock", "Alternative", r"Belly - (Consuela|Might Not|Trap Phone|Man Listen|Immigration|Lullaby|Alcantara|Papyrus|What You Want|All For Me)", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop rapper (Palestinian-Canadian), not rock"),
    ("Rock", "General Rock", r"Ahmir - Welcome To My Party", "R&B", "Contemporary R&B", "R&B group, not rock"),
    ("Rock", "General Rock", r"Labrinth - Earthquake", "Pop", "General Pop", "UK pop/electronic artist, not rock"),
    ("Rock", "General Rock", r"Labrinth - Let the Sun Shine", "Pop", "General Pop", "UK pop/electronic artist, not rock"),
    ("Rock", "General Rock", r"Swift - Pull Up", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop track, not rock"),

    # =========================================================================
    # DANCE -> Correct genre (R&B/Hip-Hop/other tracks misplaced in Dance)
    # =========================================================================
    ("Dance", None, r"Whitney Houston - My Love Is Your Love\.mp3", "R&B", "Contemporary R&B", "R&B ballad, not a dance track"),
    ("Dance", None, r"Whitney Houston - I Learned From The Best", "R&B", "Contemporary R&B", "R&B ballad"),
    ("Dance", None, r"Whitney Houston - Run to You", "R&B", "Contemporary R&B", "R&B ballad"),
    ("Dance", None, r"Whitney Houston - I Will Always Love You", "R&B", "Contemporary R&B", "R&B ballad"),
    ("Dance", None, r"Whitney Houston - Heartbreak Hotel", "R&B", "Contemporary R&B", "R&B track"),
    ("Dance", None, r"Whitney Houston - I'm Every Woman \(Album", "R&B", "Contemporary R&B", "R&B track (album version, not dance mix)"),
    ("Dance", None, r"Whitney Houston - Million Dollar Bill", "R&B", "Contemporary R&B", "R&B track"),
    # Note: Whitney Houston "It's Not Right But It's Okay" and the Thunderpuss Mix are dance-appropriate
    ("Dance", None, r"Destiny's Child - Bootylicious", "R&B", "Contemporary R&B", "R&B group, R&B track"),
    ("Dance", None, r"Destiny's Child - Independent Women", "R&B", "Contemporary R&B", "R&B group, R&B track"),
    ("Dance", None, r"Destiny's Child - Emotion", "R&B", "Contemporary R&B", "R&B ballad"),
    ("Dance", None, r"Destiny's Child - Brown Eyes", "R&B", "Contemporary R&B", "R&B ballad"),
    ("Dance", None, r"Destiny's Child - Nuclear", "R&B", "Contemporary R&B", "R&B track"),
    ("Dance", None, r"Destiny's Child - No, No, No", "R&B", "Contemporary R&B", "R&B track"),
    ("Dance", None, r"Jennifer Hudson - No One Gonna Love You", "R&B", "Contemporary R&B", "R&B singer, R&B track"),
    ("Dance", None, r"Jennifer Hudson - Spotlight", "R&B", "Contemporary R&B", "R&B singer"),
    ("Dance", None, r"Jennifer Hudson - And I Am Telling You", "R&B", "Contemporary R&B", "R&B/musical theater"),
    ("Dance", None, r"Jennifer Hudson - If This Isn't Love", "R&B", "Contemporary R&B", "R&B singer"),
    ("Dance", None, r"Jennifer Hudson - Giving Myself", "R&B", "Contemporary R&B", "R&B singer"),
    ("Dance", None, r"USHER - Can U Help Me", "R&B", "Contemporary R&B", "R&B singer, R&B ballad"),
    ("Dance", None, r"Melanie Fiona - It Kills Me", "R&B", "Contemporary R&B", "R&B singer, R&B ballad"),
    ("Dance", None, r"Melanie Fiona - Give It To Me Right", "R&B", "Contemporary R&B", "R&B singer"),
    ("Dance", None, r"Des'ree - You Gotta Be", "R&B", "Contemporary R&B", "R&B/soul singer, not dance"),
    ("Dance", None, r"Jeremih - Birthday Sex.*Album Version", "R&B", "Contemporary R&B", "R&B singer, R&B track (not a dance remix)"),
    ("Dance", None, r"Tony Touch - I Wonder Why", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop DJ, hip-hop track"),
    ("Dance", None, r"Mark Ronson - Ooh Wee.*Ghostface", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop track with rappers"),
    ("Dance", None, r"Rapsody - Sojourner", "Hip-Hop:Rap", "Conscious", "Hip-hop rapper"),
    ("Dance", None, r"Gucci Mane - Party Started", "Hip-Hop:Rap", "Trap", "Hip-hop/trap rapper"),
    ("Dance", None, r"Drake - Jimmy Cooks", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop track"),
    ("Dance", None, r"Kodak Black - Too Many Years", "Hip-Hop:Rap", "Trap", "Hip-hop/trap rapper"),
    ("Dance", None, r"Jacki-O - Pussy \(Real Good\)", "Hip-Hop:Rap", "Southern", "Hip-hop rapper"),
    ("Dance", None, r"Frankee - F\.U\.R\.B", "Pop", "General Pop", "Pop novelty track"),
    ("Dance", None, r"James Brown - Say It Loud", "Soul", None, "Soul/funk classic, not dance"),
    ("Dance", None, r"James Brown - Blind Man Can See It", "Soul", None, "Soul/funk track"),
    ("Dance", None, r"Khia - My Neck, My Back", "Hip-Hop:Rap", "Southern", "Hip-hop/crunk track"),
    ("Dance", None, r"Myles Smith - Stargazing", "Pop", "General Pop", "Pop/folk artist, not dance"),
    ("Dance", None, r"Myles Smith - Wait For You", "Pop", "General Pop", "Pop/folk artist, not dance"),
    ("Dance", None, r"Gracie Abrams - Close To You", "Pop", "Indie Pop", "Indie pop artist, not dance"),
    ("Dance", None, r"Mike Posner - Bow Chicka", "Pop", "Pop Rap", "Pop/hip-hop crossover"),
    ("Dance", None, r"Mike Posner - Smoke & Drive", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop track with rappers"),
    ("Dance", None, r"Mike Posner - Still Not Over You", "Pop", "General Pop", "Pop artist, pop ballad"),
    ("Dance", None, r"Kid Cudi - Pursuit Of Happiness", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop artist, hip-hop track"),
    ("Dance", None, r"Trent Reznor and Atticus Ross - Yeah x10", "Rock", "Alternative", "Industrial/alt-rock composers"),
    ("Dance", None, r"Florence \+ The Machine - Seven Devils", "Rock", "Alternative", "Indie/alt-rock band"),

    # =========================================================================
    # REGGAE -> Correct genre (Reggaeton/Latin artists misplaced in Reggae)
    # =========================================================================
    ("Reggae", None, r"Daddy Yankee - (Tu Príncipe|Lo Que Pasó|No Me Dejes|Gasolina|King Daddy|Dale Caliente|Noche De Entierro|Ella Me Levanto|Rompe)", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"J\. Balvin - (Machika|Mi Gente|Ginza|COMO UN BEBÉ|Bobo|Doblexxó|Cosa De Locos)", "Reggaeton", None, "Reggaeton/Latin artist, not reggae"),
    ("Reggae", None, r"Don Omar - (Dile|Pobre Diabla|Dale Don Dale|Salió El Sol)", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Nicky Jam - (Hasta el Amanecer|X \(feat|En La Cama|Yo no Soy)", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Feid - (CHORRITO|CLASSY|Brickell|SORRY 4|LUNA)", "Reggaeton", None, "Reggaeton/Latin artist, not reggae"),
    ("Reggae", None, r"Wisin & Yandel", "Reggaeton", None, "Reggaeton duo, not reggae"),
    ("Reggae", None, r"Héctor.*Father", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Plan B - (Es Un Secreto|Guatauba|She Said)", "Reggaeton", None, "Reggaeton duo, not reggae"),
    ("Reggae", None, r"Tego Calde", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Ivy Queen", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Angel Y Khriz", "Reggaeton", None, "Reggaeton duo, not reggae"),
    ("Reggae", None, r"La Factoria - Perdóname", "Reggaeton", None, "Reggaeton group"),
    ("Reggae", None, r"FloyyMenor", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Ryan Castro", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Cris MJ - SI NO ES CONTIGO", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Zion (& Lennox|- Zun Da Da|- Alocate)", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Big Boy - Mis Ojos", "Reggaeton", None, "Reggaeton artist, not reggae"),
    ("Reggae", None, r"Justin Quiles - TU ROPA", "Reggaeton", None, "Reggaeton artist"),
    ("Reggae", None, r"J Alvarez - La Pregunta", "Reggaeton", None, "Reggaeton artist"),
    ("Reggae", None, r"Alex Gargolas", "Reggaeton", None, "Reggaeton producer"),
    ("Reggae", None, r"Blessed - (Mírame|SI SABE)", "Reggaeton", None, "Latin artist, not reggae"),
    ("Reggae", None, r"Tainy - COLMILLO", "Reggaeton", None, "Reggaeton/Latin producer"),
    ("Reggae", None, r"N\.O\.R\.E\. - Nothin", "Hip-Hop:Rap", "East Coast", "Hip-hop rapper, not reggae"),
    ("Reggae", None, r"N\.O\.R\.E\. - Oye Mi Canto", "Hip-Hop:Rap", "East Coast", "Hip-hop rapper (reggaeton-influenced but N.O.R.E. is hip-hop)"),
    ("Reggae", None, r"319 - Tito El Bambino - Booty", "Reggaeton", None, "Reggaeton artist"),

    # Soca artists in Reggae
    ("Reggae", None, r"KES - Savannah Grass", "Soca", None, "Soca artist from Trinidad"),
    ("Reggae", None, r"Skinny Fabulous - Famalay", "Soca", None, "Soca artist from St. Vincent"),
    ("Reggae", None, r"Voice - Alive and Well", "Soca", None, "Soca artist from Trinidad"),
    ("Reggae", None, r"Bunji Garlin - Badang", "Soca", None, "Soca artist from Trinidad"),

    # =========================================================================
    # JAZZ -> Correct genre (non-Jazz tracks in Jazz)
    # =========================================================================
    ("Jazz", None, r"The Shields - You Cheated", "Soul", None, "Doo-wop group, not jazz"),
    ("Jazz", None, r"The Dells - A Heart Is A House", "Soul", None, "R&B/soul vocal group"),
    ("Jazz", None, r"The Dells - The Love We Had", "Soul", None, "R&B/soul vocal group"),
    ("Jazz", None, r"The Dells - Oh, What A Night", "Soul", None, "R&B/soul vocal group"),
    ("Jazz", None, r"Brenton Wood - (Darlin|Baby You Got|Me And You|I Like The Way|Catch You)", "Soul", None, "Soul/R&B singer, not jazz"),
    ("Jazz", None, r"The Mellow Kings - Tonite", "Soul", None, "Doo-wop group, not jazz"),
    ("Jazz", None, r"The Dubs - Could This Be Magic", "Soul", None, "Doo-wop group, not jazz"),
    ("Jazz", None, r"Natalie Cole - Someone That I Used", "R&B", "General R&B", "R&B/pop singer (this track is R&B ballad)"),
    ("Jazz", None, r"Natalie Cole - Day Dreaming", "R&B", "General R&B", "R&B/pop singer"),
    ("Jazz", None, r"Stella\. - more parties in LA", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop/R&B track, not jazz"),
    ("Jazz", None, r"Will Downing - A Million Ways", "R&B", "Quiet Storm", "R&B/quiet storm singer"),
    ("Jazz", None, r"Ann Nesby - I Apologize", "R&B", "General R&B", "R&B/gospel singer"),
    ("Jazz", None, r"Paul Hardcastle - 19", "Dance", None, "Synth-pop/dance track, not jazz"),

    # Note: Dean Martin, Frank Sinatra, Bobby Darin are classic crooners - jazz-adjacent, leaving them
    # Note: Big Joe Turner is jump blues/early R&B - jazz-adjacent, leaving him

    # =========================================================================
    # SOUL -> Correct genre
    # =========================================================================
    ("Soul", None, r"Olivia - Bizounce", "R&B", "Contemporary R&B", "2000s R&B singer, not soul"),

    # =========================================================================
    # DISCO -> Correct genre
    # =========================================================================
    ("Disco", None, r"Kash Doll - Buss It", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop/rap artist, not disco"),

    # =========================================================================
    # SOCA -> Correct genre
    # =========================================================================
    ("Soca", None, r"KILLY - (No Sad|Doomsday|Deadtalks)", "Hip-Hop:Rap", "General Hip-Hop", "Toronto rapper, not soca"),
    ("Soca", None, r"Yaga & Mackie - Aparentemente", "Reggaeton", None, "Reggaeton duo, not soca"),

    # =========================================================================
    # LO-FI -> Correct genre
    # =========================================================================
    ("Lo-Fi", None, r"Tyler, The Creator - (SORRY NOT SORRY|KEEP DA|IFHY|ARE WE STILL)", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop artist, not lo-fi"),
    ("Lo-Fi", None, r"Red Cafe - I'm Ill", "Hip-Hop:Rap", "East Coast", "Hip-hop rapper, not lo-fi"),
    ("Lo-Fi", None, r"C-Side - Boyfriend.Girlfriend", "R&B", "Contemporary R&B", "R&B group, not lo-fi"),

    # =========================================================================
    # HOUSE -> Correct genre
    # =========================================================================
    ("House", None, r"ATL - Calling All Girls", "R&B", "Contemporary R&B", "R&B group ATL, not house music"),
    ("House", None, r"ATL - Make It Up With Love", "R&B", "Contemporary R&B", "R&B group ATL, not house music"),
    ("House", None, r"Jungle Brothers - Jimbrowski", "Hip-Hop:Rap", "Golden Age", "Hip-hop group (Native Tongues), not house"),
    ("House", None, r"Various Artists - I Smoke.*Youngbloodz", "Hip-Hop:Rap", "Southern", "Hip-hop track, not house"),

    # Note: Azealia Banks tracks LEFT in House - she makes house-rap, it's appropriate

    # =========================================================================
    # MOTOWN -> Correct genre
    # =========================================================================
    ("Motown", None, r"Profyle - (Liar|Damn)", "R&B", "Contemporary R&B", "2000s R&B group, not Motown era"),

    # =========================================================================
    # GARAGE -> Correct genre
    # =========================================================================
    ("Garage", None, r"Boxie - Let Me Show You.*Juelz", "Hip-Hop:Rap", "East Coast", "Hip-hop track ft. Juelz Santana"),
    ("Garage", None, r"Genius - One Year Later.*K Camp", "Hip-Hop:Rap", "General Hip-Hop", "Hip-hop track"),

    # =========================================================================
    # CHRISTIAN -> Correct genre
    # =========================================================================
    ("Christian", None, r"Coko - Triflin.*Eve", "R&B", "Contemporary R&B", "R&B track ft. Eve, not Christian music"),
    ("Christian", None, r"Ruben Studdard - (Sorry 2004|What If|Don't Make)", "R&B", "Contemporary R&B", "R&B singer, these are secular R&B tracks"),
]

# Summary statistics
print(f"Total corrections: {len(CORRECTIONS)}")

# Count by source genre
from collections import Counter
sources = Counter(c[0] for c in CORRECTIONS)
print("\nCorrections by source genre:")
for genre, count in sources.most_common():
    print(f"  {genre}: {count}")

# Count by destination genre
dests = Counter(c[3] for c in CORRECTIONS)
print("\nCorrections by destination genre:")
for genre, count in dests.most_common():
    print(f"  {genre}: {count}")
