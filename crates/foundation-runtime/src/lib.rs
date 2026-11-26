//! Platform-neutral runtime contracts for RustMultiConsoleFoundation.
//!
//! Game consumers depend on these types, not SDL, Orbital, browser, or console SDKs.
//! The optional native-sdl3 feature supplies one concrete desktop host.

use std::time::Duration;

/// Shared, backend-neutral PCM mixer. It has no windowing or device dependency.
pub mod audio;

/// Portable manifest builder and validation for WebAssembly exports.
pub mod web_export;

/// Native SDL3 window, keyboard, texture, WAV and audio-device host.
#[cfg(all(feature = "native-sdl3", not(target_arch = "wasm32")))]
pub mod native_sdl;

/// Browser Canvas 2D, keyboard, asset-manifest and WebAudio host.
///
/// This adapter targets `wasm32-unknown-unknown`; it does not compile SDL or WebGL.
#[cfg(all(feature = "web-canvas", target_arch = "wasm32"))]
pub mod web;

/// Requested logical game display configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Window/game title where a window exists.
    pub title: String,
    /// Logical width in pixels or target-specific logical units.
    pub width: u32,
    /// Logical height in pixels or target-specific logical units.
    pub height: u32,
    /// Prefer integer scaling for pixel-art presentation.
    pub pixel_art: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            title: "Foundation Game".into(),
            width: 320,
            height: 240,
            pixel_art: true,
        }
    }
}

/// Capability availability reported by a host adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityStatus {
    Available,
    Unavailable,
    Emulated,
    Degraded,
    Unknown,
}

/// Runtime feature support, queried instead of assuming target parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub display: CapabilityStatus,
    pub keyboard: CapabilityStatus,
    pub pointer: CapabilityStatus,
    pub gamepad: CapabilityStatus,
    pub audio: CapabilityStatus,
    pub storage: CapabilityStatus,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            display: CapabilityStatus::Unknown,
            keyboard: CapabilityStatus::Unknown,
            pointer: CapabilityStatus::Unknown,
            gamepad: CapabilityStatus::Unknown,
            audio: CapabilityStatus::Unknown,
            storage: CapabilityStatus::Unknown,
        }
    }
}

/// Normalized one-shot/held/released state, independent of physical key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonState {
    pub pressed: bool,
    pub held: bool,
    pub released: bool,
}

/// Eight-way player direction emitted by a platform adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadDirection {
    #[default]
    Neutral,
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

/// Normalized game-player input. Button slots 0..3 are the four ordinary attack buttons.
/// Slots 8..12 are reserved for custom-character special-move activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerInputSnapshot {
    pub direction: PadDirection,
    pub held_buttons: u32,
    pub pressed_buttons: u32,
    pub released_buttons: u32,
}

/// Input snapshot for one runtime frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputSnapshot {
    pub up: ButtonState,
    pub down: ButtonState,
    pub left: ButtonState,
    pub right: ButtonState,
    pub confirm: ButtonState,
    pub cancel: ButtonState,
    pub pointer_x: i32,
    pub pointer_y: i32,
    pub pointer_primary: ButtonState,
    pub quit_requested: bool,
    /// Two normalized player slots, independent of keyboard/controller layout.
    pub players: [PlayerInputSnapshot; 2],
}

/// Logical rectangle used by draw commands and texture source regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

/// Opaque texture handle allocated by a host. It is never an SDL/browser handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);

/// Decoded RGBA image data, suitable for upload to any renderer backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// Tightly packed row-major RGBA8 pixels.
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, RuntimeError> {
        if width == 0 || height == 0 {
            return Err(RuntimeError::InvalidConfig("image dimensions must be non-zero"));
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(RuntimeError::InvalidConfig("image dimensions overflow"))?;
        if pixels.len() != expected {
            return Err(RuntimeError::Host(format!(
                "RGBA image has {} bytes; expected {expected}",
                pixels.len()
            )));
        }
        Ok(Self { width, height, pixels })
    }
}

/// Renderer-neutral ordered drawing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawCommand {
    Clear { rgba: [u8; 4] },
    Rect { rect: Rect, rgba: [u8; 4] },
    Texture {
        texture: TextureId,
        source: Option<Rect>,
        destination: Rect,
        flip_horizontal: bool,
    },
}

/// Renderer-neutral output for one host frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameOutput {
    pub draw_commands: Vec<DrawCommand>,
    pub audio: Vec<AudioRequest>,
}

/// Opaque audio clip handle allocated by a host after asset decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioClipId(pub u64);

/// Semantic audio request. The consumer refers to clips by Foundation IDs only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioRequest {
    Beep { frequency_hz: u32, duration_ms: u32 },
    PlayClip { clip: AudioClipId, looped: bool },
    StopClip { clip: AudioClipId },
    StopAll,
}

/// Context passed to the consumer's per-frame update callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameContext {
    pub frame_number: u64,
    pub elapsed: Duration,
    pub delta: Duration,
    pub input: InputSnapshot,
}

/// Typed errors with actionable messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidConfig(&'static str),
    Host(String),
    Unsupported(&'static str),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(f, "invalid runtime configuration: {message}"),
            Self::Host(message) => write!(f, "runtime host error: {message}"),
            Self::Unsupported(feature) => write!(f, "runtime capability unsupported: {feature}"),
        }
    }
}
impl std::error::Error for RuntimeError {}

