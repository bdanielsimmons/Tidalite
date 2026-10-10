//! Our own symphonia-based decoder. rodio 0.19's built-in one panics on MP4/AAC
//! ("Seek errors should not occur during initialization") because it never tells
//! symphonia how long the in-memory file is, so the MP4 reader can't seek to the index.
//! Here the byte length is known, so it works.

use rodio::source::SeekError;
use rodio::Source;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SErr;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

struct MemSrc {
    cur: Cursor<Vec<u8>>,
    len: u64,
}

impl Read for MemSrc {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.cur.read(b)
    }
}

impl Seek for MemSrc {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        self.cur.seek(p)
    }
}

impl MediaSource for MemSrc {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

pub struct SymSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    buf: Vec<i16>,
    idx: usize,
    rate: u32,
    channels: u16,
    dur: Option<Duration>,
    tb: Option<TimeBase>,
    /// samples still to discard after a coarse seek, so seeks land on the exact frame
    skip: usize,
}

impl SymSource {
    pub fn new(bytes: Vec<u8>) -> Result<SymSource, String> {
        let len = bytes.len() as u64;
        let mss = MediaSourceStream::new(Box::new(MemSrc { cur: Cursor::new(bytes), len }), Default::default());
        let hint = Hint::new();
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| format!("unrecognized format: {}", e))?;
        let mut format = probed.format;
        let (track_id, params) = {
            let t = format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .ok_or_else(|| "no playable audio track".to_string())?;
            (t.id, t.codec_params.clone())
        };
        let dur = params.time_base.zip(params.n_frames).map(|(tb, n)| {
            let t = tb.calc_time(n);
            Duration::from_secs_f64(t.seconds as f64 + t.frac)
        });
        let mut decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| format!("codec not supported: {}", e))?;

        // Decode the first packet so we know the real rate / channel count.
        let mut errors = 0;
        let (rate, channels, first) = loop {
            let pkt = format.next_packet().map_err(|e| format!("read: {}", e))?;
            if pkt.track_id() != track_id {
                continue;
            }
            match decoder.decode(&pkt) {
                Ok(d) => {
                    let spec = *d.spec();
                    let mut sb = SampleBuffer::<i16>::new(d.capacity() as u64, spec);
                    sb.copy_interleaved_ref(d);
                    break (spec.rate, spec.channels.count() as u16, sb.samples().to_vec());
                }
                Err(SErr::DecodeError(_)) => {
                    errors += 1;
                    if errors > 50 {
                        return Err("too many decode errors".to_string());
                    }
                }
                Err(e) => return Err(format!("decode: {}", e)),
            }
        };
        Ok(SymSource {
            format,
            decoder,
            track_id,
            buf: first,
            idx: 0,
            rate,
            channels: channels.max(1),
            dur,
            tb: params.time_base,
            skip: 0,
        })
    }
}

impl Iterator for SymSource {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        loop {
            if self.idx < self.buf.len() {
                let s = self.buf[self.idx];
                self.idx += 1;
                return Some(s);
            }
            let pkt = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => return None,
            };
            if pkt.track_id() != self.track_id {
                continue;
            }
            // a panic inside the codec must not kill the audio thread silently
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.decoder.decode(&pkt).map(|d| {
                    let spec = *d.spec();
                    let mut sb = SampleBuffer::<i16>::new(d.capacity() as u64, spec);
                    sb.copy_interleaved_ref(d);
                    sb.samples().to_vec()
                })
            }));
            match res {
                Ok(Ok(samples)) => {
                    self.buf = samples;
                    self.idx = 0;
                    if self.skip > 0 {
                        let n = self.skip.min(self.buf.len());
                        self.idx = n;
                        self.skip -= n;
                    }
                }
                Ok(Err(SErr::DecodeError(_))) => continue,
                _ => return None,
            }
        }
    }
}

impl Source for SymSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.dur
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let t = Time::new(pos.as_secs(), pos.subsec_nanos() as f64 / 1e9);
        match self.format.seek(SeekMode::Coarse, SeekTo::Time { time: t, track_id: Some(self.track_id) }) {
            Ok(to) => {
                self.decoder.reset();
                self.buf.clear();
                self.idx = 0;
                self.skip = 0;
                if let Some(tb) = self.tb {
                    let diff = to.required_ts.saturating_sub(to.actual_ts);
                    let d = tb.calc_time(diff);
                    let frames = (d.seconds as f64 + d.frac) * self.rate as f64;
                    self.skip = frames.round() as usize * self.channels as usize;
                }
                Ok(())
            }
            Err(_) => Err(SeekError::NotSupported { underlying_source: "symphonia" }),
        }
    }
}
