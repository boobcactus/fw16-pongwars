use anyhow::Result;
use clap::Parser;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod game;
mod led_matrix;

use game::{GameState, DEFAULT_GRID_HEIGHT};
use led_matrix::LedMatrix;

#[derive(Parser, Debug)]
#[command(author, version, about = "Framework Laptop 16 Pong Wars", long_about = None)]
struct Args {
    #[arg(short = 'd', long = "dualmode")]
    dual_mode: bool,

    #[arg(short = 'b', long = "balls", num_args = 0..=1, default_missing_value = "2", value_parser = clap::value_parser!(u8).range(1..=20))]
    balls: Option<u8>,

    #[arg(short = 's', long = "speed", default_value_t = 64, value_parser = clap::value_parser!(u8).range(1..=64))]
    speed: u8,

    #[arg(short = 'B', long = "brightness", default_value_t = 50, value_parser = clap::value_parser!(u8).range(0..=100))]
    brightness: u8,

    #[arg(long = "debug")]
    debug: bool,
}

fn percent_to_led_value(percent: u8) -> u8 {
    ((percent as u16 * 255) / 100) as u8
}

fn main() -> Result<()> {
    let args = Args::parse();

    let brightness_value = percent_to_led_value(args.brightness);
    let brightness_atomic = Arc::new(AtomicU8::new(brightness_value));

    let mut matrix = LedMatrix::new_with_brightness(
        brightness_atomic.clone(),
        args.dual_mode,
        DEFAULT_GRID_HEIGHT,
    )?;
    matrix.set_brightness(brightness_value)?;

    let width = matrix.width();
    let max_fps = matrix.estimated_max_fps() as u8;
    let effective_fps = args.speed.min(max_fps).max(1);
    println!(
        "Starting Pong Wars (width={} height={} speed={}fps brightness={}%)",
        width, DEFAULT_GRID_HEIGHT, effective_fps, args.brightness
    );

    ctrlc::set_handler(|| {
        println!("Received interrupt, shutting down...");
        SHUTDOWN.store(true, Ordering::SeqCst);
    })?;

    let balls_per_team: u8 = args.balls.unwrap_or(1);
    run_game_loop(&mut matrix, effective_fps, brightness_atomic, args.debug, balls_per_team)?;

    println!("Exited cleanly.");
    Ok(())
}

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

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

    let mut last_sent_brightness = brightness.load(Ordering::SeqCst);
    while !SHUTDOWN.load(Ordering::SeqCst) {
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

            game_state.update();

            if let Err(e) = matrix.render(&game_state) {
                eprintln!("Render error: {}", e);
                std::thread::sleep(Duration::from_millis(10));
            }

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

            if let Some(coarse_sleep) = sleep_duration.checked_sub(Duration::from_millis(1)) {
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

        let desired_brightness = brightness.load(Ordering::SeqCst);
        if desired_brightness != last_sent_brightness {
            matrix.set_brightness(desired_brightness)?;
            last_sent_brightness = desired_brightness;
        }
    }

    Ok(())
}