/// Adapter implemented by a concrete platform/backend.
///
/// It owns host integration and must not initialize a competing window, event loop, or input path.
pub trait RuntimeHost {
    fn capabilities(&self) -> Capabilities;
    /// Monotonic time from an adapter-defined origin.
    fn now(&self) -> Duration;
    /// Poll normalized input for this frame.
    fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError>;
    /// Present backend-neutral ordered draw commands.
    fn present(&mut self, output: &FrameOutput) -> Result<(), RuntimeError>;
    /// Dispatch a semantic audio request.
    fn audio(&mut self, request: &AudioRequest) -> Result<(), RuntimeError>;

    /// Release any held physical inputs after a host pause/visibility transition.
    ///
    /// Default no-op keeps existing consumer host implementations source-compatible.
    fn reset_input_after_pause(&mut self) {}
}

/// Resource registration contract shared by native and browser renderer hosts.
///
/// The returned ID is backend-neutral; the concrete host keeps its platform handle private.
pub trait TextureRegistrar {
    fn register_texture(&mut self, image: &RgbaImage) -> Result<TextureId, RuntimeError>;
}

/// Frame coordinator that consumers drive from their chosen host loop.
pub struct Runtime<H: RuntimeHost> {
    host: H,
    config: RuntimeConfig,
    frame_number: u64,
    started_at: Duration,
    last_frame_at: Duration,
}

impl<H: RuntimeHost> Runtime<H> {
    /// Create the coordinator around an already initialized adapter.
    pub fn new(host: H, config: RuntimeConfig) -> Result<Self, RuntimeError> {
        if config.width == 0 || config.height == 0 {
            return Err(RuntimeError::InvalidConfig("logical dimensions must be non-zero"));
        }
        let now = host.now();
        Ok(Self {
            host,
            config,
            frame_number: 0,
            started_at: now,
            last_frame_at: now,
        })
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn capabilities(&self) -> Capabilities {
        self.host.capabilities()
    }

    /// Access the concrete host for loading resources and diagnostics.
    pub fn host(&self) -> &H {
        &self.host
    }

    /// Mutably access the concrete host for resource loading or host-specific configuration.
    pub fn host_mut(&mut self) -> &mut H {
        &mut self.host
    }

    /// Poll input, run consumer update, dispatch audio and present one frame.
    ///
    /// The consumer owns game timing semantics. Deterministic games should use a fixed-step
    /// clock in their core and may call the host methods directly instead of this helper.
    pub fn step(
        &mut self,
        update: impl FnOnce(&FrameContext) -> FrameOutput,
    ) -> Result<FrameContext, RuntimeError> {
        let now = self.host.now();
        let input = self.host.poll_input()?;
        let context = FrameContext {
            frame_number: self.frame_number,
            elapsed: now.saturating_sub(self.started_at),
            delta: now.saturating_sub(self.last_frame_at),
            input,
        };
        let output = update(&context);
        for request in &output.audio {
            self.host.audio(request)?;
        }
        self.host.present(&output)?;
        self.frame_number = self.frame_number.saturating_add(1);
        self.last_frame_at = now;
        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockHost {
        time: Duration,
        presentations: usize,
        audio_requests: Vec<AudioRequest>,
    }

    impl RuntimeHost for MockHost {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                display: CapabilityStatus::Available,
                keyboard: CapabilityStatus::Available,
                pointer: CapabilityStatus::Emulated,
                gamepad: CapabilityStatus::Unavailable,
                audio: CapabilityStatus::Available,
                storage: CapabilityStatus::Unknown,
            }
        }

        fn now(&self) -> Duration {
            self.time
        }

        fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError> {
            Ok(InputSnapshot::default())
        }

        fn present(&mut self, _frame: &FrameOutput) -> Result<(), RuntimeError> {
            self.presentations += 1;
            Ok(())
        }

        fn audio(&mut self, request: &AudioRequest) -> Result<(), RuntimeError> {
            self.audio_requests.push(request.clone());
            Ok(())
        }
    }

    #[test]
    fn rejects_zero_dimensions() {
        let cfg = RuntimeConfig { width: 0, ..RuntimeConfig::default() };
        assert!(matches!(
            Runtime::new(MockHost::default(), cfg),
            Err(RuntimeError::InvalidConfig(_))
        ));
    }

    #[test]
    fn frame_step_dispatches_audio_and_presents() {
        // The runtime captures the host clock as its origin during construction.
        // Advance the mock clock afterwards so this frame really has a 16 ms delta.
        let host = MockHost::default();
        let mut runtime = Runtime::new(host, RuntimeConfig::default()).unwrap();
        runtime.host_mut().time = Duration::from_millis(16);
        let frame = runtime
            .step(|ctx| {
                assert_eq!(ctx.frame_number, 0);
                FrameOutput {
                    draw_commands: vec![DrawCommand::Clear {
                        rgba: [0, 0, 0, 255],
                    }],
                    audio: vec![AudioRequest::Beep {
                        frequency_hz: 440,
                        duration_ms: 50,
                    }],
                }
            })
            .unwrap();
        assert_eq!(frame.delta, Duration::from_millis(16));
        assert_eq!(runtime.host().presentations, 1);
        assert_eq!(runtime.host().audio_requests.len(), 1);
    }

    #[test]
    fn rgba_image_rejects_truncated_pixel_buffers() {
        assert!(RgbaImage::new(2, 2, vec![0; 15]).is_err());
        assert_eq!(RgbaImage::new(1, 1, vec![0, 1, 2, 3]).unwrap().pixels.len(), 4);
    }
}
