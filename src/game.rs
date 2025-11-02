use rand::Rng;

pub const DEFAULT_GRID_HEIGHT: usize = 34;

const MIN_SPEED: f32 = 0.2;
const MAX_SPEED: f32 = 0.5;
const SPEED_RANDOMNESS: f32 = 0.001;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SquareColor {
    Day,
    Night,
}

#[derive(Clone, Copy)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub color_type: SquareColor,
}

impl Ball {
    #[inline]
    fn new(x: f32, y: f32, dx: f32, dy: f32, color_type: SquareColor) -> Self {
        Ball {
            x,
            y,
            dx,
            dy,
            color_type,
        }
    }
}

pub struct GameState {
    width: usize,
    height: usize,
    width_f32: f32,
    height_f32: f32,
    pub squares: Vec<Vec<SquareColor>>,
    pub balls: Vec<Ball>,
    pub day_score: usize,
    pub night_score: usize,
    pub rng: rand::rngs::ThreadRng,
}

impl GameState {
    pub fn new(width: usize, height: usize, balls_per_team: u8) -> Self {
        assert!(width > 0, "width must be positive");
        assert!(height > 0, "height must be positive");

        let half_height = height / 2;
        let width_f32 = width as f32;
        let height_f32 = height as f32;

        let mut squares = vec![vec![SquareColor::Day; height]; width];
        for column in squares.iter_mut() {
            for y in 0..half_height {
                column[y] = SquareColor::Night;
            }
        }

        let mut rng = rand::thread_rng();
        let base_speed = 0.3;
        
        let top_y = 2.0;
        let bottom_y = height_f32 - 2.0;

        let jitter = std::f32::consts::PI / 6.0;

        let n = balls_per_team.max(1) as usize;
        let mut balls = Vec::with_capacity(n * 2);

        for i in 0..n {
            let frac = (i as f32 + 1.0) / (n as f32 + 1.0);
            let x = 1.0 + frac * (width_f32 - 2.0);
            let xn = width_f32 - x;

            let day_angle = (top_y - bottom_y).atan2(xn - x) + rng.gen_range(-jitter..jitter);
            let night_angle = (bottom_y - top_y).atan2(x - xn) + rng.gen_range(-jitter..jitter);

            balls.push(Ball::new(
                x,
                bottom_y,
                base_speed * day_angle.cos(),
                base_speed * day_angle.sin(),
                SquareColor::Day,
            ));

            balls.push(Ball::new(
                xn,
                top_y,
                base_speed * night_angle.cos(),
                base_speed * night_angle.sin(),
                SquareColor::Night,
            ));
        }

        let day_score = half_height * width;
        let night_score = half_height * width;

        GameState {
            width,
            height,
            width_f32,
            height_f32,
            squares,
            balls,
            day_score,
            night_score,
            rng,
        }
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline]
    pub fn update(&mut self) {
        let mut new_squares = self.squares.clone();
        let mut day_score_delta = 0i32;
        let mut night_score_delta = 0i32;

        let original_balls = self.balls.clone();
        let mut updated_balls = original_balls.clone();

        for (index, ball) in original_balls.iter().enumerate() {
            let mut ball_state = *ball;

            if ball_state.x + ball_state.dx >= self.width_f32 - 0.5
                || ball_state.x + ball_state.dx < 0.5
            {
                let nx = if ball.x + ball.dx >= self.width_f32 - 0.5 {
                    -1.0
                } else {
                    1.0
                };
                let ny = 0.0;
                let dot_product = ball_state.dx * nx + ball_state.dy * ny;
                ball_state.dx = ball_state.dx - 2.0 * dot_product * nx;
                ball_state.dy = ball_state.dy - 2.0 * dot_product * ny;
            }

            if ball_state.y + ball_state.dy >= self.height_f32 - 0.5
                || ball_state.y + ball_state.dy < 0.5
            {
                let nx = 0.0;
                let ny = if ball_state.y + ball_state.dy >= self.height_f32 - 0.5 {
                    -1.0
                } else {
                    1.0
                };
                let dot_product = ball_state.dx * nx + ball_state.dy * ny;
                ball_state.dx = ball_state.dx - 2.0 * dot_product * nx;
                ball_state.dy = ball_state.dy - 2.0 * dot_product * ny;
            }

            const CHECK_OFFSETS: [(f32, f32); 4] =
                [(0.5, 0.0), (-0.5, 0.0), (0.0, 0.5), (0.0, -0.5)];

            for &(offset_x, offset_y) in &CHECK_OFFSETS {
                let check_x = ball_state.x + offset_x;
                let check_y = ball_state.y + offset_y;

                if check_x < 0.0 || check_y < 0.0 {
                    continue;
                }

                let grid_x = check_x as usize;
                let grid_y = check_y as usize;

                if grid_x < self.width && grid_y < self.height {
                    if self.squares[grid_x][grid_y] != ball_state.color_type {
                        new_squares[grid_x][grid_y] = ball_state.color_type;

                        match ball_state.color_type {
                            SquareColor::Day => {
                                day_score_delta += 1;
                                night_score_delta -= 1;
                            }
                            SquareColor::Night => {
                                night_score_delta += 1;
                                day_score_delta -= 1;
                            }
                        }

                        let (nx, ny) = if offset_x.abs() > offset_y.abs() {
                            if offset_x > 0.0 {
                                (-1.0, 0.0)
                            } else {
                                (1.0, 0.0)
                            }
                        } else if offset_y > 0.0 {
                            (0.0, -1.0)
                        } else {
                            (0.0, 1.0)
                        };

                        let dot_product = ball_state.dx * nx + ball_state.dy * ny;
                        ball_state.dx = ball_state.dx - 2.0 * dot_product * nx;
                        ball_state.dy = ball_state.dy - 2.0 * dot_product * ny;

                        let angle_randomness = self.rng.gen_range(-0.1..0.1);
                        let speed = (ball_state.dx * ball_state.dx + ball_state.dy * ball_state.dy).sqrt();
                        let angle = ball_state.dy.atan2(ball_state.dx) + angle_randomness;
                        ball_state.dx = speed * angle.cos();
                        ball_state.dy = speed * angle.sin();
                    }
                }
            }

            ball_state.x += ball_state.dx;
            ball_state.y += ball_state.dy;

            ball_state.dx += self.rng.gen_range(-SPEED_RANDOMNESS..SPEED_RANDOMNESS);
            ball_state.dy += self.rng.gen_range(-SPEED_RANDOMNESS..SPEED_RANDOMNESS);

            ball_state.dx = ball_state.dx.clamp(-MAX_SPEED, MAX_SPEED);
            ball_state.dy = ball_state.dy.clamp(-MAX_SPEED, MAX_SPEED);

            if ball_state.dx.abs() < MIN_SPEED {
                ball_state.dx = if ball_state.dx > 0.0 {
                    MIN_SPEED
                } else {
                    -MIN_SPEED
                };
            }
            if ball_state.dy.abs() < MIN_SPEED {
                ball_state.dy = if ball_state.dy > 0.0 {
                    MIN_SPEED
                } else {
                    -MIN_SPEED
                };
            }

            updated_balls[index] = ball_state;
        }

        self.balls = updated_balls;
        self.squares = new_squares;

        self.day_score = (self.day_score as i32 + day_score_delta).max(0) as usize;
        self.night_score = (self.night_score as i32 + night_score_delta).max(0) as usize;
    }
}
