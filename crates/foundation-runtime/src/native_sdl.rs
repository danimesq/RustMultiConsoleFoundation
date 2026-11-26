//! Concrete native SDL3 host for Foundation game consumers.
//!
//! SDL owns the window, event queue, textures, WAV decoding and audio device here.
//! Consumers exchange only Foundation-owned IDs, input snapshots and draw/audio commands.

use crate::audio::{AudioMixer, SampleId, MIXER_CHANNELS, MIXER_RATE_HZ};
use crate::{
    AudioClipId, AudioRequest, Capabilities, CapabilityStatus, DrawCommand, FrameOutput,
    InputSnapshot, PadDirection, PhysicalKey, PlayerInputSnapshot, PlayerOneKeyMap, RgbaImage,
    RuntimeConfig, RuntimeError, RuntimeHost, TextureId,
};
use sdl3::audio::{
    AudioCallback, AudioFormat, AudioSpec, AudioSpecWAV, AudioStream, AudioStreamWithCallback,
};
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Scancode;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::{BlendMode, FRect, ScaleMode, Texture, TextureCreator, WindowCanvas};
use sdl3::surface::Surface;
use sdl3::video::WindowContext;
use sdl3::{AudioSubsystem, EventPump, Sdl, VideoSubsystem};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// SDL requests S16 values from this callback. The mixer preallocates its work buffer.
impl AudioCallback<i16> for AudioMixer {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let samples = self.render_callback_buffer(requested.max(0) as usize);
        let _ = stream.put_data_i16(samples);
    }
}

/// Native window/input/render/audio host using SDL3.
///
/// The field order is deliberate: textures are destroyed before their renderer, the
/// audio stream before its subsystem, and all SDL subsystems before the SDL context.
pub struct Sdl3Host {
    textures: HashMap<TextureId, Texture>,
    clips: HashMap<AudioClipId, Arc<[i16]>>,
    audio_stream: Option<AudioStreamWithCallback<AudioMixer>>,
    texture_creator: TextureCreator<WindowContext>,
    canvas: WindowCanvas,
    event_pump: EventPump,
    audio: Option<AudioSubsystem>,
    _video: VideoSubsystem,
    _sdl: Sdl,
    config: RuntimeConfig,
    started_at: Instant,
    held_keys: HashSet<Scancode>,
    pending_pressed: [u32; 2],
    pending_released: [u32; 2],
    pending_physical_key: Option<PhysicalKey>,
    p1_key_map: PlayerOneKeyMap,
    next_texture_id: u64,
    next_clip_id: u64,
    quit_requested: bool,
}

impl Sdl3Host {
    /// Open a native SDL3 window at 2x the requested logical resolution.
    ///
    /// Display/input initialization is required. Audio is optional: failure to open an
    /// audio device leaves the window and simulation usable, with audio capability
    /// reported as unavailable.
    pub fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        if config.width == 0 || config.height == 0 {
            return Err(RuntimeError::InvalidConfig("logical dimensions must be non-zero"));
        }
        let window_width = config.width.saturating_mul(2).max(config.width);
        let window_height = config.height.saturating_mul(2).max(config.height);

        let sdl = sdl3::init().map_err(host_error)?;
        let video = sdl.video().map_err(host_error)?;
        let mut builder = video.window(&config.title, window_width, window_height);
        builder.position_centered().resizable();
        let window = builder.build().map_err(host_error)?;
        let mut canvas = window.into_canvas();
        canvas
            .set_logical_size(
                config.width,
                config.height,
                sdl3::sys::render::SDL_LOGICAL_PRESENTATION_LETTERBOX,
            )
            .map_err(host_error)?;

        let texture_creator = canvas.texture_creator();
        let event_pump = sdl.event_pump().map_err(host_error)?;

        // A headless machine or a missing sound device must not prevent the game from opening.
        let audio = sdl.audio().ok();
        let audio_stream = audio.as_ref().and_then(|audio| {
            let spec = AudioSpec::new(
                Some(MIXER_RATE_HZ as i32),
                Some(MIXER_CHANNELS as i32),
                Some(AudioFormat::s16_sys()),
            );
            let mut stream = audio
                .open_playback_stream(&spec, AudioMixer::default())
                .ok()?;
            stream.resume().ok()?;
            Some(stream)
        });

