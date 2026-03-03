use anyhow::{anyhow, Result};
use serialport::{DataBits, Parity, SerialPort, StopBits};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::game::{GameState, SquareColor};
use crate::power::SuspendSync;

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
const FADE_FRAMES: u16 = 24; // Frames to fade brightness in (~0.5s at 48fps)

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
    fn new(mut port: Box<dyn SerialPort>, _height: usize) -> Result<Self> {
        if let Err(e) = port.clear(serialport::ClearBuffer::All) {
            return Err(anyhow!("Failed clearing port: {}", e));
        }

        // Immediately hide any stale display content from a previous session
        let brightness_off = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, 0];
        let _ = port.write_all(&brightness_off);
        let mut blank_frame = [0u8; 42];
        blank_frame[0] = MAGIC_WORD[0];
        blank_frame[1] = MAGIC_WORD[1];
        blank_frame[2] = CMD_DRAW_BW;
        let _ = port.write_all(&blank_frame);
        let _ = port.flush();

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

/// Enumerate connected Framework LED Matrix modules without opening ports.
/// Returns a vec of `(com_port_name, usb_serial_number)` pairs.
pub fn detect_modules() -> Vec<(String, String)> {
    let candidates: Vec<serialport::SerialPortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| matches!(p.port_type, serialport::SerialPortType::UsbPort(ref info) if info.vid == 0x32AC && (info.pid == 0x0020 || info.pid == 0x0021)))
        .collect();

    candidates.into_iter().map(|p| {
        let sn = match &p.port_type {
            serialport::SerialPortType::UsbPort(info) => {
                info.serial_number.clone().unwrap_or_default()
            }
            _ => String::new(),
        };
        (p.port_name, sn)
    }).collect()
}

/// Find the COM port name for a module with the given USB serial number.
fn find_port_for_serial(serial: &str) -> Option<String> {
    detect_modules().into_iter()
        .find(|(_, sn)| sn == serial)
        .map(|(port, _)| port)
}

/// Open a single module's serial port by its USB serial number.
/// Returns the opened port handle, or an error.
pub fn open_port_by_serial(serial: &str) -> Result<Box<dyn SerialPort>> {
    let port_name = find_port_for_serial(serial)
        .ok_or_else(|| anyhow!("Module with serial {} not found", serial))?;
    let port = serialport::new(&port_name, BAUD_RATE)
        .timeout(Duration::from_millis(TIMEOUT_MS))
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .open()
        .map_err(|e| anyhow!("Failed opening {}: {}", port_name, e))?;
    Ok(Box::from(port))
}

/// Light up all LEDs on a module at the given brightness (0-255).
pub fn flash_module(port: &mut Box<dyn SerialPort>, brightness: u8) -> Result<()> {
    let bright_cmd = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, brightness];
    let mut full_frame = [0xFFu8; 42];
    full_frame[0] = MAGIC_WORD[0];
    full_frame[1] = MAGIC_WORD[1];
    full_frame[2] = CMD_DRAW_BW;
    // bytes 3..42 are already 0xFF (all LEDs on)
    port.write_all(&bright_cmd)?;
    port.write_all(&full_frame)?;
    port.flush()?;
    Ok(())
}

/// Turn off all LEDs on a module.
pub fn blank_module(port: &mut Box<dyn SerialPort>) -> Result<()> {
    let bright_off = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, 0];
    let mut blank_frame = [0u8; 42];
    blank_frame[0] = MAGIC_WORD[0];
    blank_frame[1] = MAGIC_WORD[1];
    blank_frame[2] = CMD_DRAW_BW;
    port.write_all(&bright_off)?;
    port.write_all(&blank_frame)?;
    port.flush()?;
    Ok(())
}

pub struct LedMatrix {
    ports: Vec<MatrixPort>,
    brightness: Arc<AtomicU8>,
    state: MatrixState,
    frame_count: u64,
    last_recovery_attempt: Instant,
    resume_flag: Arc<AtomicBool>,
    suspend_sync: Arc<SuspendSync>,
    left_serial: String,
    right_serial: String,
    preferred_side: String,
    width: usize,
    height: usize,
    fade_remaining: u16,
    reconnected_flag: bool,
}

