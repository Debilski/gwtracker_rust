mod datafetch;
mod take_with_fade;

use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread::{self, sleep};
use std::time::Duration;

use chrono::{DateTime, Local};

use dashmap::DashMap;
use rand::Rng;
use rodio::queue::{queue, SourcesQueueInput};
use rodio::source::Source;
use rodio::{dynamic_mixer, Decoder, OutputStream, Sample, Sink};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use take_with_fade::TakeWithFade;

use colored::Colorize;

use datafetch::read_tsv;
use tokio::time::sleep_until;

type SourceOnce = Decoder<BufReader<File>>;
type SourceOnceBuffered = rodio::source::Buffered<SourceOnce>;
type SourceInfinite = rodio::source::Repeat<SourceOnce>;
type SourceInfiniteBuffered = rodio::source::Repeat<SourceOnceBuffered>;

fn source(str: &str) -> SourceOnce {
    // We either check relative to the current folder and if nothing is found,
    // we search relative to the exe path

    let rel_to_cwd_path = PathBuf::from(str);

    let path = if rel_to_cwd_path.exists() {
        rel_to_cwd_path
    } else {
        let current_exe = std::env::current_exe().unwrap();
        let rel_to_exe_path = current_exe.parent().unwrap().join(str);
        if rel_to_exe_path.exists() {
            rel_to_exe_path
        } else {
            println!(
                "Neither {:?} nor {:?} exit.",
                rel_to_cwd_path, rel_to_exe_path
            );
            std::process::exit(1)
        }
    };

    println!("Opening {:?}", path);
    let file = File::open(path.clone()).unwrap();

    let data = Decoder::new(BufReader::new(file))
        .unwrap()
        .convert_samples()
        .buffered();
    let max: Option<f32> = data.clone().max_by(|x: &f32, y: &f32| x.total_cmp(y));
    println!("Max amplitude: {:?}", max.unwrap());

    let file = File::open(path.clone()).unwrap();
    let data = Decoder::new(BufReader::new(file)).unwrap();
    data
}

// fn play_background {
// 35 + 75 Hz in loop

// 75 fast immer laufen lassen
// 35 Hz gelegentlich zuschalten
// Zufallsüberlagerung. Wie?

// -> Letzte 3 Events in Massen

// M1 über 3 Minuten schwingen lassen

// Zu Beginn M1,2,3 sounds besser trennen

// 1/3 Langeweile, 2/3 Spannung (z.B. 200)

// auf neue Instrumemte warten (triangel 44-44.22, (+200) ~ als Entfernung) oder lautstaerke
// nach 2 min f. 10x einspielen <2000Pc<4000Pc<
// sounds für location 139-139.5

// 200Hz für maximales Chaos nach paar Sekunden (+ 201Hz für wenige Sekunden gleichzeitig)
// Lautstärke = Entfernung

// Wenn Ort klar lokalisiert ist, 44+44.22 Hz stärker, wenn Ort unklar ist, weniger
// Bild Unschärfe ansehen
// Zyklisch mit Überlappungen variieren
// Hauptsächlich spielen, wenn das relative M1,2,3 auch spielt

// Wenn unscharf: Grau ausschalten

// default volume per cli
// }

/*

M75 / M35


*/

trait SourceExt {
    #[inline]
    fn take_duration_with_fade(
        self,
        duration: Duration,
        fade_duration: Duration,
    ) -> TakeWithFade<Self>
    where
        Self: Sized,
        Self: Source,
        Self::Item: Sample,
    {
        take_with_fade::take_with_fade(self, duration, fade_duration)
    }
}

impl<Source> SourceExt for Source {}

fn sty(title: &str) -> ProgressStyle {
    ProgressStyle::with_template(
        format!(
            "{{spinner}} [{:3}] {{bar:40.cyan/blue}} {{pos:>7}}/{{len:7}} {{msg}}",
            title
        )
        .as_str(),
    )
    .unwrap()
    //.progress_chars("##-")
}

fn download_tsv(url: &str) -> Result<(&str, usize), Box<dyn std::error::Error>> {
    let filename = "O4_Events_cache.tsv";

    let content = reqwest::blocking::get(url)?.error_for_status()?;
    let text = content.text()?;
    let mut f = File::create(filename)?;
    let bytes = text.as_bytes();
    f.write_all(bytes)?;
    Ok((filename, bytes.len()))
}

