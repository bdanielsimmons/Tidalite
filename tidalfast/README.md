# Tidalite

A retro, Winamp-flavoured desktop player for Tidal. Native Rust (egui), no browser engine, no API keys:
you log in with the normal "approve in your browser" flow. Needs an active Tidal subscription.
Unofficial: it talks to the same endpoints the Tidal apps and the open-source `tidalapi` library use.

## Get the .exe (no Rust needed on your machine)

1. Upload everything in this folder to your GitHub repo (keep `.github/workflows/build.yml`;
   if hidden folders get dropped, create that file in the GitHub web UI and paste in `build.yml.copy`).
2. Actions tab -> the `build` run -> wait ~5-10 min.
3. Download the `tidalite-windows` artifact (a zip containing `tidalite.exe`) and run it.

## v8 STUDIO: the practice studio

The library now has five lists: **TIDAL | FILES | YT | TUNES | DIARY**.

- **FILES**: drag audio files or folders onto the window (or ADD FILES / ADD FOLDER). Nothing is copied.
  Folders are remembered; RESCAN picks up new files. Loops, slow-down and everything else work on them.
- **YT** (YouTube): press GET YT-DLP once (downloads the free yt-dlp tool from its GitHub page), paste a link, ADD.
  First play downloads only the audio (AAC) and stores it; after that it starts instantly and loops like anything else.
  Downloading from YouTube may go against its terms; personal practice use is at your discretion.
- **TUNES**: your repertoire. Each tune has a status (LEARNING / WORKING / READY), key, tempo, notes and the
  *recordings worth studying* (Tidal, files or YouTube) with 1-5 stars and a note each.
  Right-click any song -> "Add to a tune...". LOOK UP (only when you press it) fetches the composer from
  MusicBrainz and lists other recordings on Tidal; right-click one to add it.
- **DIARY**: practice time is counted automatically while you are in PRACTICE mode. Daily goal bar,
  week dots, a gentle streak (today not yet practiced never breaks it), per-tune time, and a journal (LOG PRACTICE).
- **CONTINUE** button at the top of the library resumes the last track, position and speed.

### Practice panel
- Waveform timeline. Click to jump, **Shift + drag** to select a loop.
- **SAVE LOOP** keeps named A-B loops per track (chips below; click to go, right-click to delete).
- **MORE** opens: speed **TRAIN** (AUTO speeds up every N loops, EARNED speeds up after N clean passes you confirm with CLEAN / **K**),
  **PITCH** transpose without changing speed, **EAR** modes (left, right, mono, no-center, bass only),
  **EXPORT WAV** of the loop (Music/Tidalite loops), **METRO** + BPM + **TAP** tempo, **COUNT** in (2 or 4 clicks before each loop pass),
  and a **TIMER** (focus block, then a break, with a chime; time shows in the window title).
- **STEMS**: GET STEMS TOOL downloads (once, ~170 MB) the ONNX Runtime library and the HT-Demucs model from
  Hugging Face / GitHub. Then SPLIT THIS TRACK separates a stored track into drums, bass, other (guitar, keys, horns)
  and vocals; they are saved on disk (DISK view shows the size, CLEAR STEMS deletes them). Afterwards the pixel icons
  (drums, bass, other, vocals) switch each stem on or off; right-click an icon to hear only that one.
  Loops, slow-down, pitch and EQ all keep working on the mix.
- **FOCUS** (chip on the player, or in MORE) hides the lists: just the player, your tools and the chart.

### Lead sheet
Right-bottom tab **CHART** (next to QUEUE): the chord changes of the open tune as a static grid, with
CONCERT / Bb / Eb / F transposition. EDIT lets you type changes (`T44 *A | Dm7 G7 | Cmaj7 |`) or paste an
iReal Pro `irealb://` link (fills changes, key, tempo, composer; playlist links import every song).
It is a visual reference only: it does not follow the recording.

Data lives in `%APPDATA%\tidalite\library.json`.

## Controls

| | |
|---|---|
| Space | play / pause |
| Left / Right | seek -5s / +5s |
| Z X C V B | previous, play, pause, stop, next (the classic Winamp keys) |
| H | like / unlike the current song (heart in the player) |
| A | open / close the cover viewer |
| G | colour <-> black & white cover |
| L | lyrics on / off (in the cover viewer) |
| F or F11 | fullscreen (Esc leaves it) |
| M | mini player |
| P | listening <-> practice mode |
| [  ]  \ | practice: set loop start (A), loop end (B), loop on/off |
| ,  . | practice: back 2 seconds, restart loop / track |
| Up / Down | practice: speed +5% / -5% |