        Ok(Self {
            textures: HashMap::new(),
            clips: HashMap::new(),
            audio_stream,
            texture_creator,
            canvas,
            event_pump,
            audio,
            _video: video,
            _sdl: sdl,
            config,
            started_at: Instant::now(),
            held_keys: HashSet::new(),
            pending_pressed: [0; 2],
            pending_released: [0; 2],
            pending_physical_key: None,
            p1_key_map: PlayerOneKeyMap::default(),
            next_texture_id: 1,
            next_clip_id: 1,
            quit_requested: false,
        })
    }

    /// Decode a BMP with SDL and return tightly packed RGBA8 data.
    ///
    /// The game owns game-specific transparency rules. For Street Chaves sprite sheets,
    /// the original white color key can be converted to alpha before texture registration.
    pub fn load_bmp(path: impl AsRef<Path>) -> Result<RgbaImage, RuntimeError> {
        let source = Surface::load_bmp(path).map_err(host_error)?;
        let surface = source
            .convert_format(PixelFormat::RGBA32)
            .map_err(host_error)?;
        let width = surface.width();
        let height = surface.height();
        let pitch = surface.pitch() as usize;
        let row_bytes = width as usize * 4;
        let mut pixels = vec![0_u8; row_bytes * height as usize];
        surface.with_lock(|source_pixels| {
            for y in 0..height as usize {
                let src_start = y * pitch;
                let dst_start = y * row_bytes;
                pixels[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&source_pixels[src_start..src_start + row_bytes]);
            }
        });
        RgbaImage::new(width, height, pixels)
    }

    /// Decode an uncompressed PNG using the SDL3 image function included in SDL3 itself.
    pub fn load_png(path: impl AsRef<Path>) -> Result<RgbaImage, RuntimeError> {
        let source = Surface::load_png(path).map_err(host_error)?;
        let surface = source
            .convert_format(PixelFormat::RGBA32)
            .map_err(host_error)?;
        let width = surface.width();
        let height = surface.height();
        let pitch = surface.pitch() as usize;
        let row_bytes = width as usize * 4;
        let mut pixels = vec![0_u8; row_bytes * height as usize];
        surface.with_lock(|source_pixels| {
            for y in 0..height as usize {
                let src_start = y * pitch;
                let dst_start = y * row_bytes;
                pixels[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&source_pixels[src_start..src_start + row_bytes]);
            }
        });
        RgbaImage::new(width, height, pixels)
    }

    /// Upload one Foundation RGBA image and return a backend-neutral texture ID.
    pub fn register_texture(&mut self, image: &RgbaImage) -> Result<TextureId, RuntimeError> {
        let mut surface = Surface::new(image.width, image.height, PixelFormat::RGBA32)
            .map_err(host_error)?;
        let row_bytes = image.width as usize * 4;
        surface.with_lock_mut(|destination| {
            for y in 0..image.height as usize {
                let start = y * row_bytes;
                destination[start..start + row_bytes]
                    .copy_from_slice(&image.pixels[start..start + row_bytes]);
            }
        });

        let mut texture = self
            .texture_creator
            .create_texture_from_surface(&surface)
            .map_err(host_error)?;
        texture.set_scale_mode(if self.config.pixel_art {
            ScaleMode::Nearest
        } else {
            ScaleMode::Linear
        });
        texture.set_blend_mode(BlendMode::Blend);

        let id = TextureId(self.next_texture_id);
        self.next_texture_id = self.next_texture_id.saturating_add(1);
        self.textures.insert(id, texture);
        Ok(id)
    }

    /// Decode a WAV to the Foundation's canonical 44.1 kHz S16 stereo mixer format.
    /// Volume follows the original Chaves convention: integer percent, clamped to 0..400.
    pub fn load_wav(
        &mut self,
        path: impl AsRef<Path>,
        volume_percent: i32,
    ) -> Result<AudioClipId, RuntimeError> {
        let audio = self
            .audio
            .as_ref()
            .ok_or(RuntimeError::Unsupported("audio output is unavailable"))?;
        let wav = AudioSpecWAV::load_wav(path).map_err(host_error)?;
        let source_spec = AudioSpec::new(
            Some(wav.freq),
            Some(i32::from(wav.channels)),
            Some(wav.format),
        );
        let destination_spec = AudioSpec::new(
            Some(MIXER_RATE_HZ as i32),
            Some(MIXER_CHANNELS as i32),
            Some(AudioFormat::s16_sys()),
        );
        let mut converter = audio
            .new_stream(Some(&source_spec), Some(&destination_spec))
            .map_err(host_error)?;
        converter.put_data(wav.buffer()).map_err(host_error)?;
        converter.flush().map_err(host_error)?;

        let available = converter.available_bytes().map_err(host_error)?.max(0) as usize;
        let mut pcm = vec![0_i16; available / std::mem::size_of::<i16>()];
        let samples_read = converter
            .read_i16_samples(&mut pcm)
            .map_err(|error| RuntimeError::Host(error.to_string()))?;
        pcm.truncate(samples_read - (samples_read % MIXER_CHANNELS));

        let volume = volume_percent.clamp(0, 400);
        if volume != 100 {
            for sample in &mut pcm {
                let scaled = (i32::from(*sample) * volume) / 100;
                *sample = scaled.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            }
        }

        let id = AudioClipId(self.next_clip_id);
        self.next_clip_id = self.next_clip_id.saturating_add(1);
        self.clips.insert(id, Arc::from(pcm));
        Ok(id)
    }

    /// Resolve an SDL physical key to the platform-neutral physical-key vocabulary.
    fn physical_key(scancode: Scancode) -> Option<PhysicalKey> {
        use Scancode::*;
        Some(match scancode {
            A => PhysicalKey::A, B => PhysicalKey::B, C => PhysicalKey::C, D => PhysicalKey::D,
            E => PhysicalKey::E, F => PhysicalKey::F, G => PhysicalKey::G, H => PhysicalKey::H,
            I => PhysicalKey::I, J => PhysicalKey::J, K => PhysicalKey::K, L => PhysicalKey::L,
            M => PhysicalKey::M, N => PhysicalKey::N, O => PhysicalKey::O, P => PhysicalKey::P,
            Q => PhysicalKey::Q, R => PhysicalKey::R, S => PhysicalKey::S, T => PhysicalKey::T,
            U => PhysicalKey::U, V => PhysicalKey::V, W => PhysicalKey::W, X => PhysicalKey::X,
            Y => PhysicalKey::Y, Z => PhysicalKey::Z,
            _0 => PhysicalKey::Digit0, _1 => PhysicalKey::Digit1,
            _2 => PhysicalKey::Digit2, _3 => PhysicalKey::Digit3,
            _4 => PhysicalKey::Digit4, _5 => PhysicalKey::Digit5,
            _6 => PhysicalKey::Digit6, _7 => PhysicalKey::Digit7,
            _8 => PhysicalKey::Digit8, _9 => PhysicalKey::Digit9,
            Up => PhysicalKey::ArrowUp, Down => PhysicalKey::ArrowDown,
            Left => PhysicalKey::ArrowLeft, Right => PhysicalKey::ArrowRight,
            Kp0 => PhysicalKey::Numpad0, Kp1 => PhysicalKey::Numpad1,
            Kp2 => PhysicalKey::Numpad2, Kp3 => PhysicalKey::Numpad3,
            Kp4 => PhysicalKey::Numpad4, Kp5 => PhysicalKey::Numpad5,
            Kp6 => PhysicalKey::Numpad6, Kp7 => PhysicalKey::Numpad7,
            Kp8 => PhysicalKey::Numpad8, Kp9 => PhysicalKey::Numpad9,
            F1 => PhysicalKey::F1, F2 => PhysicalKey::F2, F3 => PhysicalKey::F3,
            F4 => PhysicalKey::F4, F5 => PhysicalKey::F5, F6 => PhysicalKey::F6,
            F7 => PhysicalKey::F7, F8 => PhysicalKey::F8, F9 => PhysicalKey::F9,
            F10 => PhysicalKey::F10, F11 => PhysicalKey::F11, F12 => PhysicalKey::F12,
            _ => return None,
        })
    }

    fn key_binding(&self, scancode: Scancode) -> Option<(usize, u8)> {
        let key = Self::physical_key(scancode)?;
        if let Some(action) = self.p1_key_map.action_index(key) {
            if action >= 4 {
                return Some((0, (action - 4) as u8));
            }
        }
        use Scancode::*;
        match scancode {
            F1 => Some((0, 8)), F2 => Some((0, 9)), F3 => Some((0, 10)),
            F4 => Some((0, 11)), F5 => Some((0, 12)),
            R => Some((1, 0)), T => Some((1, 1)), Y => Some((1, 2)), U => Some((1, 3)),
            Kp1 => Some((1, 8)), Kp2 => Some((1, 9)), Kp3 => Some((1, 10)),
            Kp4 => Some((1, 11)), Kp5 => Some((1, 12)),
            _ => None,
        }
    }

    fn held_mask(&self, player: usize) -> u32 {
        self.held_keys
            .iter()
            .filter_map(|key| self.key_binding(*key))
            .filter(|(owner, _)| *owner == player)
            .fold(0_u32, |mask, (_, slot)| mask | (1_u32 << slot))
    }

    fn direction_for(&self, player: usize) -> PadDirection {
        let is_down = |key: PhysicalKey| {
            self.held_keys.iter().any(|scancode| Self::physical_key(*scancode) == Some(key))
        };
        let (up, down, left, right) = if player == 0 {
            (
                is_down(self.p1_key_map.keys[0]), is_down(self.p1_key_map.keys[1]),
                is_down(self.p1_key_map.keys[2]), is_down(self.p1_key_map.keys[3]),
            )
        } else {
            (
                self.held_keys.contains(&Scancode::W), self.held_keys.contains(&Scancode::S),
                self.held_keys.contains(&Scancode::A), self.held_keys.contains(&Scancode::D),
            )
        };
        match (up, down, left, right) {
            (true, false, true, false) => PadDirection::UpLeft,
            (true, false, false, true) => PadDirection::UpRight,
            (false, true, true, false) => PadDirection::DownLeft,
            (false, true, false, true) => PadDirection::DownRight,
            (true, false, false, false) => PadDirection::Up,
            (false, true, false, false) => PadDirection::Down,
            (false, false, true, false) => PadDirection::Left,
            (false, false, false, true) => PadDirection::Right,
            _ => PadDirection::Neutral,
        }
    }

    fn apply_key_down(&mut self, scancode: Scancode, repeat: bool) {
        if self.held_keys.insert(scancode) && !repeat {
            self.pending_physical_key = Self::physical_key(scancode);
            if let Some((player, slot)) = self.key_binding(scancode) {
                self.pending_pressed[player] |= 1_u32 << slot;
            }
        }
    }

    fn apply_key_up(&mut self, scancode: Scancode) {
        if self.held_keys.remove(&scancode) {
            if let Some((player, slot)) = self.key_binding(scancode) {
                self.pending_released[player] |= 1_u32 << slot;
            }
        }
    }

    fn reset_input(&mut self) {
        // End the mutable drain borrow before resolving keys through the current map.
        let held: Vec<_> = self.held_keys.drain().collect();
        for key in held {
            if let Some((player, slot)) = self.key_binding(key) {
                self.pending_released[player] |= 1_u32 << slot;
            }
        }
    }

    fn play_clip(&mut self, clip: AudioClipId, looped: bool) -> Result<(), RuntimeError> {
        let samples = self
            .clips
            .get(&clip)
            .cloned()
            .ok_or_else(|| RuntimeError::Host(format!("audio clip {} is not loaded", clip.0)))?;
        let Some(stream) = self.audio_stream.as_mut() else {
            return Err(RuntimeError::Unsupported("audio output is unavailable"));
        };
        let Some(mut callback) = stream.lock() else {
            return Err(RuntimeError::Host("SDL audio callback lock failed".to_owned()));
        };
        if !callback.play(SampleId(clip.0), samples, looped) {
            return Err(RuntimeError::Host("SDL audio mixer rejected an empty clip".to_owned()));
        }
        Ok(())
    }

    fn stop_clip(&mut self, clip: AudioClipId) -> Result<(), RuntimeError> {
        let Some(stream) = self.audio_stream.as_mut() else {
            return Err(RuntimeError::Unsupported("audio output is unavailable"));
        };
        let Some(mut callback) = stream.lock() else {
            return Err(RuntimeError::Host("SDL audio callback lock failed".to_owned()));
        };
        callback.stop(SampleId(clip.0));
        Ok(())
    }

    fn play_beep(&mut self, frequency_hz: u32, duration_ms: u32) -> Result<(), RuntimeError> {
        let frequency = frequency_hz.clamp(40, 12_000) as f64;
        let frames = ((MIXER_RATE_HZ as u64 * duration_ms.min(5_000) as u64) / 1_000)
            .max(1) as usize;
        let mut pcm = Vec::with_capacity(frames * MIXER_CHANNELS);
        for frame in 0..frames {
            let phase = std::f64::consts::TAU * frequency * frame as f64 / MIXER_RATE_HZ as f64;
            // Keep the diagnostic beep quiet enough to avoid clipping on mixed playback.
            let sample = (phase.sin() * 7_000.0) as i16;
            pcm.extend_from_slice(&[sample, sample]);
        }
        let id = AudioClipId(self.next_clip_id);
        self.next_clip_id = self.next_clip_id.saturating_add(1);
        self.clips.insert(id, Arc::from(pcm));
        self.play_clip(id, false)
    }

    fn letterbox_viewport(&mut self) -> Result<(), RuntimeError> {
        self.canvas
            .set_logical_size(
                self.config.width,
                self.config.height,
                sdl3::sys::render::SDL_LOGICAL_PRESENTATION_LETTERBOX,
            )
            .map_err(host_error)
    }
}