fn main() {
    println!("==== {} ====", "GWrust".blue());

    let tsv_url = "https://raw.githubusercontent.com/Debilski/gwtracker_rust/main/O4_Events.tsv";

    let res = download_tsv(tsv_url);
    println!("Download status: {:?}", res);
    if let Ok((filename, _size)) = res {
        let elems = read_tsv(filename);

        if let Ok(el) = elems {
            println!("Last 3 events:");
            for i in el.len() - 3..el.len() {
                println!("{:?}", el[i]);
            }
        }
    }

    let source_m1 = source("sounds/M-1ab_130.mp3").buffered().repeat_infinite();
    let source_m2 = source("sounds/M-2ab_140.mp3").buffered().repeat_infinite();
    let source_m3 = source("sounds/M-3ab_150.mp3").buffered().repeat_infinite();
    let source_m35 = source("sounds/M35-perma.mp3").buffered().repeat_infinite();
    let source_m75 = source("sounds/M75-perma.mp3").buffered().repeat_infinite();

    let tria_44_00 = source("sounds/Triangle_44,00-50-loop.mp3").buffered().repeat_infinite();
    let tria_44_22 = source("sounds/Triangle_44,22-ca70-loop.mp3").buffered().repeat_infinite();
    let tria_44_23 = source("sounds/Triangle_44,23-100-loop.mp3").buffered().repeat_infinite();
    let tria_44_25 = source("sounds/Triangle_44,25-ca85-loop.mp3").buffered().repeat_infinite();
    let tria_200 = source("sounds/Triangle_200-ca70 10sec oh.mp3").buffered();
    let tria_201 = source("sounds/Triangle_201_ca30 10sec oh.mp3").buffered();
    let tria_202 = source("sounds/Triangle_202_ca20 2 sec oh.mp3").buffered();
    let tria_203 = source("sounds/Triangle_203_ca70 2 sec oh.mp3").buffered();

    let (controller, mixer) = dynamic_mixer::mixer(2, 44_100);
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let m = MultiProgress::new();
    let pb_m1 = m.add(ProgressBar::hidden());
    let pb_m2 = m.add(ProgressBar::hidden());
    let pb_m3 = m.add(ProgressBar::hidden());
    let pb_m35 = m.add(ProgressBar::hidden());
    let pb_m75 = m.add(ProgressBar::hidden());

    // let (pb_send, pb_recv) = mpsc::channel();
    // let (pb_m2_send, pb_m2_recv) = mpsc::channel();
    // let (pb_m3_send, pb_m3_recv) = mpsc::channel();
    // let (pb_m35_send, pb_m35_recv) = mpsc::channel();
    // let (pb_m75_send, pb_m75_recv) = mpsc::channel();

    {
        let progress_bars = vec![&pb_m1, &pb_m2, &pb_m3, &pb_m35, &pb_m75];

        for pb in progress_bars.iter() {
            pb.disable_steady_tick();

            //pb.set_message("...");
        }
    }

    pb_m1.set_style(sty("M1"));
    pb_m2.set_style(sty("M2"));
    pb_m3.set_style(sty("M3"));
    pb_m35.set_style(sty("M35"));
    pb_m75.set_style(sty("M75"));

    // thread::spawn(move || match pb_m1_recv.recv() {
    //     Ok(false) => pb_m1.finish_with_message("done"),
    //     Ok(true) => pb_m1.reset(),
    //     Err(_) => {}
    // });

    // Create queues
    let (tx_m1, rx_m1) = queue(true);
    let (tx_m2, rx_m2) = queue(true);
    let (tx_m3, rx_m3) = queue(true);
    let (tx_m35, rx_m35) = queue(true);
    let (tx_m75, rx_m75) = queue(true);

    let (tx_m44_00, rx_m44_00) = queue(true);
    let (tx_m44_22, rx_m44_22) = queue(true);
    let (tx_m200, rx_m200) = queue(true);
    let (tx_m201, rx_m201) = queue(true);

    controller.add(rx_m1);
    controller.add(rx_m2);
    controller.add(rx_m3);
    controller.add(rx_m35);
    controller.add(rx_m75);
    controller.add(rx_m44_00);
    controller.add(rx_m44_22);
    controller.add(rx_m200);
    controller.add(rx_m201);

    struct StartEnd {
        from: DateTime<Local>,
        until: DateTime<Local>,
    }
    impl StartEnd {
        fn new(from: DateTime<Local>, until: DateTime<Local>) -> Self {
            StartEnd { from, until }
        }
    }
    impl Debug for StartEnd {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "[from {} until {}]",
                self.from.format("%H:%M:%S"),
                self.until.format("%H:%M:%S")
            )
        }
    }

    let now_playing: Arc<DashMap<String, StartEnd>> = Arc::new(DashMap::new());

    fn play_once<S, Smpl>(
        log: &str,
        source: &S,
        queue: &Arc<SourcesQueueInput<Smpl>>,
        duration_secs: u64,
        fade_millis: u64,
        volume: f32,
        now_playing: &Arc<DashMap<String, StartEnd>>,
    ) where
        S: Source<Item = Smpl> + Send + Clone + 'static,
        S::Item: Sample,
        Smpl: Sample + Send + 'static,
    {
        println!(
            "Playing {} for {} seconds. (Vol: {})",
            log.red(),
            duration_secs,
            volume
        );
        let recv =
            queue.append_with_signal(source.clone().amplify(volume).take_duration_with_fade(
                Duration::from_secs(duration_secs),
                Duration::from_millis(fade_millis),
            ));
        let start = Local::now();
        let finished = start
            .checked_add_signed(chrono::Duration::seconds(duration_secs.try_into().unwrap()))
            .unwrap();
        let key = log.to_string();
        now_playing.insert(key.clone(), StartEnd::new(start, finished));
        let np = now_playing.clone();
        thread::spawn(move || {
            recv.recv();
            np.remove(&key);
            println!("Stopped {}.", &key);
        });
    }

    fn play_repeat<S, Smpl>(
        log: &str,
        source: &S,
        queue: &Arc<SourcesQueueInput<Smpl>>,
        duration_secs: u64,
        fade_millis: u64,
        volume: f32,
        now_playing: &Arc<DashMap<String, StartEnd>>,
    ) where
        S: Source<Item = Smpl> + Send + Clone + 'static,
        S::Item: Sample,
        Smpl: Sample + Send + 'static,
    {
        // TODO: Add constraints for repeat?
        play_once(
            log,
            source,
            queue,
            duration_secs,
            fade_millis,
            volume,
            now_playing,
        )
    }

    let play_m1 = {
        let np = now_playing.clone();
        move |secs: u64| play_repeat("M1", &source_m1, &tx_m1, secs, 100, 1.0, &np)
    };

    let play_m2 = {
        let np = now_playing.clone();
        move |secs: u64| play_repeat("M2", &source_m2, &tx_m2, secs, 100, 1.0, &np)
    };

    let play_m3 = {
        let np = now_playing.clone();
        move |secs: u64| play_repeat("M3", &source_m3, &tx_m3, secs, 100, 1.0, &np)
    };

    let play_m35 = {
        let np = now_playing.clone();
        move |secs: u64| play_repeat("M35", &source_m35, &tx_m35, secs, 500, 0.5, &np)
    };

    let play_m75 = {
        let np = now_playing.clone();
        move |secs: u64| play_repeat("M75", &source_m75, &tx_m75, secs, 500, 0.5, &np)
    };

    // Mit fadein?
    let play_m44_00 = {
        let np = now_playing.clone();
        let fade_in = Duration::from_secs(30);
        let with_fade = tria_44_00.fade_in(fade_in);
        move |secs: u64| play_once("M44.00", &with_fade, &tx_m44_00, secs, 30000, 0.33, &np)
    };

    // Mit fadein?
    let play_m44_22 = {
        let np = now_playing.clone();
        let fade_in = Duration::from_secs(30);
        let with_fade = tria_44_22.fade_in(fade_in);
        move |secs: u64| play_once("M44.22", &with_fade, &tx_m44_22, secs, 25000, 0.33, &np)
    };

    let play_m200 = {
        let np = now_playing.clone();
        move |secs: u64| play_once("M200.00", &tria_200, &tx_m200, secs, 500, 0.05, &np)
    };

    let play_m201 = {
        let np = now_playing.clone();
        move |secs: u64| play_once("M201.00", &tria_201, &tx_m201, secs, 500, 0.05, &np)
    };

    sink.append(mixer);
    //sink.set_speed(1);
    //sink.set_volume(0.3);

    m.println("starting!").unwrap();

    {
        let now_playing = now_playing.clone();

        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            println!("{:?}", now_playing);
        });
    }

    loop {
        let mut rng = rand::thread_rng();
        // Looping for around ten minutes:
        let mut remainder: u32 = 60 * 10;

        let mut sleep = |secs: u32, fuzzy: bool| {
            if fuzzy {
                let fuzz: i64 = rng.gen_range(-200..=200);
                let secs: i64 = secs.into();
                let millis: i64 = secs * 1000;
                let total = millis + fuzz;
                thread::sleep(Duration::from_millis(u64::try_from(total).unwrap_or(0)));
            } else {
                thread::sleep(Duration::from_secs(secs.into()));
            }
            remainder = remainder.checked_sub(secs).unwrap_or(0);
        };

        // Starting with only M35 for ~ 30 seconds
        play_m35(10 * 60);
        sleep(30, false);
        // m75 will play for ~ 9 minutes
        play_m75(9 * 60);
        sleep(30, false);

        // // First run of Masses: M1/M2 for ~ 3:30 minutes
        play_m1(210);
        sleep(10, false);
        play_m2(190);
        sleep(5 * 10, false);

        // Short bursts of M200, M201 for 20 seconds
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        sleep(4, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        sleep(4, true);

        sleep(5 * 10, false);


        play_m44_00(25 * 10);
        sleep(10, false);
        play_m44_22(23 * 10);


        sleep(14 * 10, false);

        // Second run of Masses: M1/M2 for ~ 3:30 minutes
        play_m1(210);
        sleep(10, false);
        play_m2(190);

        // Short bursts of M200, M201 for 20 seconds
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        sleep(4, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        play_m200(2);
        sleep(1, true);
        play_m201(2);
        sleep(1, true);
        sleep(4, true);

        //        sleep(200, false);

        thread::sleep(Duration::from_secs(remainder.into())); // wait until silence
    }
}
