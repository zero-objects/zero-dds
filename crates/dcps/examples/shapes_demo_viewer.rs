//! ShapesDemo-Live-Viewer (Terminal).
//!
//! Subscribiert auf alle drei Standard-Shapes-Topics (Square / Circle /
//! Triangle) und zeichnet die jeweils zuletzt empfangenen Sample-
//! Positionen als Unicode-Blocks in einem ANSI-Terminal-Fenster.
//!
//! Interop-Test:
//! - Starte `rtishapesdemo` (RTI 7.7.0), Cyclone- oder FastDDS-
//!   ShapesDemo, oder den ZeroDDS-Publisher (`shapes_demo_publisher`),
//!   und du siehst die Forme live im Terminal wandern.
//! - Multiple Publishers (z.B. Cyclone schickt Square, ZeroDDS schickt
//!   Circle, RTI schickt Triangle) sind moeglich — jeder publisher
//!   landet auf seinem eigenen Topic.
//!
//! # Usage
//!
//! ```text
//! cargo run -p zerodds-dcps --example shapes_demo_viewer
//! cargo run -p zerodds-dcps --example shapes_demo_viewer -- 0   # domain id
//! ```
//!
//! Beenden: Ctrl-C.
//!
//! # Layout
//!
//! ShapesDemo-Canvas geht (per Cyclone/RTI/FastDDS-Default) von 0..240
//! horizontal und 0..270 vertikal. Wir mappen das auf 80×30 Terminal-
//! Zellen. Farbe der Forme wird via ANSI-256-Color-Code re-mapped
//! (BLUE → Blau, RED → Rot, etc.).

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use zerodds_dcps::interop::ShapeType;
use zerodds_dcps::{
    DataReaderQos, DomainParticipantFactory, DomainParticipantQos, SubscriberQos, TopicQos,
};

const VIEW_W: usize = 80;
const VIEW_H: usize = 30;
const SHAPES_CANVAS_W: i32 = 240;
const SHAPES_CANVAS_H: i32 = 270;

/// ANSI-Foreground-Color-Code fuer ShapesDemo-Standard-Color-Strings.
fn ansi_color(name: &str) -> &'static str {
    match name.to_ascii_uppercase().as_str() {
        "BLUE" => "\x1B[34m",
        "RED" => "\x1B[31m",
        "GREEN" => "\x1B[32m",
        "ORANGE" => "\x1B[38;5;208m",
        "YELLOW" => "\x1B[33m",
        "MAGENTA" => "\x1B[35m",
        "CYAN" => "\x1B[36m",
        "PURPLE" => "\x1B[38;5;93m",
        _ => "\x1B[37m",
    }
}

const ANSI_RESET: &str = "\x1B[0m";

fn glyph(topic: &str) -> char {
    match topic {
        "Square" => '■',
        "Circle" => '●',
        "Triangle" => '▲',
        _ => '?',
    }
}

fn map_x(x: i32) -> usize {
    let r = x.clamp(0, SHAPES_CANVAS_W - 1);
    (usize::try_from(r).unwrap_or(0) * VIEW_W) / usize::try_from(SHAPES_CANVAS_W).unwrap_or(1)
}

fn map_y(y: i32) -> usize {
    let r = y.clamp(0, SHAPES_CANVAS_H - 1);
    (usize::try_from(r).unwrap_or(0) * VIEW_H) / usize::try_from(SHAPES_CANVAS_H).unwrap_or(1)
}

fn install_signal_handler(stop: Arc<AtomicBool>) {
    // Best-effort: SIGINT setzt stop-flag.
    let s = stop.clone();
    ctrlc_setter(move || s.store(true, Ordering::Relaxed));
}

