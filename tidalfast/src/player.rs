//! Audio engine on its own thread (rodio + symphonia: FLAC / AAC / MP3 decode in pure Rust).

use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
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
    pub error: Option<String>,
}

pub struct Player {
    tx: Sender<Cmd>,
    pub shared: Arc<Mutex<Shared>>,
}

impl Player {
    pub fn new() -> Player {
        let (tx, rx) = channel::<Cmd>();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let sh = shared.clone();

        std::thread::spawn(move || {
            let (_stream, handle) = match OutputStream::try_default() {
                Ok(x) => x,
                Err(e) => {
                    sh.lock().unwrap().error = Some(format!("No audio output device: {}", e));
                    return;
                }
            };
            let mut sink: Option<Sink> = None;
            let mut vol = 0.8f32;
            let mut base = 0.0f32; // seconds played before `started`
            let mut started: Option<Instant> = None; // None while paused / stopped

            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Cmd::Play(bytes)) => {
                        if let Some(s) = sink.take() {
                            s.stop();
                        }
                        match (Sink::try_new(&handle), Decoder::new(Cursor::new(bytes))) {
                            (Ok(s), Ok(dec)) => {
                                s.set_volume(vol);
                                s.append(dec);
                                sink = Some(s);
                                base = 0.0;
                                started = Some(Instant::now());
                                let mut g = sh.lock().unwrap();
                                g.ended = false;
                                g.error = None;
                                g.pos = 0.0;
                            }
                            (_, Err(e)) => {
                                sh.lock().unwrap().error = Some(format!("Can't decode audio: {}", e));
                            }
                            (Err(e), _) => {
                                sh.lock().unwrap().error = Some(format!("Audio error: {}", e));
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
                    g.ended = true;
                }
            }
        });

        Player { tx, shared }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}
