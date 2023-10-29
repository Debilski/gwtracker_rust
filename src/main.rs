
mod take_with_fade;
use chrono::format::Item;
use take_with_fade::TakeWithFade;

use std::fs::File;
use std::io::BufReader;
use std::thread::{sleep, self};
use std::time::Duration;

use clokwerk::{Scheduler, TimeUnits, Job};

// Import week days and WeekDay
use clokwerk::Interval::*;

use rodio::queue::queue;
use rodio::source::{SineWave, Source, FadeIn, Empty};
use rodio::{dynamic_mixer, OutputStream, Sink, Decoder, Sample};

use tokio;
use tokio::select;

const FADEOUTTIME: u64 = 0;

fn source(str: String) -> Decoder<BufReader<File>> {
    println!("Opening {:?}", str);
    let file = File::open(str).unwrap();
    Decoder::new(BufReader::new(file)).unwrap()
}

fn source_m1() -> Decoder<BufReader<File>> {
    source("sounds/M-1ab_140.mp3".to_string())
}

fn source_m2() -> Decoder<BufReader<File>> {
    source("sounds/M-2ab_130.mp3".to_string())
}

fn source_m3() -> Decoder<BufReader<File>> {
    source("sounds/M-3ab_150.mp3".to_string())
}

fn source_m35() -> Decoder<BufReader<File>> {
    source("sounds/M35-perma.mp3".to_string())
}

fn source_m75() -> Decoder<BufReader<File>> {
    source("sounds/M75-perma.mp3".to_string())
}


    #[inline]
    fn take_duration_with_fade<I>(source: I, duration: Duration, fade_duration: Duration) -> TakeWithFade<I>
        where
        I: Source,
        I::Item: Sample,
    {
        take_with_fade::take_with_fade(source, duration, fade_duration)
    }



fn main() {
    // Construct a dynamic controller and mixer, stream_handle, and sink.
    let (controller, mixer) = dynamic_mixer::mixer::<f32>(2, 44_100);
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    // Create queues
    let (tx_m1, mut rx_m1) = queue(true);
    let (tx_m2, mut rx_m2) = queue(true);
    let (tx_m3, mut rx_m3) = queue(true);
    let (tx_m35, mut rx_m35) = queue(true);
    let (tx_m75, mut rx_m75) = queue(true);

    controller.add(rx_m1);
    controller.add(rx_m2);
    controller.add(rx_m3);
    controller.add(rx_m35);
    controller.add(rx_m75);

    // Create four unique sources. The frequencies used here correspond
    // notes in the key of C and in octave 4: C4, or middle C on a piano,
    // E4, G4, and A4 respectively.
    let source_c = SineWave::new(261.63)
        .take_duration(Duration::from_secs_f32(5.))
        .amplify(0.20);
    let source_e = SineWave::new(329.63)
        .take_duration(Duration::from_secs_f32(5.))
        .amplify(0.20);
    let source_g = SineWave::new(392.0)
        .take_duration(Duration::from_secs_f32(5.))
        .amplify(0.20);
    let mut source_a = SineWave::new(440.0)
        .take_duration(Duration::from_secs_f32(4.));
    source_a.set_filter_fadeout();
    let source_a = source_a.amplify(0.20);


    // // Add sources C, E, G, and A to the mixer controller.
    let rxx_m1 = tx_m1.append_with_signal(take_duration_with_fade(source_m1().convert_samples(), Duration::from_secs(5), Duration::from_millis(100))); //.take_crossfade_with(Empty::<f32>::default(), Duration::from_millis(FADEOUTTIME)));
    // let rxx_m2 = tx_m2.append_with_signal(take_duration_with_fade(source_m2().convert_samples(), Duration::from_secs(2), Duration::from_millis(100))); // .take_crossfade_with(Empty::<f32>::default(), Duration::from_millis(FADEOUTTIME)));
    // let rxx_m3 = tx_m3.append_with_signal(take_duration_with_fade(source_m3().convert_samples(), Duration::from_secs(3), Duration::from_millis(100))); // .take_crossfade_with(Empty::<f32>::default(), Duration::from_millis(FADEOUTTIME)));

    // let rxx_m35 = tx_m35.append_with_signal(source_m35().convert_samples().take_duration(Duration::from_secs(2))); // .take_crossfade_with(Empty::<f32>::default(), Duration::from_millis(FADEOUTTIME)));

    //controller.add(source_a);

    // Append the dynamic mixer to the sink to play a C major 6th chord.
    sink.append(mixer);

    let mut scheduler = Scheduler::with_tz(chrono::Utc);

    let c = controller.clone();

    scheduler.every(Seconds(0)).once().run(move || {
        let (c, mixer) = dynamic_mixer::mixer::<f32>(2, 44_100);
        let mut source_c = take_duration_with_fade(SineWave::new(220.0), Duration::from_secs(1), Duration::from_millis(5000)).fade_in(Duration::from_millis(100));
        //let mut source_a = take_duration_with_fade(SineWave::new(330.63), Duration::from_secs(10), Duration::from_millis(1000)).fade_in(Duration::from_millis(100));
        // let mut source_c = SineWave::new(261.63)
        // .fade_in(Duration::from_millis(100))
        // .take_duration(Duration::from_secs_f32(2.))
        // ;
        // source_c.set_filter_fadeout();
        //let source_c = source_c.amplify(0.20);

//        let source_c = source_m75().convert_samples();
        //c.add(source_a);
        c.add(source_c);
        controller.add(mixer);
    }    );

thread::spawn(move || {
rxx_m1.recv();
        println!("Finished m1");
});

// thread::spawn(move || {
// rxx_m3.recv();
//         println!("Finished m3");
// });

    // Sleep the thread until sink is empty.


// Or run it in a background thread
let thread_handle = scheduler.watch_thread(Duration::from_millis(100));
// The scheduler stops when `thread_handle` is dropped, or `stop` is called

// if let Ok(_) = rxx_m2.recv_timeout(Duration::from_millis(40)) {
//     println!("Finished");
// }
sleep(Duration::from_secs(10));

thread_handle.stop();

}
