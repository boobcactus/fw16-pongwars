use anyhow::{anyhow, Result};
use serialport::{DataBits, Parity, SerialPort, StopBits};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::game::{GameState, SquareColor};

const BAUD_RATE: u32 = 115200;
const TIMEOUT_MS: u64 = 1000;

// Framework LED Matrix Protocol Constants
const MAGIC_WORD: [u8; 2] = [0x32, 0xAC];

// Command IDs
const CMD_BRIGHTNESS: u8 = 0x00;
const CMD_DRAW_BW: u8 = 0x06;
const MODULE_WIDTH: usize = 9;

// Flow control constants
const HEARTBEAT_INTERVAL: u64 = 64; // Frames between health checks (~1s at 64fps)
const RECOVERY_INTERVAL_MS: u64 = 2000; // Minimum ms between reconnect attempts

#[derive(Debug, Clone, Copy, PartialEq)]
enum MatrixState {
    Active,
    Suspended,
    Recovering,
}

struct MatrixPort {
    port: Box<dyn SerialPort>,
    width: usize,
    bw_buffer: [u8; 42],
}

impl MatrixPort {
    fn new(port: Box<dyn SerialPort>, _height: usize) -> Result<Self> {
        if let Err(e) = port.clear(serialport::ClearBuffer::All) {
            return Err(anyhow!("Failed clearing port: {}", e));
        }
        thread::sleep(Duration::from_millis(100));

        let mut bw_buffer = [0u8; 42];
        bw_buffer[0] = MAGIC_WORD[0];
        bw_buffer[1] = MAGIC_WORD[1];
        bw_buffer[2] = CMD_DRAW_BW;

        Ok(MatrixPort {
            port,
            width: MODULE_WIDTH,
            bw_buffer,
        })
    }
}

pub struct LedMatrix {
    ports: Vec<MatrixPort>,
    brightness: Arc<AtomicU8>,
    state: MatrixState,
    frame_count: u64,
    last_recovery_attempt: Instant,
    resume_flag: Arc<AtomicBool>,
    width: usize,
    height: usize,
}