impl LedMatrix {
    pub fn new_with_brightness(
        brightness: Arc<AtomicU8>,
        dual_mode: bool,
        left_serial: &str,
        right_serial: &str,
        preferred_side: &str,
        height: usize,
    ) -> Result<Self> {
        let detected = detect_modules();
        if detected.is_empty() {
            return Err(anyhow!("No Framework LED Matrix modules found."));
        }

        // Build the ordered list of port names to open.
        // In dual mode: [left_port, right_port]  (port index 0 = left columns, 1 = right).
        // In single mode: [chosen_port].
        let port_names: Vec<String> = if dual_mode {
            let left_port = detected.iter()
                .find(|(_, sn)| sn == left_serial)
                .map(|(p, _)| p.clone());
            let right_port = detected.iter()
                .find(|(_, sn)| sn == right_serial)
                .map(|(p, _)| p.clone());
            match (left_port, right_port) {
                (Some(l), Some(r)) => {
                    println!("Dual mode: left={} right={}", l, r);
                    vec![l, r]
                }
                _ if detected.len() >= 2 => {
                    // Fallback: serials not calibrated, use first two detected modules
                    println!(
                        "Dual mode (uncalibrated): using {} and {}",
                        detected[0].0, detected[1].0
                    );
                    vec![detected[0].0.clone(), detected[1].0.clone()]
                }
                _ => {
                    return Err(anyhow!(
                        "Dual mode requested but fewer than 2 modules detected."
                    ));
                }
            }
        } else {
            // Single mode: pick by preferred side
            let target_serial = if preferred_side == "left" { left_serial } else { right_serial };
            if let Some((port, _)) = detected.iter().find(|(_, sn)| sn == target_serial) {
                println!("Single mode: using {} side on {}", preferred_side, port);
                vec![port.clone()]
            } else if let Some((port, sn)) = detected.first() {
                // Fallback: use whatever is available
                println!("Single mode: preferred module not found, falling back to {} ({})", port, sn);
                vec![port.clone()]
            } else {
                return Err(anyhow!("No Framework LED Matrix modules found."));
            }
        };

        // Phase 1: Open every port and immediately quench brightness so stale
        //          framebuffer content is hidden before any slow per-port init.
        let brightness_off = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, 0];
        let mut raw_ports: Vec<(String, Box<dyn SerialPort>)> = Vec::new();
        for name in &port_names {
            match serialport::new(name, BAUD_RATE)
                .timeout(Duration::from_millis(TIMEOUT_MS))
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .open()
            {
                Ok(port) => raw_ports.push((name.clone(), Box::from(port))),
                Err(e) => eprintln!("Failed opening port {}: {}", name, e),
            }
        }
        for (_, port) in &mut raw_ports {
            let _ = port.write_all(&brightness_off);
            let _ = port.flush();
        }

        // Phase 2: Full per-port initialisation (clear, blank frame, settle).
        let mut matrix_ports: Vec<MatrixPort> = Vec::new();
        for (name, port) in raw_ports {
            match MatrixPort::new(port, height) {
                Ok(matrix_port) => {
                    println!("Connected LED Matrix on {}", name);
                    matrix_ports.push(matrix_port);
                }
                Err(e) => eprintln!("Failed initializing port {}: {}", name, e),
            }
        }

        if matrix_ports.is_empty() {
            return Err(anyhow!("Unable to open any Framework LED Matrix modules."));
        }