#[cfg(target_os = "linux")]
fn ctrlc_setter<F: Fn() + Send + Sync + 'static>(f: F) {
    use std::sync::Mutex;
    static HOOK: Mutex<Option<Box<dyn Fn() + Send + Sync>>> = Mutex::new(None);
    if let Ok(mut g) = HOOK.lock() {
        *g = Some(Box::new(f));
    }
    extern "C" fn handler(_: i32) {
        if let Ok(g) = HOOK.lock() {
            if let Some(h) = g.as_ref() {
                h();
            }
        }
    }
    // SAFETY: libc::signal nimmt C-ABI-Funktionspointer; `handler` ist
    // `extern "C"` und hat exakt die erwartete Signatur (i32).
    unsafe {
        libc::signal(libc::SIGINT, handler as usize);
    }
}

#[cfg(not(target_os = "linux"))]
fn ctrlc_setter<F: Fn() + Send + Sync + 'static>(_: F) {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let domain_id: i32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    let stop = Arc::new(AtomicBool::new(false));
    install_signal_handler(stop.clone());

    let factory = DomainParticipantFactory::instance();
    let participant = factory.create_participant(domain_id, DomainParticipantQos::default())?;
    let subscriber = participant.create_subscriber(SubscriberQos::default());

    let topics = ["Square", "Circle", "Triangle"];
    let mut readers = Vec::new();
    for t in &topics {
        let topic = participant.create_topic::<ShapeType>(t, TopicQos::default())?;
        let reader = subscriber.create_datareader::<ShapeType>(&topic, DataReaderQos::default())?;
        readers.push((*t, reader));
    }

    eprintln!(
        "shapes_demo_viewer: Domain={domain_id} — subscribed to Square / Circle / Triangle. Ctrl-C beendet."
    );
    eprintln!("Warte auf Discovery + erste Samples...");

    // Hide cursor + clear screen.
    print!("\x1B[?25l\x1B[2J");
    let mut stdout = std::io::stdout();
    stdout.flush().ok();

    // Pro (topic, color) speichern wir die letzte Position.
    let mut shapes: HashMap<(String, String), (i32, i32)> = HashMap::new();
    let mut sample_count: u64 = 0;
    let mut grid: Vec<Vec<(char, &'static str)>> = vec![vec![(' ', ""); VIEW_W]; VIEW_H];

    while !stop.load(Ordering::Relaxed) {
        // 1) Take all pending samples per topic.
        for (topic_name, reader) in &readers {
            if let Ok(samples) = reader.take() {
                for sample in samples {
                    shapes.insert(
                        ((*topic_name).to_string(), sample.color.clone()),
                        (sample.x, sample.y),
                    );
                    sample_count += 1;
                }
            }
        }

        // 2) Clear grid.
        for row in &mut grid {
            for cell in row {
                *cell = (' ', "");
            }
        }

        // 3) Plot shapes.
        for ((topic, color), (x, y)) in &shapes {
            let gx = map_x(*x);
            let gy = map_y(*y);
            if gy < VIEW_H && gx < VIEW_W {
                grid[gy][gx] = (glyph(topic), ansi_color(color));
            }
        }

        // 4) Render: home, draw, status line.
        print!("\x1B[H");
        // top border
        print!("┌");
        for _ in 0..VIEW_W {
            print!("─");
        }
        println!("┐");
        for row in &grid {
            print!("│");
            for (ch, color) in row {
                if color.is_empty() {
                    print!(" ");
                } else {
                    print!("{color}{ch}{ANSI_RESET}");
                }
            }
            println!("│");
        }
        print!("└");
        for _ in 0..VIEW_W {
            print!("─");
        }
        println!("┘");
        println!(
            "shapes={:3} samples={:6} domain={} — Ctrl-C beendet                            ",
            shapes.len(),
            sample_count,
            domain_id,
        );
        stdout.flush().ok();

        thread::sleep(Duration::from_millis(60));
    }

    // Show cursor again.
    print!("\x1B[?25h");
    println!("[shapes_demo_viewer] beendet. Total samples empfangen: {sample_count}");
    Ok(())
}