impl RuntimeHost for Sdl3Host {
    fn set_player_one_key_map(&mut self, map: PlayerOneKeyMap) {
        self.reset_input();
        self.p1_key_map = map;
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            display: CapabilityStatus::Available,
            keyboard: CapabilityStatus::Available,
            pointer: CapabilityStatus::Unavailable,
            gamepad: CapabilityStatus::Unavailable,
            audio: if self.audio_stream.is_some() {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Unavailable
            },
            storage: CapabilityStatus::Unknown,
        }
    }

    fn now(&self) -> Duration {
        self.started_at.elapsed()
    }

    fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError> {
        let events: Vec<_> = self.event_pump.poll_iter().collect();
        for event in events {
            match event {
                Event::Quit { .. } => self.quit_requested = true,
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat,
                    ..
                } => {
                    if scancode == Scancode::Escape && !repeat {
                        self.quit_requested = true;
                    } else {
                        self.apply_key_down(scancode, repeat);
                    }
                }
                Event::KeyUp {
                    scancode: Some(scancode),
                    ..
                } => self.apply_key_up(scancode),
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => self.reset_input(),
                Event::Window {
                    win_event: WindowEvent::CloseRequested,
                    ..
                } => self.quit_requested = true,
                _ => {}
            }
        }

        let players = std::array::from_fn(|player| PlayerInputSnapshot {
            direction: self.direction_for(player),
            held_buttons: self.held_mask(player),
            pressed_buttons: std::mem::take(&mut self.pending_pressed[player]),
            released_buttons: std::mem::take(&mut self.pending_released[player]),
        });
        let physical_key_pressed = self.pending_physical_key.take();
        let p1 = players[0];
        let direction = p1.direction;
        Ok(InputSnapshot {
            up: crate::ButtonState {
                pressed: false,
                held: matches!(direction, PadDirection::Up | PadDirection::UpLeft | PadDirection::UpRight),
                released: false,
            },
            down: crate::ButtonState {
                pressed: false,
                held: matches!(direction, PadDirection::Down | PadDirection::DownLeft | PadDirection::DownRight),
                released: false,
            },
            left: crate::ButtonState {
                pressed: false,
                held: matches!(direction, PadDirection::Left | PadDirection::UpLeft | PadDirection::DownLeft),
                released: false,
            },
            right: crate::ButtonState {
                pressed: false,
                held: matches!(direction, PadDirection::Right | PadDirection::UpRight | PadDirection::DownRight),
                released: false,
            },
            confirm: crate::ButtonState {
                pressed: p1.pressed_buttons & 1 != 0,
                held: p1.held_buttons & 1 != 0,
                released: p1.released_buttons & 1 != 0,
            },
            cancel: crate::ButtonState {
                pressed: p1.pressed_buttons & (1 << 1) != 0,
                held: p1.held_buttons & (1 << 1) != 0,
                released: p1.released_buttons & (1 << 1) != 0,
            },
            quit_requested: self.quit_requested,
            physical_key_pressed,
            players,
            ..Default::default()
        })
    }

    fn present(&mut self, output: &FrameOutput) -> Result<(), RuntimeError> {
        self.letterbox_viewport()?;
        for command in &output.draw_commands {
            match command {
                DrawCommand::Clear { rgba } => {
                    self.canvas.set_draw_color(Color::RGBA(rgba[0], rgba[1], rgba[2], rgba[3]));
                    self.canvas.clear();
                }
                DrawCommand::Rect { rect, rgba } => {
                    self.canvas.set_draw_color(Color::RGBA(rgba[0], rgba[1], rgba[2], rgba[3]));
                    self.canvas
                        .fill_rect(Some(FRect::new(
                            rect.x as f32,
                            rect.y as f32,
                            rect.width as f32,
                            rect.height as f32,
                        )))
                        .map_err(host_error)?;
                }
                DrawCommand::Texture {
                    texture,
                    source,
                    destination,
                    flip_horizontal,
                } => {
                    let texture = self.textures.get(texture).ok_or_else(|| {
                        RuntimeError::Host(format!("texture {} is not registered", texture.0))
                    })?;
                    let source = source.map(|rect| {
                        FRect::new(
                            rect.x as f32,
                            rect.y as f32,
                            rect.width as f32,
                            rect.height as f32,
                        )
                    });
                    let destination = FRect::new(
                        destination.x as f32,
                        destination.y as f32,
                        destination.width as f32,
                        destination.height as f32,
                    );
                    self.canvas
                        .copy_ex(texture, source, destination, 0.0, None, *flip_horizontal, false)
                        .map_err(host_error)?;
                }
            }
        }
        if !self.canvas.present() {
            return Err(RuntimeError::Host("SDL failed to present the window backbuffer".to_owned()));
        }
        Ok(())
    }

    fn audio(&mut self, request: &AudioRequest) -> Result<(), RuntimeError> {
        match request {
            AudioRequest::Beep {
                frequency_hz,
                duration_ms,
            } => self.play_beep(*frequency_hz, *duration_ms),
            AudioRequest::PlayClip { clip, looped } => self.play_clip(*clip, *looped),
            AudioRequest::StopClip { clip } => self.stop_clip(*clip),
            AudioRequest::StopAll => {
                let Some(stream) = self.audio_stream.as_mut() else {
                    return Err(RuntimeError::Unsupported("audio output is unavailable"));
                };
                let Some(mut callback) = stream.lock() else {
                    return Err(RuntimeError::Host("SDL audio callback lock failed".to_owned()));
                };
                for clip_id in self.clips.keys().copied() {
                    callback.stop(SampleId(clip_id.0));
                }
                Ok(())
            }
        }
    }
}

impl crate::TextureRegistrar for Sdl3Host {
    fn register_texture(&mut self, image: &RgbaImage) -> Result<TextureId, RuntimeError> {
        Sdl3Host::register_texture(self, image)
    }
}

fn host_error(error: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Host(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_defaults_preserve_the_pascal_physical_layout() {
        let host_map = PlayerOneKeyMap::default();
        assert_eq!(Sdl3Host::physical_key(Scancode::Up), Some(PhysicalKey::ArrowUp));
        assert_eq!(host_map.action_index(PhysicalKey::ArrowUp), Some(0));
        assert_eq!(host_map.action_index(PhysicalKey::Z), Some(4));
        assert_eq!(host_map.action_index(PhysicalKey::X), Some(5));
        assert!(host_map.has_unique_keys());
    }

    #[test]
    fn key_edges_are_not_repeated_by_host_repeat_events() {
        // The key tracking used by the adapter emits each press edge only on first down.
        let mut keys = HashSet::new();
        assert!(keys.insert(Scancode::Z));
        assert!(!keys.insert(Scancode::Z));
        assert!(keys.remove(&Scancode::Z));
        assert!(!keys.remove(&Scancode::Z));
    }
}
