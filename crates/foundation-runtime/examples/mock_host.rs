use foundation_runtime::{
    AudioRequest, Capabilities, CapabilityStatus, DrawCommand, FrameOutput, InputSnapshot,
    Runtime, RuntimeConfig, RuntimeError, RuntimeHost,
};
use std::time::Duration;

struct MockHost;
impl RuntimeHost for MockHost {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            display: CapabilityStatus::Emulated,
            keyboard: CapabilityStatus::Available,
            pointer: CapabilityStatus::Unavailable,
            gamepad: CapabilityStatus::Unavailable,
            audio: CapabilityStatus::Unavailable,
            storage: CapabilityStatus::Unknown,
        }
    }
    fn now(&self) -> Duration { Duration::from_millis(16) }
    fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError> { Ok(InputSnapshot::default()) }
    fn present(&mut self, frame: &FrameOutput) -> Result<(), RuntimeError> {
        println!("present {} commands", frame.draw_commands.len());
        Ok(())
    }
    fn audio(&mut self, _request: &AudioRequest) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("audio"))
    }
}
fn main() -> Result<(), RuntimeError> {
    let mut runtime = Runtime::new(MockHost, RuntimeConfig::default())?;
    let frame = runtime.step(|_| FrameOutput {
        draw_commands: vec![DrawCommand::Clear { rgba: [20, 30, 40, 255] }],
        audio: vec![],
    })?;
    println!("frame {} elapsed {:?}", frame.frame_number, frame.elapsed);
    println!("capabilities: {:?}", runtime.capabilities());
    Ok(())
}
