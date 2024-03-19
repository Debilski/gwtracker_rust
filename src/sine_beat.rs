use std::f32::consts::PI;
use std::time::Duration;

use rodio::source::Source;

/// An infinite source that produces a sine.
///
/// Always has a rate of 48kHz and one channel.
#[derive(Clone, Debug)]
pub struct SineBeat {
    freq1: f32,
    freq2: f32,
    num_sample: usize,
}

impl SineBeat {
    /// The frequency of the sine.
    #[inline]
    pub fn new(freq: f32, beat_length: f32) -> SineBeat {
        // our beat length ignores phase, therefore we must divide it by 2
        let beat = 1.0 / beat_length / 2.0;

        let freq1 = freq + beat;
        let freq2 = freq - beat;

        println!("Sine wave generator: base freq {freq:.3}Hz, beat length {beat_length:.3} s. -> f1: {freq1:.3}Hz, f2: {freq2:.3}Hz.");

        // we skip at the beginning so that we start in between beats
        let skip = (48000.0 * beat_length / 2.0) as usize;
        //println!("{skip}");

        SineBeat { freq1, freq2, num_sample: skip }
    }
}

impl Iterator for SineBeat {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        self.num_sample = self.num_sample.wrapping_add(1);

        let value1 = 2.0 * PI * self.freq1 * self.num_sample as f32 / 48000.0;
        let value2 = 2.0 * PI * self.freq2 * self.num_sample as f32 / 48000.0;
        Some((value1.sin() + value2.sin()) / 2.0)
    }
}

impl Source for SineBeat {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> u16 {
        1
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        48000
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
