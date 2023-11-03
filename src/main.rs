mod log_source;
mod take_with_fade;

use std::error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, sleep};
use std::time::Duration;

use rand::Rng;

use rodio::queue::queue;
use rodio::source::{SineWave, Source};
use rodio::{dynamic_mixer, Decoder, OutputStream, Sample, Sink};

use tokio;
use tokio::select;

use clokwerk::{Job, Scheduler, TimeUnits};
// Import week days and WeekDay
use clokwerk::Interval::*;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use log_source::LogSource;
use take_with_fade::TakeWithFade;

const FADEOUTTIME: u64 = 0;

type SourceOnce = Decoder<BufReader<File>>;
type SourceInfinite = rodio::source::Repeat<SourceOnce>;

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
    let data = Decoder::new(BufReader::new(file))
        .unwrap();
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

// default volume per cli
// }

trait SourceExt {
    #[inline]
    fn log_source(self, str: String) -> LogSource<Self>
    where
        Self: Sized,
        Self: Source,
        Self::Item: Sample,
    {
        log_source::log_source(self, str)
    }

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


fn main() {
    let source_m1 = source("sounds/M-1ab_140.mp3").buffered().repeat_infinite();
    let source_m2 = source("sounds/M-2ab_130.mp3").buffered().repeat_infinite();
    let source_m3 = source("sounds/M-3ab_150.mp3").buffered().repeat_infinite();
    let source_m35 = source("sounds/M35-perma.mp3").buffered().repeat_infinite();
    let source_m75 = source("sounds/M75-perma.mp3").buffered().repeat_infinite();

    let tria_44_00 = source("sounds/Triangle_44,00-50-loop.mp3").buffered();
    let tria_44_22 = source("sounds/Triangle_44,22-ca70-loop.mp3").buffered();
    let tria_44_23 = source("sounds/Triangle_44,23-100-loop.mp3").buffered();
    let tria_44_25 = source("sounds/Triangle_44,25-ca85-loop.mp3").buffered();
    let tria_200 = source("sounds/Triangle_200-ca70 2 sec oh.mp3").buffered();
    let tria_201 = source("sounds/Triangle_201_ca30 2 sec oh.mp3").buffered();
    let tria_202 = source("sounds/Triangle_202_ca20 2 sec oh.mp3").buffered();
    let tria_203 = source("sounds/Triangle_203_ca70 2 sec oh.mp3").buffered();

    // Construct a dynamic controller and mixer, stream_handle, and sink.
    // TODO Define sample type? ::<f32>?
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
    let (tx_m1, mut rx_m1) = queue(true);
    let (tx_m2, mut rx_m2) = queue(true);
    let (tx_m3, mut rx_m3) = queue(true);
    let (tx_m35, mut rx_m35) = queue(true);
    let (tx_m75, mut rx_m75) = queue(true);

    let (tx_m44_00, mut rx_m44_00) = queue(true);
    let (tx_m44_22, mut rx_m44_22) = queue(true);
    let (tx_m200, mut rx_m200) = queue(true);
    let (tx_m201, mut rx_m201) = queue(true);

    controller.add(rx_m1);
    controller.add(rx_m2);
    controller.add(rx_m3);
    controller.add(rx_m35);
    controller.add(rx_m75);
    controller.add(rx_m44_00);
    controller.add(rx_m44_22);
    controller.add(rx_m200);
    controller.add(rx_m201);


    let play_m1 = move |secs: u64| {
        println!("Playing M1 for {} seconds.", secs);
        let recv = tx_m1.append_with_signal(
            source_m1
                .clone()
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(100)),
        );
        recv
    };