        // Stabilize ports: retry blank frame writes until the hardware is responsive
        let blank_frame = {
            let mut buf = [0u8; 42];
            buf[0] = MAGIC_WORD[0];
            buf[1] = MAGIC_WORD[1];
            buf[2] = CMD_DRAW_BW;
            buf
        };
        for port in &mut matrix_ports {
            for attempt in 0..10 {
                match port.port.write_all(&blank_frame) {
                    Ok(()) => {
                        let _ = port.port.flush();
                        break;
                    }
                    Err(_) if attempt < 9 => {
                        let _ = port.port.clear(serialport::ClearBuffer::All);
                        thread::sleep(Duration::from_millis(200));
                    }
                    Err(e) => {
                        eprintln!("Warning: port stabilization failed: {}", e);
                    }
                }
            }
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
            suspend_sync: SuspendSync::new(),
            left_serial: left_serial.to_string(),
            right_serial: right_serial.to_string(),
            preferred_side: preferred_side.to_string(),
            fade_remaining: FADE_FRAMES,
            reconnected_flag: false,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn resume_flag(&self) -> Arc<AtomicBool> {
        self.resume_flag.clone()
    }

    pub fn suspend_sync(&self) -> Arc<SuspendSync> {
        self.suspend_sync.clone()
    }

    pub fn just_reconnected(&mut self) -> bool {
        let flag = self.reconnected_flag;
        self.reconnected_flag = false;
        flag
    }

    fn blank_all(&mut self) {
        let brightness_off = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, 0];
        let mut blank_frame = [0u8; 42];
        blank_frame[0] = MAGIC_WORD[0];
        blank_frame[1] = MAGIC_WORD[1];
        blank_frame[2] = CMD_DRAW_BW;

        for port in &mut self.ports {
            let _ = port.port.write_all(&brightness_off);
            let _ = port.port.write_all(&blank_frame);
            let _ = port.port.flush();
        }
    }

    fn try_reconnect(&mut self) -> Result<()> {
        let dual_mode = self.width > MODULE_WIDTH;
        let left_serial = self.left_serial.clone();
        let right_serial = self.right_serial.clone();
        let preferred_side = self.preferred_side.clone();
        // Drop old port handles so the OS can release them
        self.ports.clear();
        thread::sleep(Duration::from_millis(500));

        let brightness = self.brightness.clone();
        let resume_flag = self.resume_flag.clone();
        let suspend_sync = self.suspend_sync.clone();
        let new_matrix = Self::new_with_brightness(
            brightness, dual_mode, &left_serial, &right_serial, &preferred_side, self.height
        )?;
        *self = new_matrix;
        self.resume_flag = resume_flag;
        self.suspend_sync = suspend_sync;
        Ok(())
    }

    #[inline]
    pub fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        self.brightness.store(brightness, Ordering::SeqCst);
        self.send_brightness_hw(brightness)
    }

    #[inline]
    fn send_brightness_hw(&mut self, brightness: u8) -> Result<()> {
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
        if self.suspend_sync.requested.load(Ordering::Acquire) {
            eprintln!("Suspend detected, blanking display...");
            self.blank_all();
            self.suspend_sync.acked.store(true, Ordering::Release);
            self.state = MatrixState::Suspended;
            self.ports.clear();
            return Ok(());
        }

        if self.resume_flag.swap(false, Ordering::Acquire) {
            eprintln!("Resume detected, triggering reconnect...");
            self.state = MatrixState::Suspended;
            self.ports.clear();
        }

        match self.state {
            MatrixState::Active => {
                self.frame_count += 1;

                if self.fade_remaining > 0 {
                    self.fade_remaining -= 1;
                    let target = self.brightness.load(Ordering::Relaxed) as u16;
                    let progress = FADE_FRAMES - self.fade_remaining;
                    let interp = ((target * progress) / FADE_FRAMES) as u8;
                    let _ = self.send_brightness_hw(interp);
                }

                if self.frame_count % HEARTBEAT_INTERVAL == 0 {
                    let healthy = self.ports.first()
                        .map_or(false, |p| p.port.bytes_to_read().is_ok());
                    if !healthy {
                        eprintln!("Health check failed, suspending...");
                        self.state = MatrixState::Suspended;
                        self.ports.clear();
                        return Err(anyhow!("Port health check failed"));
                    }
                }

                match self.render_internal(game_state) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        eprintln!("Render error: {}", e);
                        self.state = MatrixState::Suspended;
                        self.ports.clear();
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
                        self.fade_remaining = FADE_FRAMES;
                        self.reconnected_flag = true;
                        println!("Successfully reconnected to LED Matrix");
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

impl Drop for LedMatrix {
    fn drop(&mut self) {
        self.blank_all();
        // Give the serial driver time to drain the blank commands to the
        // firmware before the port handles are closed.
        thread::sleep(Duration::from_millis(50));
    }
}
