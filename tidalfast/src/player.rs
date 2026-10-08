//! Audio engine on its own thread (rodio + symphonia: FLAC / AAC / MP3 decode in pure Rust).
//! Also taps the decoded samples so the UI can draw a real spectrum analyzer.

use crate::api::log;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::source::SeekError;
use crate::decode::SymSource;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub enum Cmd {
    Play(Vec<u8>),
    Pause,
    Resume,
    Seek(f32),
    Volume(f32),
    Stop,
}

#[derive(Default)]
pub struct Shared {
    pub pos: f32,
    pub ended: bool,
    pub rate: u32,
    pub error: Option<String>,
    /// Set if the audio device could not be opened at all (playback is impossible).
    pub dead: Option<String>,
}

/// Most recent mono samples (-1..1), newest last. Read by the UI for the spectrum.
#[derive(Default)]
pub struct Viz {
    pub samples: Vec<f32>,
    pub rate: u32,
}

pub struct Player {
    tx: Sender<Cmd>,
    pub shared: Arc<Mutex<Shared>>,
    pub viz: Arc<Mutex<Viz>>,
}

/// Pass-through source that copies one channel of samples into `Viz`.
struct Tap<S: Source<Item = i16>> {
    inner: S,
    viz: Arc<Mutex<Viz>>,
    scratch: Vec<f32>,
    ch: u16,
    phase: u16,
}

impl<S: Source<Item = i16>> Tap<S> {
    fn new(inner: S, viz: Arc<Mutex<Viz>>) -> Tap<S> {
        let ch = inner.channels().max(1);
        Tap { inner, viz, scratch: Vec::with_capacity(256), ch, phase: 0 }
    }

    fn flush(&mut self) {
        if let Ok(mut v) = self.viz.lock() {
            v.samples.extend(self.scratch.drain(..));
            let len = v.samples.len();
            if len > 4096 {
                v.samples.drain(0..len - 4096);
            }
        } else {
            self.scratch.clear();
        }
    }
}

impl<S: Source<Item = i16>> Iterator for Tap<S> {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        let s = self.inner.next()?;
        self.phase += 1;
        if self.phase >= self.ch {
            self.phase = 0;
            self.scratch.push(s as f32 / 32768.0);
            if self.scratch.len() >= 256 {
                self.flush();
            }
        }
        Some(s)
    }
}

impl<S: Source<Item = i16>> Source for Tap<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)
    }
}

/// Open the default output device; if that fails, try every device the system lists.
fn open_output() -> Result<(OutputStream, OutputStreamHandle), String> {
    let why = match OutputStream::try_default() {
        Ok(x) => {
            log("audio: default output device opened");
            return Ok(x);
        }
        Err(e) => format!("default device failed: {}", e),
    };
    log(&format!("audio: {}", why));
    let host = rodio::cpal::default_host();
    if let Ok(devs) = host.output_devices() {
        for d in devs {
            let name = d.name().unwrap_or_else(|_| "?".to_string());
            match OutputStream::try_from_device(&d) {
                Ok(x) => {
                    log(&format!("audio: opened device '{}'", name));
                    return Ok(x);
                }
                Err(e) => log(&format!("audio: device '{}' failed: {}", name, e)),
            }
        }
    }
    Err(why)
}

impl Player {
    pub fn new() -> Player {
        let (tx, rx) = channel::<Cmd>();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let viz = Arc::new(Mutex::new(Viz::default()));
        let sh = shared.clone();
        let vz = viz.clone();

        std::thread::spawn(move || {
            let (_stream, handle) = match open_output() {
                Ok(x) => x,
                Err(e) => {
                    let msg = format!("NO AUDIO OUTPUT: {}", e);
                    log(&msg);
                    let mut g = sh.lock().unwrap();
                    g.error = Some(msg.clone());
                    g.dead = Some(msg);
                    return;
                }
            };
            let mut sink: Option<Sink> = None;
            let mut vol = 0.64f32;
            let mut base = 0.0f32; // seconds played before `started`
            let mut started: Option<Instant> = None; // None while paused / stopped

            loop {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(Cmd::Play(bytes)) => {
                        log(&format!("player: received {} bytes", bytes.len()));
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        let sink_res = Sink::try_new(&handle);
                        let (dtx, drx) =
                            channel::<Result<SymSource, String>>();
                        std::thread::spawn(move || {
                            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                SymSource::new(bytes)
                            }));
                            let _ = dtx.send(match r {
                                Ok(x) => x,
                                Err(_) => Err("decoder panicked".to_string()),
                            });
                        });
                        let dec_res = match drx.recv_timeout(Duration::from_secs(15)) {
                            Ok(r) => r,
                            Err(_) => Err("decoder timed out".to_string()),
                        };
                        match (sink_res, dec_res) {
                            (Ok(s), Ok(dec)) => {
                                let rate = dec.sample_rate();
                                log(&format!("decoder ok: {} Hz, {} ch", rate, dec.channels()));
                                s.set_volume(vol);
                                {
                                    let mut v = vz.lock().unwrap();
                                    v.samples.clear();
                                    v.rate = rate;
                                }
                                s.append(Tap::new(dec, vz.clone()));
                                sink = Some(s);
                                base = 0.0;
                                started = Some(Instant::now());
                                let mut g = sh.lock().unwrap();
                                g.ended = false;
                                g.error = None;
                                g.pos = 0.0;
                                g.rate = rate;
                            }
                            (_, Err(e)) => {
                                let m = format!("DECODE: can't decode audio ({})", e);
                                log(&m);
                                sh.lock().unwrap().error = Some(m);
                            }
                            (Err(e), _) => {
                                let m = format!("Audio device error: {}", e);
                                log(&m);
                                sh.lock().unwrap().error = Some(m);
                            }
                        }
                    }
                    Ok(Cmd::Pause) => {
                        if let Some(s) = &sink {
                            s.pause();
                            if let Some(t) = started.take() {
                                base += t.elapsed().as_secs_f32();
                            }
                        }
                    }
                    Ok(Cmd::Resume) => {
                        if let Some(s) = &sink {
                            s.play();
                            if started.is_none() {
                                started = Some(Instant::now());
                            }
                        }
                    }
                    Ok(Cmd::Seek(t)) => {
                        if let Some(s) = &sink {
                            if s.try_seek(Duration::from_secs_f32(t.max(0.0))).is_ok() {
                                base = t;
                                if started.is_some() {
                                    started = Some(Instant::now());
                                }
                            }
                        }
                    }
                    Ok(Cmd::Volume(v)) => {
                        vol = v;
                        if let Some(s) = &sink {
                            s.set_volume(v);
                        }
                    }
                    Ok(Cmd::Stop) => {
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        started = None;
                        base = 0.0;
                        vz.lock().unwrap().samples.clear();
                        let mut g = sh.lock().unwrap();
                        g.ended = false;
                        g.pos = 0.0;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                let pos = base + started.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
                let finished = sink.as_ref().map(|s| s.empty()).unwrap_or(false);
                if finished {
                    sink = None;
                    started = None;
                }
                let mut g = sh.lock().unwrap();
                g.pos = pos;
                if finished {
                    if pos < 1.0 {
                        // The decoder gave us (almost) nothing: don't treat that as "track finished",
                        // or the whole queue would race by in silence.
                        log("decoder produced no audio");
                        g.error = Some("DECODE: decoder produced no audio".to_string());
                    } else {
                        g.ended = true;
                    }
                }
            }
        });

        Player { tx, shared, viz }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}
