# Backline

A retro, Winamp-flavoured desktop player for Tidal. Native Rust (egui), no browser engine, no API keys:
you log in with the normal "approve in your browser" flow. Needs an active Tidal subscription.
Unofficial: it talks to the same endpoints the Tidal apps and the open-source `tidalapi` library use.

<!-- demo:start -->
**See it in action:** [the tour](https://github.com/bdanielsimmons/Backline/releases/download/demo/backline-tour.mp4) · [the practice tour](https://github.com/bdanielsimmons/Backline/releases/download/demo/backline-practice-tour.mp4) (MP4 videos)
<!-- demo:end -->

## What's new

<!-- whats-new -->
- The practice tour now shows the CHORDS tab (strumming a chord), LINES (playing a line) and the lead sheet with the band playing a ii-V-I
- Tidalite is now called Backline - your library, login, skins and settings move across by themselves
- Tracks stream without being saved to disk; switch SAVING on under DISK to keep them
- LINES under PRACTICE - Slonimsky's Diary (lines from Slonimsky's system, or a pattern book you add) and scales in intervals, in notation and tab, looped, counted
- Practice progressions under TUNES - fold open a group, pick a key, hear it, or send it to the lead sheet for the band
- Drag across bars on the lead sheet to loop just those
- Charts can change time signature bar by bar (iReal Pro imports keep them)
- Tunes have WRITTEN BY, YEAR, FROM and STYLE, filled in for you when they can be found
- Set your own key and BPM for any song (right-click > Key and BPM...), kept on this computer
- A new tune opens straight to its lead sheet, ready to fill in
<!-- /whats-new -->

Every release lists its full changes on the [Releases page](../../releases).

## Get the .exe (no Rust needed on your machine)

1. Upload everything in this folder to your GitHub repo (keep `.github/workflows/build.yml`;
   if hidden folders get dropped, create that file in the GitHub web UI and paste in `build.yml.copy`).
2. Actions tab -> the `build` run -> wait ~5-10 min.
3. Each run makes four downloads (Artifacts, bottom of the run page):
   - `backline-windows-practice` / `backline-mac-practice`: the full app with the practice studio
   - `backline-windows-simple` / `backline-mac-simple`: just the Tidal player, none of the practice tools
   Windows: unzip and run the `.exe`. Mac (Apple Silicon and Intel in one): unzip, then `tar -xzf` the archive, and the first
   time right-click `backline` -> Open (or run `xattr -dr com.apple.quarantine backline`) because it is not signed.
   Stem separation is Windows-only for now; everything else works on both.

## v9.1: what changed in the last round

- **Chart** follows the song: change tracks and it looks the new one up in the background. REFRESH re-runs it; SAVE TO TUNES keeps it.
  Repeated bars show as a slash, plus repeat signs, 1st/2nd endings, segno, coda, D.C./D.S./Fine. Gospel songs are not in the free chart list:
  the lookup suggests close matches (also by partial title) and you can paste an iReal link or type the changes.
- **Band**: a simple built-in player (drums, bass, comping) that plays the chart in several styles; the current bar lights up.
- **Metronome** (METRONOME button, floats bottom-right, independent of any track): beats, subdivisions, downbeat on/off,
  Soundbrenner-style per-beat volumes (click a beat to cycle 3/3, 2/3, 1/3, off), a clickable beat row and a swinging pendulum,
  gap click (play N bars, mute M), faster-every-N-bars, FOLLOW SPEED (scales with the speed slider) and LOCK TO TRACK (detects
  the beat of the playing track; x2 / /2 and SHIFT ms fix a wrong guess; approximate on loose-timed music).
- **Typed values** in practice: click A / B / speed % / trainer loops / +% and type (times like 1:23.5).
- MORE is tabbed: TRAINER | PITCH & EAR | STEMS.
- **Skins**: eight, still SKIN button. Pixel buttons, boxes, tabs, bubbles and a pixel logo. Library / queue splitters are shares of the window.
- **LOOK UP** ranks recordings for studying: Tidal popularity, minus karaoke / tribute / compilation tracks, plus a boost for artists you've starred 3+ in other tunes. **PLAY-ALONG** opens a YouTube search for backing tracks.
- **SoundCloud** tab (in both builds): search it, or paste a track / playlist / profile-likes link. It uses the same yt-dlp
  one-time setup as YT. No sign-in is used (public content only); right-click a result to keep it in MY SOUNDCLOUD.
  SIGN-IN FROM (FIREFOX / CHROME / EDGE) borrows the SoundCloud sign-in of a browser on this PC, for private playlists and Go+. Chrome and Edge
  sometimes block this while they are open; Firefox is the reliable one.
