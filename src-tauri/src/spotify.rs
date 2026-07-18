// Spotify artist-genre lookup via Client Credentials flow (no user auth).
// Used as a fallback when local folder/alias matching can't classify a track.
// Results are cached in-memory for the lifetime of a scan; negative results
// are cached too so we don't re-hit the API for the same missing artist.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SpotifyArtist {
    pub name: String,
    pub genres: Vec<String>,
}

pub struct SpotifyClient {
    http: Client,
    client_id: String,
    client_secret: String,
    token: Option<(String, Instant)>,
    cache: HashMap<String, Option<SpotifyArtist>>,
}

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
    #[allow(dead_code)]
    expires_in: u64,
}

#[derive(Deserialize)]
struct SearchResp {
    artists: SearchArtists,
}
#[derive(Deserialize)]
struct SearchArtists {
    items: Vec<ArtistItem>,
}
#[derive(Deserialize)]
struct ArtistItem {
    name: String,
    #[serde(default)]
    genres: Vec<String>,
}

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> Option<Self> {
        if client_id.trim().is_empty() || client_secret.trim().is_empty() {
            return None;
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .ok()?;
        Some(Self {
            http,
            client_id,
            client_secret,
            token: None,
            cache: HashMap::new(),
        })
    }

    fn normalize(name: &str) -> String {
        let mut s = name.to_lowercase().replace(['-', '_'], " ");
        // collapse whitespace
        s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        s.trim().to_string()
    }

    fn ensure_token(&mut self) -> Result<String, String> {
        if let Some((t, at)) = &self.token {
            if at.elapsed() < Duration::from_secs(3500) {
                return Ok(t.clone());
            }
        }
        let creds = STANDARD.encode(format!("{}:{}", self.client_id, self.client_secret));
        let resp = self
            .http
            .post("https://accounts.spotify.com/api/token")
            .header("Authorization", format!("Basic {}", creds))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .map_err(|e| format!("spotify token: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("spotify token status {}", resp.status()));
        }
        let tok: TokenResp = resp.json().map_err(|e| format!("spotify token json: {}", e))?;
        self.token = Some((tok.access_token.clone(), Instant::now()));
        Ok(tok.access_token)
    }

    pub fn search_artist(&mut self, name: &str) -> Result<Option<SpotifyArtist>, String> {
        let key = Self::normalize(name);
        if key.is_empty() {
            return Ok(None);
        }
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }
        let token = self.ensure_token()?;
        let query = format!("artist:\"{}\"", key);
        let q = urlencoding::encode(&query);
        let url = format!(
            "https://api.spotify.com/v1/search?q={}&type=artist&limit=1",
            q
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .map_err(|e| format!("spotify search: {}", e))?;
        if !resp.status().is_success() {
            self.cache.insert(key, None);
            return Ok(None);
        }
        let sr: SearchResp = resp.json().map_err(|e| format!("spotify json: {}", e))?;
        let artist = sr.artists.items.into_iter().next().map(|a| SpotifyArtist {
            name: a.name,
            genres: a.genres,
        });
        self.cache.insert(key, artist.clone());
        Ok(artist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_and_collapses() {
        assert_eq!(SpotifyClient::normalize("Whitney-Houston"), "whitney houston");
        assert_eq!(SpotifyClient::normalize("  Jay   Z  "), "jay z");
        assert_eq!(SpotifyClient::normalize("J_Dilla"), "j dilla");
    }

    #[test]
    fn empty_creds_yield_no_client() {
        assert!(SpotifyClient::new("".into(), "x".into()).is_none());
        assert!(SpotifyClient::new("x".into(), "".into()).is_none());
    }
}
