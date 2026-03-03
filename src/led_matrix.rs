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
/// Returns a vec of `(com_port_name, position)` where position is `"left"` or `"right"`.
/// With a single module, it is labelled `"right"` (the more common slot).
pub fn detect_modules() -> Vec<(String, String)> {
    let mut candidates: Vec<serialport::SerialPortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| matches!(p.port_type, serialport::SerialPortType::UsbPort(ref info) if info.vid == 0x32AC && (info.pid == 0x0020 || info.pid == 0x0021)))
        .collect();

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

    candidates.truncate(2);

    match candidates.len() {
        0 => vec![],
        1 => vec![(candidates[0].port_name.clone(), "right".to_string())],
        _ => vec![
            (candidates[0].port_name.clone(), "right".to_string()),
            (candidates[1].port_name.clone(), "left".to_string()),
        ],
    }
}

pub struct LedMatrix {
    ports: Vec<MatrixPort>,
    brightness: Arc<AtomicU8>,
    state: MatrixState,
    frame_count: u64,
    last_recovery_attempt: Instant,
    resume_flag: Arc<AtomicBool>,
    suspend_sync: Arc<SuspendSync>,
    preferred_module: String,
    width: usize,
    height: usize,
    fade_remaining: u16,
    reconnected_flag: bool,
}

impl LedMatrix {
    pub fn new_with_brightness(brightness: Arc<AtomicU8>, dual_mode: bool, preferred_module: &str, height: usize) -> Result<Self> {
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
        } else if candidates.len() >= 2 {
            // Two modules detected in single mode — pick based on preference
            candidates.truncate(2);
            let idx = if preferred_module == "left" { 1 } else { 0 };
            println!(
                "Single mode: selected {} module on {}",
                preferred_module, candidates[idx].port_name
            );
            vec![candidates.remove(idx)]
        } else {
            candidates.truncate(1);
            candidates
        };

        if dual_mode && desired_ports.len() == 2 {
            desired_ports.reverse();
            println!(
                "Auto-ordered modules: {} = left, {} = right",
                desired_ports[0].port_name, desired_ports[1].port_name
            );
        }

        // Phase 1: Open every port and immediately quench brightness so stale
        //          framebuffer content is hidden before any slow per-port init.
        let brightness_off = [MAGIC_WORD[0], MAGIC_WORD[1], CMD_BRIGHTNESS, 0];
        let mut raw_ports: Vec<(String, Box<dyn SerialPort>)> = Vec::new();
        for info in &desired_ports {
            match serialport::new(&info.port_name, BAUD_RATE)
                .timeout(Duration::from_millis(TIMEOUT_MS))
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .open()
            {
                Ok(port) => raw_ports.push((info.port_name.clone(), Box::from(port))),
                Err(e) => eprintln!("Failed opening port {}: {}", info.port_name, e),
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
            preferred_module: preferred_module.to_string(),
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
        let preferred = self.preferred_module.clone();
        // Drop old port handles so the OS can release them
        self.ports.clear();
        thread::sleep(Duration::from_millis(500));

        let brightness = self.brightness.clone();
        let resume_flag = self.resume_flag.clone();
        let suspend_sync = self.suspend_sync.clone();
        let new_matrix = Self::new_with_brightness(brightness, dual_mode, &preferred, self.height)?;
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
