#!/bin/bash
# Waits for Spotify rate limit to clear, then runs enrichment + reclassification.
# Usage: nohup ./run_when_ready.sh &

VENV="/Users/koryjcampbell/Projects/dj-crates-tools/.venv/bin/activate"
SCRIPTS="/Users/koryjcampbell/Projects/dj-crates-tools/scripts"
LOG="/Users/koryjcampbell/Projects/dj-crates-tools/scripts/enrichment.log"

source "$VENV"

echo "$(date): Waiting for Spotify rate limit to clear..." | tee "$LOG"

while true; do
    # Test if Spotify is available
    python3 -u -c "
import sys
sys.path.insert(0, '$SCRIPTS')
import spotipy
from spotipy.oauth2 import SpotifyClientCredentials
from enrich_metadata import spotify_creds
cid, secret = spotify_creds()
sp = spotipy.Spotify(auth_manager=SpotifyClientCredentials(
    client_id=cid,
    client_secret=secret,
))
r = sp.search(q='test', type='track', limit=1)
print('READY')
" 2>&1 | grep -q "READY"

    if [ $? -eq 0 ]; then
        echo "$(date): Spotify available! Starting enrichment..." | tee -a "$LOG"
        break
    fi

    echo "$(date): Still rate limited, retrying in 10 minutes..." | tee -a "$LOG"
    sleep 600
done

# Step 1: Metadata enrichment
echo "$(date): Running metadata enrichment..." | tee -a "$LOG"
python3 -u "$SCRIPTS/enrich_metadata.py" 2>&1 | tee -a "$LOG"

# Step 2: Subgenre reclassification
echo "$(date): Running subgenre reclassification..." | tee -a "$LOG"
python3 -u "$SCRIPTS/reclassify_subgenres.py" 2>&1 | tee -a "$LOG"

echo "$(date): Done!" | tee -a "$LOG"