    let play_m2 = move |secs: u64| {
        println!("Playing M2 for {} seconds.", secs);
        let recv = tx_m2.append_with_signal(
            source_m2
                .clone()
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(100)),
        );
        recv
    };

    let play_m3 = |secs: u64| {
        println!("Playing M3 for {} seconds.", secs);
        let recv = tx_m3.append_with_signal(
            source_m3
                .clone()
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(100)),
        );
        recv
    };

    let play_m35 = move |secs: u64| {
        println!("Playing M35 for {} seconds.", secs);
        let recv =
            tx_m35.append_with_signal(source_m35
                .clone()
                .amplify(1.)
                .take_duration_with_fade(
                Duration::from_secs(secs),
                Duration::from_millis(500),
            ));
        recv
    };
    let play_m75 = move |secs: u64| {
        println!("Playing M75 for {} seconds.", secs);
        let recv = tx_m75.append_with_signal(
            source_m75.clone()
                .amplify(1.)
                .take_duration_with_fade(
                Duration::from_secs(secs),
                Duration::from_millis(500),
            ),
        );
        recv
    };

    let play_m44_00 = move |secs: u64| {
        println!("Playing M44.00 for {} seconds.", secs);
        tx_m44_00.append_with_signal(
            tria_44_00
                .clone()
                .amplify(0.33)
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(3000)),
        )
    };

    let play_m44_22 = move |secs: u64| {
        println!("Playing M44.22 for {} seconds.", secs);
        tx_m44_22.append_with_signal(
            tria_44_22
                .clone()
                .amplify(0.33)
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(2500)),
        )
    };

    let play_m200 = move |secs: u64| {
        println!("Playing M200.00 for {} seconds.", secs);
        tx_m200.append_with_signal(
            tria_200
                .clone()
                .amplify(0.2)
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(500)),
        )
    };

    let play_m201 = move |secs: u64| {
        println!("Playing M201.00 for {} seconds.", secs);
        tx_m201.append_with_signal(
            tria_201
                .clone()
                .amplify(0.2)
                .take_duration_with_fade(Duration::from_secs(secs), Duration::from_millis(500)),
        )
    };

    sink.append(mixer);
    //sink.set_speed(1);
    //sink.set_volume(0.3);

    let mut scheduler = Scheduler::with_tz(chrono::Utc);

    let c = controller.clone();

    // scheduler.every(Seconds(0)).once().run(move || {
    //     let (c, mixer) = dynamic_mixer::mixer::<f32>(2, 44_100);
    //     let mut source_c = take_duration_with_fade(
    //         SineWave::new(220.0),
    //         Duration::from_secs(1),
    //         Duration::from_millis(5000),
    //     )
    //     .fade_in(Duration::from_millis(100));
    //     //let mut source_a = take_duration_with_fade(SineWave::new(330.63), Duration::from_secs(10), Duration::from_millis(1000)).fade_in(Duration::from_millis(100));
    //     // let mut source_c = SineWave::new(261.63)
    //     // .fade_in(Duration::from_millis(100))
    //     // .take_duration(Duration::from_secs_f32(2.))
    //     // ;
    //     // source_c.set_filter_fadeout();
    //     //let source_c = source_c.amplify(0.20);

    //     //        let source_c = source_m75().convert_samples();
    //     //c.add(source_a);
    //     c.add(source_c);
    //     controller.add(mixer);
    // });

    m.println("starting!").unwrap();

    // let rxx_m1 = play_m1(0);
    // let rxx_m2 = play_m2(0);
    // let rxx_m3 = play_m3(0);

    thread::spawn(move || {
        let mut rng = rand::thread_rng();
        thread::sleep(Duration::from_secs(0));

        //println!("starting 35");
        let rxx_m35 = play_m35(60);
        thread::sleep(Duration::from_secs(50));
        //        pb_m35.reset();
        //pb_m35.enable_steady_tick(Duration::from_millis(200));

        loop {
            //        let play_time_35 = rng.gen_range(5..10);
            //        let play_time_75 = rng.gen_range(5..10);

            let rxx_m75 = play_m75(120);
            thread::sleep(Duration::from_secs(100));
            //            pb_m75.reset();
            //            pb_m75.enable_steady_tick(Duration::from_millis(200));

            //33333333    33333333
            //xxxxxx77777777    77777777

            let rxx_m35 = play_m35(120);
            thread::sleep(Duration::from_secs(100));
            pb_m35.reset();
        }

        // for i in 0..1024 {

        //     thread::sleep(Duration::from_millis(2));
        //     pb_m2.set_message(format!("item #{}", i + 1));
        //     pb_m2.inc(1);
        // }
        // //m_clone.println("pb3 is done!").unwrap();
        // pb_m2.finish_with_message("done");
    });

    thread::spawn(move || {
        let mut rng = rand::thread_rng();
        thread::sleep(Duration::from_secs(80));

        let rxx_m35 = play_m1(30);
        thread::sleep(Duration::from_secs(25));
        // pb_m1.reset();

        loop {
            let rxx_m75 = play_m2(30);
            thread::sleep(Duration::from_secs(60));

            // pb_m2.reset();

            let rxx_m35 = play_m1(30);
            thread::sleep(Duration::from_secs(25));
            // pb_m1.reset();
        }
    });

    thread::spawn(move || {
        let mut rng = rand::thread_rng();
        thread::sleep(Duration::from_secs(120));

        loop {

            play_m44_00(31);
            play_m44_22(30);
            thread::sleep(Duration::from_secs(12));
            play_m200(5);
            play_m200(5);
            play_m200(5);

            thread::sleep(Duration::from_secs(200));
        }
    });

    //let mut threads = vec![];

    // let m_clone = m.clone();
    // let h3 = thread::spawn(move || {
    //     for i in 0..1024 {
    //         thread::sleep(Duration::from_millis(2));
    //         pb_m2.set_message(format!("item #{}", i + 1));
    //         pb_m2.inc(1);
    //     }
    //     //m_clone.println("pb3 is done!").unwrap();
    //     pb_m2.finish_with_message("done");
    // });

    // thread::spawn(move || {
    //     let mut amp = 0.1;

    //     loop {
    //         amp += 0.001;
    //         match rxx_m1.recv_timeout(Duration::from_millis(10)) {
    //             Ok(_) => break,
    //             Err(_) => {}
    //         }
    //     }
    //     pb_m1_send.send(true);
    //     //pb_m1.finish_with_message("done")
    // });

    // thread::spawn(move || {
    //     rxx_m3.recv();
    //     pb_m3.finish_with_message("done")
    // });

    // Or run it in a background thread
    let thread_handle = scheduler.watch_thread(Duration::from_millis(100));
    // The scheduler stops when `thread_handle` is dropped, or `stop` is called

    // if let Ok(_) = rxx_m2.recv_timeout(Duration::from_millis(40)) {
    //     println!("Finished");
    // }
    sleep(Duration::from_secs(10));

    // tx_m1.append_with_signal(take_duration_with_fade(
    //     source_m1,
    //     Duration::from_secs(15),
    //     Duration::from_millis(100),
    // ));

    // Sleep the thread until sink is empty.
    sink.sleep_until_end();

    thread_handle.stop();
}