impl LedMatrix {
    pub fn new_with_brightness(brightness: Arc<AtomicU8>, dual_mode: bool, height: usize) -> Result<Self> {
        let mut candidates: Vec<serialport::SerialPortInfo> = serialport::available_ports()?
            .into_iter()
            .filter(|p| matches!(p.port_type, serialport::SerialPortType::UsbPort(ref info) if info.vid == 0x32AC && (info.pid == 0x0020 || info.pid == 0x0021)))
            .collect();

        if candidates.is_empty() {
            return Err(anyhow!("No Framework LED Matrix modules found."));
        }

        candidates.sort_by(|a, b| {
            let sa = match &a.port_type {
                serialport::SerialPortType::UsbPort(info) => info.serial_number.as_deref(),
                _ => None,
            };
            let sb = match &b.port_type {
                serialport::SerialPortType::UsbPort(info) => info.serial_number.as_deref(),
                _ => None,
            };
            match (sa, sb) {
                (Some(aa), Some(bb)) => aa.cmp(bb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.port_name.cmp(&b.port_name),
            }
        });

        let mut desired_ports = if dual_mode {
            if candidates.len() < 2 {
                return Err(anyhow!("Dual mode requested but only {} LED Matrix module detected.", candidates.len()));
            }
            candidates.truncate(2);
            candidates
        } else {
            candidates.truncate(1);
            candidates
        };

        if dual_mode && desired_ports.len() == 2 {
            desired_ports.reverse();
            println!(
                "Auto-ordered modules: {} = right, {} = left",
                desired_ports[0].port_name, desired_ports[1].port_name
            );
        }

        let mut matrix_ports: Vec<MatrixPort> = Vec::new();
        for info in desired_ports {
            match serialport::new(&info.port_name, BAUD_RATE)
                .timeout(Duration::from_millis(TIMEOUT_MS))
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .open()
            {
                Ok(port) => match MatrixPort::new(Box::from(port), height) {
                    Ok(matrix_port) => {
                        println!("Connected LED Matrix on {}", info.port_name);
                        matrix_ports.push(matrix_port);
                    }
                    Err(e) => eprintln!("Failed initializing port {}: {}", info.port_name, e),
                },
                Err(e) => eprintln!("Failed opening port {}: {}", info.port_name, e),
            }
        }

        if matrix_ports.is_empty() {
            return Err(anyhow!("Unable to open any Framework LED Matrix modules."));
        }

        Ok(LedMatrix {
            width: matrix_ports.len() * MODULE_WIDTH,
            height,
            ports: matrix_ports,
            brightness,
            state: MatrixState::Active,
            frame_count: 0,
            last_recovery_attempt: Instant::now(),
            resume_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn resume_flag(&self) -> Arc<AtomicBool> {
        self.resume_flag.clone()
    }

    fn try_reconnect(&mut self) -> Result<()> {
        let dual_mode = self.ports.len() > 1;
        let brightness = self.brightness.clone();
        let resume_flag = self.resume_flag.clone();
        let new_matrix = Self::new_with_brightness(brightness, dual_mode, self.height)?;
        *self = new_matrix;
        self.resume_flag = resume_flag;
        Ok(())
    }

    #[inline]
    pub fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.brightness.store(brightness, Ordering::SeqCst);
        for (idx, port) in self.ports.iter_mut().enumerate() {
            let buf = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, brightness];
            port
                .port
                .write_all(&buf)
                .map_err(|e| anyhow!("Failed to set brightness on port {}: {}", idx, e))?;
        }
        Ok(())
    }

    #[inline]
    pub fn render(&mut self, game_state: &GameState) -> Result<()> {
        if self.resume_flag.swap(false, Ordering::Acquire) {
            eprintln!("Resume detected, triggering reconnect...");
            self.state = MatrixState::Suspended;
        }

        match self.state {
            MatrixState::Active => {
                self.frame_count += 1;

                if self.frame_count % HEARTBEAT_INTERVAL == 0 {
                    let healthy = self.ports.first()
                        .map_or(false, |p| p.port.bytes_to_read().is_ok());
                    if !healthy {
                        eprintln!("Health check failed, suspending...");
                        self.state = MatrixState::Suspended;
                        return Err(anyhow!("Port health check failed"));
                    }
                }

                match self.render_internal(game_state) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        eprintln!("Render error: {}", e);
                        self.state = MatrixState::Suspended;
                        Err(e)
                    }
                }
            }
            MatrixState::Suspended | MatrixState::Recovering => {
                let now = Instant::now();
                if now.duration_since(self.last_recovery_attempt)
                    < Duration::from_millis(RECOVERY_INTERVAL_MS)
                {
                    thread::sleep(Duration::from_millis(100));
                    return Ok(());
                }
                self.last_recovery_attempt = now;
                self.state = MatrixState::Recovering;

                eprintln!("Attempting to reconnect to LED Matrix...");
                match self.try_reconnect() {
                    Ok(()) => {
                        self.state = MatrixState::Active;
                        self.frame_count = 0;
                        println!("Successfully reconnected to LED Matrix");
                        let brightness = self.brightness.load(Ordering::Relaxed);
                        let _ = self.set_brightness(brightness);
                        self.render_internal(game_state)
                    }
                    Err(e) => {
                        self.state = MatrixState::Suspended;
                        eprintln!("Failed to reconnect: {}", e);
                        Err(e)
                    }
                }
            }
        }
    }

    #[inline]
    fn render_internal(&mut self, game_state: &GameState) -> Result<()> {
        let gw = game_state.width();
        let gh = game_state.height();

        let mut ball_mask = [false; 18 * 34];
        for ball in &game_state.balls {
            let bx = ball.x as usize;
            let by = ball.y as usize;
            if bx < gw && by < gh {
                ball_mask[bx * gh + by] = true;
            }
        }

        for port_index in 0..self.ports.len() {
            let port = &mut self.ports[port_index];

            port.bw_buffer[3..42].fill(0);
            for y in 0..self.height {
                if y >= gh {
                    break;
                }
                for local_x in 0..port.width {
                    let global_x = port_index * port.width + local_x;
                    if global_x >= gw {
                        break;
                    }

                    let idx = global_x * gh + y;
                    let square_color = game_state.squares[idx];
                    let has_ball = ball_mask[idx];

                    let on = match square_color {
                        SquareColor::Day => !has_ball,
                        SquareColor::Night => has_ball,
                    };

                    if on {
                        let i = local_x + MODULE_WIDTH * y;
                        let byte = 3 + i / 8;
                        let bit = i % 8;
                        port.bw_buffer[byte] |= 1u8 << bit;
                    }
                }
            }

            port
                .port
                .write_all(&port.bw_buffer)
                .map_err(|e| anyhow!("Failed to write BW frame on port {}: {}", port_index, e))?;
        }

        Ok(())
    }

    pub fn estimated_max_fps(&self) -> u32 {
        // Using DrawBW (0x06): 2 magic + 1 cmd + 39 payload = 42 bytes per port per frame
        let per_port = 2 + 1 + 39;
        let total = self.ports.len() * per_port;
        let bytes_per_sec = (BAUD_RATE as f64) / 10.0;
        let fps = (bytes_per_sec / ((total as f64) * 1.1)).floor() as u32;
        if fps < 1 { 1 } else { fps }
    }
}