- **Tidal login is optional**: USE WITHOUT TIDAL on the first screen (USE SOUNDCLOUD ONLY in the simple build); LOG IN TO TIDAL appears
  in the library whenever you want it.
- The album-art view background is now a soft gradient from the cover's average colour (no stretched picture).
- Album viewer controls are pixel icons: previous / play-pause / next, B&W-color, lyrics (microphone), fullscreen, like (Tidal tracks only). Hover for names.
- The album-art view shows your saved loops and the live A-B loop on its seek bar.
- **Custom meters**: METER is two boxes, beats (1-32) and what a beat is (1-64): type 11 and 17 for 11/17, or type `11/17` straight into the beats box. 4 / 8 / 16 are one-click shortcuts.
- **Odd meters**: BEATS goes to 32 and the unit sets what a beat is (22 / 8 = twenty-two eighths; BPM stays in quarter notes).
  GROUPS accents 2s, 3s and 4s (7 = 2+2+3, 22 = 3+3+3+3+3+3+4 ...). Click any beat to set its own volume.
- Fixed: STOP on the metronome now really stops it (the click loop used to keep playing).
- **LOOK UP** has a style button next to it (AUTO / JAZZ / GOSPEL / ANY). AUTO treats a tune from the jazz standards list as jazz, searches wider
  ("Countdown jazz", "Countdown Coltrane") and puts jazz players and the composer's own recording above chart pop.
- Window title tabs have small pixel icons; the album viewer has shuffle and repeat icons.
- Pixel icons on the library toolbar (hover for names), loop / clear / back / restart buttons, shuffle and repeat on the player, and a speaker icon for volume (click to mute).
- SoundCloud results show names right away (taken from the link) and fill in the real titles, lengths and covers in the background.
- Like button is hidden for files and YouTube clips.

## v9: the practice studio

The library now has five lists: **TIDAL | FILES | YT | TUNES | DIARY**.

- **FILES**: drag audio files or folders onto the window (or ADD FILES / ADD FOLDER). Nothing is copied.
  Folders are remembered; RESCAN picks up new files. Loops, slow-down and everything else work on them.
- **YT** (YouTube): press GET YT-DLP once (downloads the free yt-dlp tool from its GitHub page), paste a link, ADD.
  First play downloads only the audio (AAC) and stores it; after that it starts instantly and loops like anything else.
  Downloading from YouTube may go against its terms; personal practice use is at your discretion.
