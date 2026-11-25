//! Platform-neutral runtime contracts for RustMultiConsoleFoundation.
//!
//! No SDL, Bevy, browser, Orbital, or console SDK dependencies are included.
//! Platform adapters implement RuntimeHost and report support through Capabilities.

use std::time::Duration;

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
        Self { title: "Foundation Game".into(), width: 320, height: 240, pixel_art: true }
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

/// Normalized button state, independent of platform key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonState {
    pub pressed: bool,
    pub held: bool,
    pub released: bool,
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
}

/// Renderer-neutral primitives for the first API slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawCommand {
    Clear { rgba: [u8; 4] },
    Rect { x: i32, y: i32, width: u32, height: u32, rgba: [u8; 4] },
}

/// Renderer-neutral output for a frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameOutput {
    pub draw_commands: Vec<DrawCommand>,
    pub audio: Vec<AudioRequest>,
}

/// Semantic audio request. Hosts may explicitly report audio as unsupported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioRequest {
    Beep { frequency_hz: u32, duration_ms: u32 },
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
/// It owns the host integration and must not initialize a competing window,
/// event loop, or input path.
pub trait RuntimeHost {
    fn capabilities(&self) -> Capabilities;
    /// Monotonic time from an adapter-defined origin.
    fn now(&self) -> Duration;
    /// Poll normalized input for this frame.
    fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError>;
    /// Present backend-neutral commands.
    fn present(&mut self, output: &FrameOutput) -> Result<(), RuntimeError>;
    /// Dispatch a semantic audio request, returning an error if unsupported.
    fn audio(&mut self, request: &AudioRequest) -> Result<(), RuntimeError>;
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
        Ok(Self { host, config, frame_number: 0, started_at: now, last_frame_at: now })
    }
    /// Return the requested logical configuration.
    pub fn config(&self) -> &RuntimeConfig { &self.config }
    /// Query adapter capabilities.
    pub fn capabilities(&self) -> Capabilities { self.host.capabilities() }
    /// Access the adapter for diagnostics.
    pub fn host(&self) -> &H { &self.host }

    /// Poll input, run consumer update, dispatch audio and present one frame.
    ///
    /// The consumer owns simulation timing semantics. Use a fixed-step loop in
    /// the game core when deterministic simulation is required.
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
        for request in &output.audio { self.host.audio(request)?; }
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
        fn now(&self) -> Duration { self.time }
        fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError> { Ok(InputSnapshot::default()) }
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
        assert!(matches!(Runtime::new(MockHost::default(), cfg), Err(RuntimeError::InvalidConfig(_))));
    }
    #[test]
    fn frame_step_dispatches_audio_and_presents() {
        let host = MockHost { time: Duration::from_millis(16), ..Default::default() };
        let mut runtime = Runtime::new(host, RuntimeConfig::default()).unwrap();
        let frame = runtime.step(|ctx| {
            assert_eq!(ctx.frame_number, 0);
            FrameOutput {
                draw_commands: vec![DrawCommand::Clear { rgba: [0, 0, 0, 255] }],
                audio: vec![AudioRequest::Beep { frequency_hz: 440, duration_ms: 50 }],
            }
        }).unwrap();
        assert_eq!(frame.delta, Duration::from_millis(16));
        assert_eq!(runtime.host().presentations, 1);
        assert_eq!(runtime.host().audio_requests.len(), 1);
    }
}
