# Tidalite

A retro, Winamp-flavoured desktop player for Tidal. Native Rust (egui), no browser engine, no API keys:
you log in with the normal "approve in your browser" flow. Needs an active Tidal subscription.
Unofficial: it talks to the same endpoints the Tidal apps and the open-source `tidalapi` library use.

## Get the .exe (no Rust needed on your machine)

1. Upload everything in this folder to your GitHub repo (keep `.github/workflows/build.yml`;
   if hidden folders get dropped, create that file in the GitHub web UI and paste in `build.yml.copy`).
2. Actions tab -> the `build` run -> wait ~5-10 min.
3. Download the `tidalite-windows` artifact (a zip containing `tidalite.exe`) and run it.

## Controls

| | |
|---|---|
| Space | play / pause |
| Left / Right | seek -5s / +5s |
| Z X C V B | previous, play, pause, stop, next (the classic Winamp keys) |
| A | open / close the cover viewer |
| G | colour <-> black & white cover |
| L | lyrics on / off (in the cover viewer) |
| F or F11 | fullscreen (Esc leaves it) |

Right-click a song: play next / add to queue. Right-click a row in the queue: play now / remove.
Click the little cover in the player (or the ART button) to open the cover viewer.

## Library

The LIBRARY window has tabs: MY TRACKS (your liked songs, loaded in the background, with PLAY and SHUFFLE),
LISTS, ALBUMS, ARTISTS. HOME shows Tidal's home feed.

## Shuffle and the queue

SHUFFLE (in the player, or on any album / playlist / MY TRACKS) really reorders the queue: the song playing
stays on top and everything after it is shuffled, so the QUEUE window always shows what plays next.
Turning shuffle off restores the original order. The queue is split into PLAYED / NOW PLAYING / UP NEXT, and the
status line of the player shows the next song.

## Cover viewer

The cover tilts toward your mouse (and sways gently when the mouse is elsewhere). COLOR / B&W switches the
filter (it also applies to the small cover). LYRICS shows time-synced lyrics from Tidal when available.
FULLSCREEN fills the screen.

## Diagnostics

LOG in the library window shows what the app is doing; COPY LOG puts it on the clipboard.
Everything is also saved in `%APPDATA%\tidalite\log.txt`. Your login lives in `%APPDATA%\tidalite\session.json`
(migrated automatically from the old `tidalfast` folder).

## Build it yourself

```
winget install Rustlang.Rustup
cargo build --release
target\release\tidalite.exe
```
If cargo hits an SSL/revocation error behind a corporate proxy: `set CARGO_HTTP_CHECK_REVOKE=false`.

## Files

- `src/api.rs`: Tidal login, token refresh, library, search, lyrics, stream URLs
- `src/player.rs`: audio thread (rodio) with the spectrum tap
- `src/decode.rs`: symphonia decoder (AAC/MP4, FLAC, MP3)
- `src/font.rs`: the pixel font engine (hand-made 5x7 font, upper + lower case)
- `src/main.rs`: the UI

## Known limits

- Hi-Res / DASH streams aren't supported (lossless requests may come back as 320k AAC for this login).
- Only Latin text can be shown in the pixel font; other scripts appear as "?".
- No Tidal Connect, no media-key integration yet.
