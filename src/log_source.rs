use std::fmt::Debug;
use std::time::Duration;

use rodio::{Sample, Source};

/// Internal function that builds a `LogSource` object.
pub fn log_source<I>(input: I, str: String) -> LogSource<I>
where
    I: Source,
    I::Item: Sample + Debug,
{
    LogSource { input, log: str, has_logged: false, max: None }
}

/// A source that LogSources the given source.
#[derive(Clone, Debug)]
pub struct LogSource<I>
where
    I: Source,
    I::Item: Sample,
{
    input: I,
    log: String,
    has_logged: bool,
    max: Option<I::Item>,
}

impl<I> Iterator for LogSource<I>
where
    I: Source,
    I::Item: Sample + Debug,
{
    type Item = <I as Iterator>::Item;

    #[inline]
    fn next(&mut self) -> Option<<I as Iterator>::Item> {
        if let Some(value) = self.input.next() {
            if !self.has_logged {
                println!("Beggining source {:?}", self.log);
                self.has_logged = true;
            }
            if Some(value) > self.max {
                println!("{:?}", value);
                self.max = Some(value);
            }
            return Some(value);
        } else {
            println!("Finished source {:?}", self.log);
            return None;
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I> Source for LogSource<I>
where
    I: Iterator + Source,
    I::Item: Sample + Debug,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}