Media keys (play/pause, next, previous, stop) work even when Tidalite is in the background (Windows).
Right-click a song: play next / add to queue / add to or remove from My Tracks.
Right-click a row in the queue: play now / remove / like. Drag queue rows to reorder them.

## Library

The LIBRARY window has tabs: MY TRACKS (your liked songs, loaded in the background, with PLAY and SHUFFLE),
LISTS, ALBUMS, ARTISTS. HOME shows Tidal's home feed.
Tools row: SKIN (olive, aqua, dark, amber), EQ (10-band equalizer), SLEEP (timer with a 20 s fade-out),
DISK (where tracks are stored), LOG. Settings are remembered between runs.

## Listening vs. practicing (transcribing)

The LISTEN / PRACTICE switch sits on the player's title line (or press P). In LISTEN mode nothing changes:
no loop, normal speed. In PRACTICE mode a PRACTICE window opens under the player:

- **A-B loop**: SET A / SET B at the current position (or right-click the seek bar or the timeline and pick
  "Loop start / end here" for the exact spot under the mouse). The loop is sample-accurate and gapless.
  -/+ nudge each point by 0.1 s (hold Shift for 1 s) and jump to it so you can hear the edge.
- **Speed**: 25-150% with the pitch kept (presets 50 / 70 / 85 / 100).
- **Loops are remembered per track.** Come back next week and your loop is still there.
- BACK 2S and RESTART for the "play that bit again" reflex.

Looping, slow-down and seeking all happen locally on the stored copy of the song. Tidalite never reports plays
to Tidal, so none of this touches your listening statistics.

## Where are my tracks?

Every track you play is saved to `%APPDATA%\tidalite\cache\<trackid>.m4a` (or `.flac`). The strip at the bottom
of the player always says what is going on: `DOWNLOADING TO: ...`, `STORED ON THIS PC: <file>` or `NOT STORED YET`
(click it to open the folder). A check mark in any list or in the queue marks songs that are stored; those start
instantly and play offline. DISK in the library window shows the folder, how many songs and how much space they
use, lets you switch saving off, open the folder, or clear it.

## Shuffle and the queue

SHUFFLE (in the player, or on any album / playlist / MY TRACKS) really reorders the queue: the song playing
stays on top and everything after it is shuffled, so the QUEUE window always shows what plays next.
Turning shuffle off restores the original order. The queue is split into PLAYED / NOW PLAYING / UP NEXT, and the
status line of the player shows the next song.

## Cover viewer

The cover tilts toward your mouse (and sways gently when the mouse is elsewhere). COLOR / B&W switches the
filter (it also applies to the small cover). LYRICS shows time-synced lyrics from Tidal when available.
FULLSCREEN fills the screen. LIKE hearts the song.

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
- `src/player.rs`: audio thread (rodio): A-B looper, pitch-preserving slow-down, equalizer, spectrum tap
- `src/cache.rs`: the on-disk copy of played tracks
- `src/media.rs`: global media keys (Windows)
- `src/decode.rs`: symphonia decoder (AAC/MP4, FLAC, MP3)
- `src/font.rs`: the pixel font engine (hand-made 5x7 font, upper + lower case)
- `src/main.rs`: the UI shell, player window and queue
- `src/views.rs`: FILES / YT / TUNES / DIARY lists, practice panel, lead sheet, text fields
- `src/extras.rs`: logic for tunes, diary, trainer, metronome, sources
- `src/store.rs`: saved tunes, diary, sections (library.json)
- `src/stems.rs`: stem separation (ONNX Runtime, loaded from a downloaded DLL)
- `src/sources.rs`: files, yt-dlp, waveform, WAV export, composer lookup
- `src/chart.rs`: chord-chart text format and iReal Pro link reader

## Known limits

- v8 has not been compiled by its author (no Rust toolchain access to crates in the sandbox). If the build fails, send the Actions error.
- Stem separation is CPU-only: expect a few minutes per song and roughly 2-3 GB of free RAM while it runs. Windows only.
- Hi-Res / DASH streams aren't supported (lossless requests may come back as 320k AAC for this login).
- Only Latin text can be shown in the pixel font; other scripts appear as "?".
- No Tidal Connect. No Windows taskbar / lock-screen now-playing widget yet (media keys work).
- Slow-down is very good down to ~50%; below ~40% it starts to sound grainy.
