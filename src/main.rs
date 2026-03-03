#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod game;
mod led_matrix;
mod power;
mod settings;
#[cfg(windows)]
mod settings_dialog;
mod tray;

use game::{GameState, DEFAULT_GRID_HEIGHT};
use led_matrix::LedMatrix;

#[derive(Parser, Debug)]
#[command(author, version, about = "Framework Laptop 16 Pong Wars", long_about = None)]
struct Args {
    #[arg(short = 'd', long = "dualmode")]
    dual_mode: bool,

    #[arg(short = 'b', long = "balls", num_args = 0..=1, default_missing_value = "2", value_parser = clap::value_parser!(u8).range(1..=20))]
    balls: Option<u8>,

    #[arg(short = 's', long = "speed", value_parser = clap::value_parser!(u8).range(1..=64))]
    speed: Option<u8>,

    #[arg(short = 'B', long = "brightness", value_parser = clap::value_parser!(u8).range(0..=100))]
    brightness: Option<u8>,

    #[arg(long = "debug")]
    debug: bool,

    #[arg(long = "settings", help = "Path to persistent settings TOML file")]
    settings: Option<PathBuf>,
}

fn has_explicit_game_flags(args: &Args) -> bool {
    args.dual_mode || args.balls.is_some() || args.speed.is_some()
        || args.brightness.is_some() || args.debug
}

fn percent_to_led_value(percent: u8) -> u8 {
    ((percent as u16 * 255) / 100) as u8
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Enforce mutual exclusion: --settings vs game flags
    if args.settings.is_some() && has_explicit_game_flags(&args) {
        eprintln!("Error: --settings cannot be combined with --dualmode, --balls, --speed, --brightness, or --debug flags.");
        std::process::exit(1);
    }

    // Resolve effective settings via three modes
    let (settings, settings_path) = if let Some(ref path) = args.settings {
        // Mode 1: --settings file (load or create with defaults)
        let s = settings::Settings::load_or_create(path)?;
        (s, Some(path.clone()))
    } else if has_explicit_game_flags(&args) {
        // Mode 2: explicit CLI flags
        let s = settings::Settings {
            dual_mode: args.dual_mode,
            balls: args.balls.unwrap_or(2),
            speed: args.speed.unwrap_or(32),
            brightness: args.brightness.unwrap_or(40),
            debug: args.debug,
            start_with_windows: false,
        };
        (s, None)
    } else {
        // Mode 3: bare run, hardcoded defaults
        (settings::Settings::default(), None)
    };

    // Apply startup registry when using a settings file
    #[cfg(windows)]
    if let Some(ref sp) = settings_path {
        if let Err(e) = settings.apply_startup_registry(sp) {
            eprintln!("Warning: could not update startup registry: {}", e);
        }
    }

    let brightness_value = percent_to_led_value(settings.brightness);
    let brightness_atomic = Arc::new(AtomicU8::new(brightness_value));

    let mut matrix = LedMatrix::new_with_brightness(
        brightness_atomic.clone(),
        settings.dual_mode,
        DEFAULT_GRID_HEIGHT,
    )?;

    let resume_flag = matrix.resume_flag();
    let suspend_sync = matrix.suspend_sync();
    #[cfg(windows)]
    let _power_guard = match power::register_power_notification(resume_flag, suspend_sync) {
        Ok(guard) => {
            println!("Registered Windows power notification (suspend + resume).");
            Some(guard)
        }
        Err(e) => {
            eprintln!("Warning: could not register power notification: {}", e);
            None
        }
    };
    #[cfg(not(windows))]
    let _ = (resume_flag, suspend_sync);

    let width = matrix.width();
    let max_fps = matrix.estimated_max_fps() as u8;
    let effective_fps = settings.speed.min(max_fps).max(1);
    println!(
        "Starting Pong Wars (width={} height={} speed={}fps brightness={}%)",
        width, DEFAULT_GRID_HEIGHT, effective_fps, settings.brightness
    );

    ctrlc::set_handler(|| {
        println!("Received interrupt, shutting down...");
        SHUTDOWN.store(true, Ordering::Release);
    })?;

    #[cfg(windows)]
    let tray_handle = {
        let sp = settings_path.clone();
        let st = settings.clone();
        std::thread::spawn(move || {
            tray::run_tray(&SHUTDOWN, &PAUSED, &RESET_REQUESTED, sp, st);
        })
    };

    let balls_per_team: u8 = settings.balls;
    run_game_loop(&mut matrix, effective_fps, brightness_atomic, settings.debug, balls_per_team)?;

    #[cfg(windows)]
    let _ = tray_handle.join();

    println!("Exited cleanly.");
    Ok(())
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static PAUSED: AtomicBool = AtomicBool::new(false);
static RESET_REQUESTED: AtomicBool = AtomicBool::new(false);

fn run_game_loop(
    matrix: &mut LedMatrix,
    target_fps: u8,
    brightness: Arc<AtomicU8>,
    debug: bool,
    balls_per_team: u8,
) -> Result<()> {
    let width = matrix.width();
    let mut game_state = GameState::new(width, DEFAULT_GRID_HEIGHT, balls_per_team);

    let frame_duration = Duration::from_secs_f64(1.0 / target_fps as f64);
    let mut next_frame_time = Instant::now();
    let mut last_frame_start = next_frame_time;
    let mut frame_index: u64 = 0;

    let mut last_sent_brightness = brightness.load(Ordering::Relaxed);
    while !SHUTDOWN.load(Ordering::Relaxed) {
        let now = Instant::now();

        if now >= next_frame_time {
            if debug {
                let actual_dt = now.saturating_duration_since(last_frame_start);
                let scheduled_next = next_frame_time + frame_duration;
                println!(
                    "[debug] frame {} start={:?} deadline={:?} actual_dt={:?} next_deadline={:?}",
                    frame_index, now, next_frame_time, actual_dt, scheduled_next
                );
            }

            if RESET_REQUESTED.swap(false, Ordering::Relaxed) {
                game_state = GameState::new(width, DEFAULT_GRID_HEIGHT, balls_per_team);
            }

            if !PAUSED.load(Ordering::Relaxed) {
                game_state.update();
            }

            if let Err(e) = matrix.render(&game_state) {
                eprintln!("Render error: {}", e);
                std::thread::sleep(Duration::from_millis(10));
            }

            if matrix.just_reconnected() {
                game_state.reset_kickoff();
            }

            let now = Instant::now();
            let scheduled_next = next_frame_time + frame_duration;
            if now.saturating_duration_since(next_frame_time) > frame_duration {
                next_frame_time = now + frame_duration;
            } else {
                next_frame_time = scheduled_next;
            }
            last_frame_start = now;
            frame_index = frame_index.wrapping_add(1);
        } else {
            let sleep_duration = next_frame_time.saturating_duration_since(now);

            if let Some(coarse_sleep) = sleep_duration.checked_sub(Duration::from_micros(500)) {
                if debug {
                    println!(
                        "[debug] sleeping {:?} before spin (coarse={:?})",
                        sleep_duration, coarse_sleep
                    );
                }
                std::thread::sleep(coarse_sleep);
            }

            while Instant::now() < next_frame_time {
                std::hint::spin_loop();
            }
            if debug {
                println!(
                    "[debug] spin-wait completed; woke at {:?} for deadline {:?}",
                    Instant::now(),
                    next_frame_time
                );
            }
        }

        let desired_brightness = brightness.load(Ordering::Relaxed);
        if desired_brightness != last_sent_brightness {
            matrix.set_brightness(desired_brightness)?;
            last_sent_brightness = desired_brightness;
        }
    }

    Ok(())
}
