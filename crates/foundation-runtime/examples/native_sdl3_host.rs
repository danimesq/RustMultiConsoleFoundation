//! Small executable proving that consumers can use the Foundation SDL3 host directly.
//!
//! Run with:
//!   cargo run -p foundation-runtime --example native_sdl3_host --features native-sdl3
//! Press Z to play a tone, Escape to quit.

use foundation_runtime::native_sdl::Sdl3Host;
use foundation_runtime::{
    AudioRequest, DrawCommand, FrameOutput, Rect, RgbaImage, RuntimeConfig, RuntimeError,
    RuntimeHost,
};
use std::time::Duration;

fn main() -> Result<(), RuntimeError> {
    let mut host = Sdl3Host::new(RuntimeConfig {
        title: "RustMultiConsoleFoundation · SDL3 host".to_owned(),
        width: 400,
        height: 300,
        pixel_art: true,
    })?;

    let mut pixels = vec![0_u8; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let index = (y * 64 + x) * 4;
            let checker = (x / 8 + y / 8) % 2 == 0;
            let color = if checker {
                [240, 180, 60, 255]
            } else {
                [40, 90, 150, 255]
            };
            pixels[index..index + 4].copy_from_slice(&color);
        }
    }
    let image = RgbaImage::new(64, 64, pixels)?;
    let texture = host.register_texture(&image)?;

    loop {
        let input = host.poll_input()?;
        if input.quit_requested {
            break;
        }

        // Default player-one confirm key is Z, matching the first Chaves keyboard map.
        if input.confirm.pressed {
            host.audio(&AudioRequest::Beep {
                frequency_hz: 660,
                duration_ms: 90,
            })?;
        }

        host.present(&FrameOutput {
            draw_commands: vec![
                DrawCommand::Clear {
                    rgba: [18, 20, 30, 255],
                },
                DrawCommand::Rect {
                    rect: Rect::new(32, 32, 336, 236),
                    rgba: [55, 63, 79, 255],
                },
                DrawCommand::Texture {
                    texture,
                    source: None,
                    destination: Rect::new(168, 118, 64, 64),
                    flip_horizontal: false,
                },
            ],
            audio: Vec::new(),
        })?;

        std::thread::sleep(Duration::from_millis(8));
    }

    Ok(())
}
