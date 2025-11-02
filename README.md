   # FW16 Pong Wars

A Rust app that plays Pong Wars on the Framework Laptop 16 LED Matrix.

https://github.com/user-attachments/assets/2d7a4b85-f580-4dbc-9378-3473213b643f

## Requirements

- At least one [Framework Laptop 16 LED Matrix](https://frame.work/products/16-led-matrix)
- Optional second LED Matrix for dual-mode (-d, --dualmode)
- Rust toolchain (stable cargo + rustc)

## Example

```bash
cargo run -- --dualmode --speed 48 --balls 5 --brightness 10
```

Flags

- `-d`, `--dualmode`  Drive two modules side-by-side (18x34)
- `-b`, `--balls [1-20]`  Balls per team. Defaults to 2 if no number is provided.
- `-s`, `--speed <1-64>`  Target FPS (default 64)
- `-B`, `--brightness <0-100>`  Brightness percent (default 50)
- `--debug`  Extra timing/log output

## License

MIT License

## Acknowledgments

- Original Pong Wars by Koen van Gilst: https://github.com/vnglst/pong-wars
- Framework Computer for the LED Matrix hardware and open-source firmware
- Windsurf and OpenAI's GPT-5 enabling me to bring this idea to life  