fn main3() {
    let m = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .progress_chars("##-");

    let n = 200;
    let pb = m.add(ProgressBar::new(n));
    pb.set_style(sty.clone());
    pb.set_message("todo");
    let pb2 = m.add(ProgressBar::new(n));
    pb2.set_style(sty.clone());
    pb2.set_message("finished");

    let pb3 = m.insert_after(&pb2, ProgressBar::new(1024));
    pb3.set_style(sty);

    m.println("starting!").unwrap();

    let mut threads = vec![];

    let m_clone = m.clone();
    let h3 = thread::spawn(move || {
        for i in 0..1024 {
            thread::sleep(Duration::from_millis(2));
            pb3.set_message(format!("item #{}", i + 1));
            pb3.inc(1);
        }
        m_clone.println("pb3 is done!").unwrap();
        pb3.finish_with_message("done");
    });

    for i in 0..n {
        thread::sleep(Duration::from_millis(15));
        if i == n / 3 {
            thread::sleep(Duration::from_secs(2));
        }
        pb.inc(1);
        let m = m.clone();
        let pb2 = pb2.clone();
        threads.push(thread::spawn(move || {
            let spinner = m.add(ProgressBar::new_spinner().with_message(i.to_string()));
            spinner.enable_steady_tick(Duration::from_millis(100));
            thread::sleep(
                rand::thread_rng().gen_range(Duration::from_secs(1)..Duration::from_secs(5)),
            );
            pb2.inc(1);
        }));
    }
    pb.finish_with_message("all jobs started");

    for thread in threads {
        let _ = thread.join();
    }
    let _ = h3.join();
    pb2.finish_with_message("all jobs done");
    m.clear().unwrap();
}