- **TUNES**: your repertoire. Each tune has a comfort level (LEARNING / OK / GOOD / COMFORTABLE, click a pip), key, tempo, notes and the
  *recordings worth studying* (Tidal, files or YouTube) with 1-5 stars and a note each.
  Right-click any song -> "Add to a tune...". LOOK UP (only when you press it) fetches the composer from
  MusicBrainz and lists the most popular recordings on Tidal (Tidal's own popularity score; hover for the album);
  right-click one to add it.
- **DIARY**: practice time is counted automatically while you are in PRACTICE mode. It shows where the time went
  (per tune, today and the last 30 days), week dots, a gentle streak (today not yet practiced never breaks it), and a journal (LOG PRACTICE).
- **CONTINUE** button at the top of the library resumes the last track, position and speed.

### Practice panel
- Waveform timeline. Click to jump, **Shift + drag** to select a loop.
- **Ctrl + drag** near the A or B marker on the waveform moves it. **Shift + drag inside the loop** slides the whole loop, same length.
- **SAVE LOOP** keeps named A-B loops per track (chips below; click to go, right-click to delete).
- **MORE** opens: speed **TRAIN** (set how many loops to play, and how many percent faster after them),
  **PITCH** transpose without changing speed, **EAR** modes (left, right, mono, no-center, bass only),
  **EXPORT WAV** of the loop (Music/Backline loops), **COUNT** in (2 or 4 clicks before each loop pass), a METRONOME button,

- **STEMS**: GET STEMS TOOL downloads (once, ~170 MB) the ONNX Runtime library and the HT-Demucs model from
  Hugging Face / GitHub. Then SPLIT THIS TRACK separates a stored track into drums, bass, other (guitar, keys, horns)
  and vocals; they are saved on disk (DISK view shows the size, CLEAR STEMS deletes them). Afterwards the pixel icons
  (drums, bass, other, vocals) switch each stem on or off; right-click an icon to hear only that one.
  Loops, slow-down, pitch and EQ all keep working on the mix.
- **TIMER** (button in the tools row, works in every mode): set focus minutes, rest minutes and how many blocks, then START.
  It floats bottom-right. When a block ends the window flashes (and chimes, unless you untick SOUND) and switches to REST; after the rest it waits
  (flashing) until you press NEXT FOCUS. The time also shows in the window title.
- **FOCUS** (chip on the player, or in MORE) hides the lists: just the player, your tools and the chart.

### Lead sheet
The changes are found for you: open a tune's CHART tab and Backline fetches them from the free Jazz Standards chart list
(about 1,300 tunes, downloaded once), plus key and composer. **I-IV-V** shows roman numerals under each chord. Tunes that aren't
in the list (gospel, say) can still be typed in or imported from an iReal Pro link with EDIT.

Right-bottom tab **CHART** (next to QUEUE): the chord changes of the open tune as a static grid, with
CONCERT / Bb / Eb / F transposition. EDIT lets you type changes (`T44 *A | Dm7 G7 | Cmaj7 |`) or paste an
iReal Pro `irealb://` link (fills changes, key, tempo, composer; playlist links import every song).
It is a visual reference only: it does not follow the recording.

Data lives in `%APPDATA%\backline\library.json`.

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

Media keys (play/pause, next, previous, stop) work even when Backline is in the background (Windows).
Right-click a song: play next / add to queue / add to or remove from My Tracks.
Right-click a row in the queue: play now / remove / like. Drag queue rows to reorder them.

## Library

The LIBRARY window has tabs: MY TRACKS (your liked songs, loaded in the background, with PLAY and SHUFFLE),
LISTS, ALBUMS, ARTISTS. HOME shows Tidal's home feed.
The BACKLINE menu (top of the library) holds the skins, preferences, help and tours, where tracks are stored, and the log.
The player has the EQ (10-band equalizer) and the sleep timer (the moon; it fades out over 20 s). In practice mode the
pomodoro timer and the metronome sit in the practice panel. Settings are remembered between runs.

## Listening vs. practicing (transcribing)

The LISTEN / PRACTICE switch sits on the player's title line (or press P). In LISTEN mode nothing changes:
no loop, normal speed. In PRACTICE mode a PRACTICE window opens under the player:

- **A-B loop**: SET A / SET B at the current position (or right-click the seek bar or the timeline and pick
  "Loop start / end here" for the exact spot under the mouse). The loop is sample-accurate and gapless.
  -/+ nudge each point by 0.1 s (hold Shift for 1 s) and jump to it so you can hear the edge.
- **Speed**: 25-150% with the pitch kept (presets 50 / 70 / 85 / 100).
- **Loops are remembered per track.** Come back next week and your loop is still there.
- BACK 2S and RESTART for the "play that bit again" reflex.

Looping, slow-down and seeking all happen locally, on the song held in memory. Backline never reports plays
to Tidal, so none of this touches your listening statistics.

## Where are my tracks?

Tracks are streamed and held in memory while they play; nothing is kept afterwards. DISK in the library window
shows the folder Backline keeps its data in, and has a SAVING switch (off unless you turn it on) and CLEAR.

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
Everything is also saved in `%APPDATA%\backline\log.txt`. Your login lives in `%APPDATA%\backline\session.json`
(migrated automatically from the old `tidalfast` folder).

## Build it yourself

```
winget install Rustlang.Rustup
cargo build --release
target\release\backline.exe
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
- `src/chart.rs`: chord-chart text format, iReal Pro link reader, Jazz Standards data, roman numerals

## Known limits

- v9 has not been compiled by its author (no Rust toolchain access to crates in the sandbox). If the build fails, send the Actions error.
- Stem separation is CPU-only: expect a few minutes per song and roughly 2-3 GB of free RAM while it runs. Windows only.
- Hi-Res / DASH streams aren't supported (lossless requests may come back as 320k AAC for this login).
- Only Latin text can be shown in the pixel font; other scripts appear as "?".
- No Tidal Connect. No Windows taskbar / lock-screen now-playing widget yet (media keys work).
- Slow-down is very good down to ~50%; below ~40% it starts to sound grainy.


## v9.14 (fixes four build errors) / v9.13
- Chart screen: STYLE and TRANSPOSE are dropdowns; buttons grow instead of shrinking their text.
- PRACTICE > MORE opens as an overlay, so the panels below never move.
- Right-click a bar of a saved tune to edit repeats, endings, sections, marks; IMPORT IREAL (links or playlists) at the top of TUNES.
- Diary: calendar, summary tiles, log practice for any day. Only time spent on an active A-B loop counts.
- Track lists: right-click the header row to choose ARTIST / ALBUM / LENGTH columns.
- Hearts on files, YouTube and SoundCloud are saved on this PC (a LIKED block in each section).
- Redrawn shuffle/repeat icons, left-aligned window title tabs, milder cover tilt (only near the cover).

## v9.20
- Three new skins (right-click the skin icon to pick): AERO GLASS (glossy blue/green glass, title bars like XP/Vista), SLEEK DARK and SLEEK LIGHT. They use a smooth font and rounded widgets; the original pixel skins are unchanged.
- Visualizer: BARS, WAVEFORM or BOTH, in the player's LCD (click it to cycle) and behind the album art. Right-click for bar width and sensitivity (art view also has opacity and height).

## v9.21
- Visualizer: set the number of bars (8-96, default 32) and pick colours: SKIN, FROM COVER ART (a main colour from the cover with a gentle complementary tip) or CLASSIC GREEN + RED. Right-click the visualizer.
- Lyrics: scroll with the mouse wheel (the view follows the song again after a few seconds) and click a line of synced lyrics to jump to it.

## v9.22
- Contrast pass on every skin: main, secondary and hint text, errors and header text all meet readable contrast (SLATE got lighter panels to make room). Phosphor skins keep their colour.
- Metronome pendulum: finer rod, faint afterimages, scale marks, a tip that flashes on the beat and fades.

## v9.23
- Metronome LOCK TO TRACK: finds the beat by itself as soon as the track's audio is stored, shows its status (listening / locked / waiting / no beat found), and falls back to your own tempo when no steady beat exists instead of going silent.

## v9.24
- Fixed the build error from a field name clash (`wave`).
- Diary and streaks use your local date on Mac/Linux (they used UTC before, so the day rolled over in the evening).
- A repeat sign or ending typed without a bar line before it now starts its own bar.
- Unit tests for the chart module (`cargo test`): repeats, 1st/2nd endings, D.C., the bar editor round trip, transposing.

## v9.25
- Rows of buttons in the practice panel, tune pages, lists and the lead sheet now wrap onto a second line instead of stretching past the edge. SAVE LOOP is sized from its real width and drops below the name box when the panel is narrow.

## v9.27
- Band chords: #11, 11, 13 and b13 now change the voicing (they were ignored, so Fmaj7#11 sounded like Fmaj7 and, in the middle voices, like F#m7b5).

## v9.28
- Modern skins (AERO GLASS, SLEEK DARK, SLEEK LIGHT) draw smooth vector icons and a vector gem logo (`vicon.rs`); only the visualizer stays pixelated. Icons without a vector form fall back to rounded dots.
- Logo strip is taller so the title no longer touches the top edge.
- Album-art view: skin picker button, spectrum WIDTH setting, filled waveform, skin-coloured bars.
- Saved playlists for SoundCloud and YouTube (links kept in library.json); YouTube playlist links list their videos; right-click a song for Copy link.
- LISTS tab: your own playlists mixing any source; right-click a song > Add to a playlist; SAVE QUEUE AS PLAYLIST.
- v9.29: main.rs split: skin.rs (palettes, colour helpers), viz.rs (visualizer), icons.rs (pixel icons); one shared skin menu. Behaviour unchanged.
- Simple build now includes Files and YouTube (only practice tools are left out). Every green build publishes a Release (build-N).
- Self-update (update.rs): checks GitHub Releases, downloads the new build in the background, installs on next start or on click. Needs a public repo; builds from the workflow carry BACKLINE_BUILD.
- Pitch is now in cents: -10c / +10c fine-tune buttons next to the semitone +/- (for records not tuned to A440).
- FIND TUNING (tuning.rs): measures how many cents a track is from A440 and offers TUNE TO A440. Unit-tested on synthetic chords (+-3 cents).
- icon.ico + build.rs embed the logo in the Windows exe (taskbar / pinned shortcuts).
- Help overlay (? button / F1): start, keys, practice, about.
- Visualizer settings are now separate for the player widget and the album view (VizCfg).
- Guided tour (tour.rs): general tour for everyone + a separate short practice tour; opens from Help > TOUR, offered once on first run.
- Update shows a restart bar; auto-restart is opt-in (see Preferences).

## Preferences (PREFS button)
- **Updates:** a downloaded update shows a bar at the top ("UPDATE vN READY - CLICK TO RESTART"). Nothing restarts by itself unless you turn on *Restart by itself when I'm away* (off by default; waits for ~5 quiet minutes with nothing playing). Otherwise the update installs the next time you open the app. Checks happen at launch and every 6 hours using a conditional request (ETag), so an unchanged check is a tiny 304 that GitHub doesn't count against its rate limit.
- **Keyboard shortcuts:** every key is rebindable (click a key, press the new one; ESC cancels; a key taken from another action unbinds that one). ESC, F1, F11, TAB and ENTER stay reserved. Reset-to-default button included. The Help KEYS tab reflects your bindings.

## Build speed
CI caches compiled dependencies (Swatinem/rust-cache), and the release profile uses thin LTO with 8 codegen units. The first build after this change is as slow as before (it fills the cache); later ones only recompile Backline itself.

## Where things are
- `src/main.rs` - the app window, state, and the main update loop
- `src/extras.rs` - logic for the practice extras and update/help actions
- `src/audio/` - player, decoder, media keys, stem separation, tuning detection, metronome/band
- `src/data/` - Tidal API, files/YouTube sources, library store, cache, lead-sheet charts, self-update
- `src/ui/` - fonts, skins, icons, visualizer, views, tools, help, tour, preferences
(Modules keep short names like `crate::player`; the folders just group the files.)

## Mac menu bar, fullscreen, softer album view
- macOS gets a real menu bar (Backline / File / Playback / View / Window / Help) via the `muda` crate (`src/ui/macmenu.rs`). Shortcuts use Cmd, never bare letters. Cmd+, opens Preferences, Ctrl+Cmd+F toggles full screen.
- Fullscreen (F, or F11) now works from any view, and the app follows the window when it goes fullscreen by itself (green Mac button), so it can't get stuck.
- Album view, modern skins: bars are a soft gradient and neighbours are blended; the waveform is smoothed. The right-click menu on the spectrum icon has a new COVER setting (cover opacity, 50-100%, default 90%) so the visualizer shows faintly through the cover.

## Modes, command palette, size, overflow fixes
- **Modes:** LIBRARY / PLAYER / ALBUM / MINI. Keys 1-4 switch from anywhere and are rebindable; in player mode a button row floats bottom-left to get back.
- **Command palette:** Ctrl+K (Cmd+K on a Mac). Type part of an action name, arrows + Enter. Same names are used by the Mac menu bar (`run_named` in `src/ui/palette.rs`).
- **Size of everything:** Preferences -> 90% to 175% (also Ctrl +/-). Saved with the settings.
- **Library tabs** now tighten their padding and shrink their text to stay inside the window; the **metronome** is wider/bigger, its rows wrap instead of spilling, and the pendulum and beat boxes are larger.
