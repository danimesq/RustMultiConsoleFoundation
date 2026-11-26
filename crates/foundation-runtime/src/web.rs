//! Browser Canvas 2D and WebAudio host for Foundation game consumers.
//!
//! The host owns the canvas, browser input listeners, decoded image resources and
//! WebAudio output. Game code exchanges only Foundation IDs and draw/audio requests.
//! This adapter intentionally does not include WebGL or SDL.

use crate::{
    AudioClipId, AudioRequest, ButtonState, Capabilities, CapabilityStatus, DrawCommand,
    FrameOutput, InputSnapshot, PadDirection, PlayerInputSnapshot, Rect, RgbaImage,
    RuntimeConfig, RuntimeError, RuntimeHost, TextureId,
};
use js_sys::{ArrayBuffer, Function, Reflect, Uint8Array};
use crate::web_export::{validate_web_export_path, WebAssetKind, WebExportManifest};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, CanvasRenderingContext2d, Event,
    HtmlCanvasElement, ImageData, KeyboardEvent, Response, Url, Window,
};

const MAX_AUDIO_VOICES: usize = 48;

#[derive(Default)]
struct BrowserInput {
    held: HashSet<String>,
    pressed: [u32; 2],
    released: [u32; 2],
    quit_requested: bool,
}

struct ActiveVoice {
    clip: AudioClipId,
    source: AudioBufferSourceNode,
    /// Retain the connected volume node for the entire voice lifetime.
    _gain: web_sys::GainNode,
    sequence: u64,
    looped: bool,
    ends_at: f64,
}

/// Canvas 2D + WebAudio runtime host for wasm32-unknown-unknown.
///
/// The browser application drives the host from requestAnimationFrame. Call
/// resume_audio() from a click/tap handler to comply with autoplay restrictions.
pub struct Canvas2dHost {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    config: RuntimeConfig,
    started_at_ms: f64,
    window: Window,
    input: Rc<RefCell<BrowserInput>>,
    key_down: Closure<dyn FnMut(Event)>,
    key_up: Closure<dyn FnMut(Event)>,
    blur: Closure<dyn FnMut(Event)>,
    textures: HashMap<TextureId, HtmlCanvasElement>,
    audio_context: Option<AudioContext>,
    audio_buffers: HashMap<AudioClipId, AudioBuffer>,
    audio_volumes: HashMap<AudioClipId, f32>,
    active_voices: Vec<ActiveVoice>,
    next_texture_id: u64,
    next_audio_clip_id: u64,
    next_voice_sequence: u64,
}

impl Canvas2dHost {
    /// Attach the host to a caller-owned canvas element.
    pub fn from_canvas(
        canvas: HtmlCanvasElement,
        config: RuntimeConfig,
    ) -> Result<Self, RuntimeError> {
        if config.width == 0 || config.height == 0 {
            return Err(RuntimeError::InvalidConfig(
                "logical dimensions must be non-zero",
            ));
        }

        let window = web_sys::window()
            .ok_or_else(|| RuntimeError::Host("browser window is unavailable".to_owned()))?;
        let performance = window
            .performance()
            .ok_or_else(|| RuntimeError::Host("browser performance clock is unavailable".to_owned()))?;

        canvas.set_width(config.width);
        canvas.set_height(config.height);
        let _ = canvas.set_attribute("tabindex", "0");
        let context = canvas
            .get_context("2d")
            .map_err(js_error)?
            .ok_or(RuntimeError::Unsupported("Canvas 2D context"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|error| js_error(error.into()))?;
        context.set_image_smoothing_enabled(!config.pixel_art);

        // Audio is optional; an unavailable device must not prevent the game from starting.
        let audio_context = AudioContext::new().ok();
        let input = Rc::new(RefCell::new(BrowserInput::default()));

        let down_state = Rc::clone(&input);
        let key_down = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(keyboard) = event.dyn_ref::<KeyboardEvent>() {
                let code = keyboard.code();
                if matches!(
                    code.as_str(),
                    "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
                        | "KeyZ" | "KeyX" | "KeyC" | "KeyV"
                        | "KeyW" | "KeyA" | "KeyS" | "KeyD"
                        | "KeyR" | "KeyT" | "KeyY" | "KeyU"
                        | "F1" | "F2" | "F3" | "F4" | "F5"
                        | "Numpad1" | "Numpad2" | "Numpad3" | "Numpad4" | "Numpad5"
                ) {
                    keyboard.prevent_default();
                }
                let mut state = down_state.borrow_mut();
                if code == "Escape" && !keyboard.repeat() {
                    state.quit_requested = true;
                } else {
                    let is_new = state.held.insert(code.clone());
                    if is_new && !keyboard.repeat() {
                        if let Some((player, slot)) = key_binding(&code) {
                            state.pressed[player] |= 1_u32 << slot;
                        }
                    }
                }
            }
        });

        let up_state = Rc::clone(&input);
        let key_up = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(keyboard) = event.dyn_ref::<KeyboardEvent>() {
                let code = keyboard.code();
                let mut state = up_state.borrow_mut();
                if state.held.remove(&code) {
                    if let Some((player, slot)) = key_binding(&code) {
                        state.released[player] |= 1_u32 << slot;
                    }
                }
            }
        });

        let blur_state = Rc::clone(&input);
        let blur = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            let mut state = blur_state.borrow_mut();
            let held = std::mem::take(&mut state.held);
            for code in held {
                if let Some((player, slot)) = key_binding(&code) {
                    state.released[player] |= 1_u32 << slot;
                }
            }
        });

        window
            .add_event_listener_with_callback("keydown", key_down.as_ref().unchecked_ref())
            .map_err(js_error)?;
        window
            .add_event_listener_with_callback("keyup", key_up.as_ref().unchecked_ref())
            .map_err(js_error)?;
        window
            .add_event_listener_with_callback("blur", blur.as_ref().unchecked_ref())
            .map_err(js_error)?;

        Ok(Self {
            canvas,
            context,
            config,
            started_at_ms: performance.now(),
            window,
            input,
            key_down,
            key_up,
            blur,
            textures: HashMap::new(),
            audio_context,
            audio_buffers: HashMap::new(),
            audio_volumes: HashMap::new(),
            active_voices: Vec::new(),
            next_texture_id: 1,
            next_audio_clip_id: 1,
            next_voice_sequence: 1,
        })
    }

    /// Locate a canvas by DOM ID and attach the host to it.
    pub fn from_canvas_id(
        canvas_id: &str,
        config: RuntimeConfig,
    ) -> Result<Self, RuntimeError> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| RuntimeError::Host("browser document is unavailable".to_owned()))?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| RuntimeError::Host(format!("canvas element #{canvas_id} was not found")))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| RuntimeError::InvalidConfig("selected element is not a canvas"))?;
        Self::from_canvas(canvas, config)
    }

    pub fn canvas(&self) -> HtmlCanvasElement {
        self.canvas.clone()
    }

    /// Resize the backing bitmap for a CSS viewport and device-pixel ratio.
    /// The logical game resolution remains unchanged.
    pub fn resize_backing_store(
        &mut self,
        css_width: u32,
        css_height: u32,
        device_pixel_ratio: f64,
    ) -> Result<(), RuntimeError> {
        if css_width == 0
            || css_height == 0
            || !device_pixel_ratio.is_finite()
            || device_pixel_ratio <= 0.0
        {
            return Err(RuntimeError::InvalidConfig("invalid canvas viewport dimensions"));
        }
        self.canvas
            .set_width((css_width as f64 * device_pixel_ratio).round() as u32);
        self.canvas
            .set_height((css_height as f64 * device_pixel_ratio).round() as u32);
        Ok(())
    }

    /// Resume suspended audio. Call this from a click/tap handler.
    pub async fn resume_audio(&self) -> Result<(), RuntimeError> {
        let context = self
            .audio_context
            .as_ref()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        JsFuture::from(context.resume().map_err(js_error)?)
            .await
            .map_err(js_error)?;
        Ok(())
    }

    /// Upload RGBA pixels into an offscreen browser canvas and return a stable texture ID.
    pub fn register_texture(&mut self, image: &RgbaImage) -> Result<TextureId, RuntimeError> {
        if image.width == 0 || image.height == 0 {
            return Err(RuntimeError::InvalidConfig("texture dimensions must be non-zero"));
        }
        let document = self
            .window
            .document()
            .ok_or_else(|| RuntimeError::Host("browser document is unavailable".to_owned()))?;
        let texture_canvas = document
            .create_element("canvas")
            .map_err(js_error)?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|error| js_error(error.into()))?;
        texture_canvas.set_width(image.width);
        texture_canvas.set_height(image.height);
        let texture_context = texture_canvas
            .get_context("2d")
            .map_err(js_error)?
            .ok_or(RuntimeError::Unsupported("offscreen Canvas 2D context"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|error| js_error(error.into()))?;
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(image.pixels.as_slice()),
            image.width,
            image.height,
        )
        .map_err(js_error)?;
        // web-sys currently gates put_image_data behind its unstable API cfg.
        // Invoke the standardized Canvas method dynamically instead of requiring
        // a global web_sys_unstable_apis flag for every consumer build.
        let put_image_data = Reflect::get(
            texture_context.as_ref(),
            &JsValue::from_str("putImageData"),
        )
        .map_err(js_error)?
        .dyn_into::<Function>()
        .map_err(js_error)?;
        put_image_data
            .call3(
                texture_context.as_ref(),
                image_data.as_ref(),
                &JsValue::from_f64(0.0),
                &JsValue::from_f64(0.0),
            )
            .map_err(js_error)?;

        let id = TextureId(self.next_texture_id);
        self.next_texture_id = self.next_texture_id.saturating_add(1);
        self.textures.insert(id, texture_canvas);
        Ok(id)
    }

    /// Decode PNG/BMP bytes and register a browser-owned texture.
    pub fn register_image_bytes(&mut self, bytes: &[u8]) -> Result<TextureId, RuntimeError> {
        let image = decode_image_bytes(bytes)?;
        self.register_texture(&image)
    }

    pub async fn load_image_url(&mut self, url: &str) -> Result<TextureId, RuntimeError> {
        let bytes = fetch_bytes(&self.window, url).await?;
        self.register_image_bytes(&bytes)
    }

    /// Decode an audio file supported by the current browser into a WebAudio buffer.
    ///
    /// This uses decodeAudioData(), so exported assets can use browser-supported formats
    /// such as WAV, OGG, MP3, or AAC/M4A where the browser supports them.
    pub async fn register_audio_bytes(
        &mut self,
        bytes: &[u8],
        volume_percent: i32,
    ) -> Result<AudioClipId, RuntimeError> {
        let context = self
            .audio_context
            .clone()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        let encoded = Uint8Array::from(bytes);
        let promise = context
            .decode_audio_data(&encoded.buffer())
            .map_err(js_error)?;
        let decoded = JsFuture::from(promise)
            .await
            .map_err(js_error)?
            .dyn_into::<AudioBuffer>()
            .map_err(js_error)?;

        let id = AudioClipId(self.next_audio_clip_id);
        self.next_audio_clip_id = self.next_audio_clip_id.saturating_add(1);
        self.audio_buffers.insert(id, decoded);
        self.audio_volumes
            .insert(id, volume_percent.clamp(0, 400) as f32 / 100.0);
        Ok(id)
    }

    /// Fetch and decode a browser-supported audio file from a web package URL.
    pub async fn load_audio_url(
        &mut self,
        url: &str,
        volume_percent: i32,
    ) -> Result<AudioClipId, RuntimeError> {
        let bytes = fetch_bytes(&self.window, url).await?;
        self.register_audio_bytes(&bytes, volume_percent).await
    }

    /// Load an exported-game manifest. Paths resolve relative to the manifest URL.
    /// Data entries are retained as bytes for game-specific INI/config parsing.
    pub async fn load_asset_manifest(
        &mut self,
        manifest_url: &str,
    ) -> Result<LoadedWebAssets, RuntimeError> {
        let bytes = fetch_bytes(&self.window, manifest_url).await?;
        let json = String::from_utf8(bytes)
            .map_err(|error| RuntimeError::Host(format!("manifest is not UTF-8: {error}")))?;
        let manifest = WebExportManifest::parse(&json)?;
        let page_url = self.window.location().href().map_err(js_error)?;
        let base = Url::new_with_base(manifest_url, &page_url).map_err(js_error)?;

        let mut loaded = LoadedWebAssets::default();
        for asset in manifest.assets {
            let mut candidates = Vec::with_capacity(1 + asset.fallbacks.len());
            candidates.push(asset.path);
            candidates.extend(asset.fallbacks);
            let mut last_error = None;
            let mut registered = false;
            for path in candidates {
                validate_web_export_path(&path)?;
                let url = Url::new_with_base(&path, &base.href()).map_err(js_error)?;
                let attempt = async {
                    let bytes = fetch_bytes(&self.window, &url.href()).await?;
                    match asset.kind {
                        WebAssetKind::Image => {
                            let image = decode_image_bytes(&bytes)?;
                            let id = self.register_texture(&image)?;
                            loaded.textures.insert(asset.id.clone(), id);
                            loaded.images.insert(asset.id.clone(), image);
                        }
                        WebAssetKind::Audio => {
                            let id = self
                                .register_audio_bytes(&bytes, asset.volume_percent)
                                .await?;
                            loaded.audio_clips.insert(asset.id.clone(), id);
                        }
                        WebAssetKind::Data => {
                            loaded.data.insert(asset.id.clone(), bytes);
                        }
                    }
                    Ok::<(), RuntimeError>(())
                }
                .await;
                match attempt {
                    Ok(()) => {
                        registered = true;
                        break;
                    }
                    Err(error) => last_error = Some(error),
                }
            }
            if !registered {
                return Err(RuntimeError::Host(format!(
                    "all packaged sources failed for asset {}: {}",
                    asset.id,
                    last_error
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "no source paths were declared".to_owned())
                )));
            }
        }
        Ok(loaded)
    }

    /// Load a Foundation Web export. All asset paths are relative to the manifest URL.
    pub async fn load_web_export(
        &mut self,
        manifest_url: &str,
    ) -> Result<LoadedWebAssets, RuntimeError> {
        self.load_asset_manifest(manifest_url).await
    }

    fn create_beep_buffer(
        &self,
        frequency_hz: u32,
        duration_ms: u32,
    ) -> Result<AudioBuffer, RuntimeError> {
        let context = self
            .audio_context
            .as_ref()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        let frequency = frequency_hz.clamp(40, 12_000) as f32;
        let frames = ((44_100_u64 * duration_ms.min(5_000) as u64) / 1_000).max(1) as usize;
        let mut samples = Vec::with_capacity(frames);
        for frame in 0..frames {
            let phase = std::f32::consts::TAU * frequency * frame as f32 / 44_100.0;
            samples.push(phase.sin() * 0.12);
        }
        let buffer = context.create_buffer(2, frames as u32, 44_100.0).map_err(js_error)?;
        buffer.copy_to_channel(&samples, 0).map_err(js_error)?;
        buffer.copy_to_channel(&samples, 1).map_err(js_error)?;
        Ok(buffer)
    }

    fn play_buffer(
        &mut self,
        clip: AudioClipId,
        buffer: AudioBuffer,
        looped: bool,
    ) -> Result<(), RuntimeError> {
        let context = self
            .audio_context
            .clone()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        self.prune_finished_voices();
        self.stop_matching_voice(clip);

        if self.active_voices.len() >= MAX_AUDIO_VOICES {
            if let Some((index, _)) = self
                .active_voices
                .iter()
                .enumerate()
                .min_by_key(|(_, voice)| voice.sequence)
            {
                let oldest = self.active_voices.remove(index);
                let _ = oldest.source.stop();
            }
        }

        let source = context.create_buffer_source().map_err(js_error)?;
        source.set_buffer(Some(&buffer));
        source.set_loop(looped);
        let gain = context.create_gain().map_err(js_error)?;
        let volume = self.audio_volumes.get(&clip).copied().unwrap_or(1.0);
        gain.gain().set_value(volume);
        source.connect_with_audio_node(&gain).map_err(js_error)?;
        gain.connect_with_audio_node(&context.destination())
            .map_err(js_error)?;
        source.start().map_err(js_error)?;

        let sequence = self.next_voice_sequence;
        self.next_voice_sequence = self.next_voice_sequence.saturating_add(1);
        let ends_at = if looped {
            f64::INFINITY
        } else {
            context.current_time() + buffer.duration()
        };
        self.active_voices.push(ActiveVoice {
            clip,
            source,
            _gain: gain,
            sequence,
            looped,
            ends_at,
        });
        Ok(())
    }

    fn prune_finished_voices(&mut self) {
        let now = self
            .audio_context
            .as_ref()
            .map(AudioContext::current_time)
            .unwrap_or(0.0);
        self.active_voices
            .retain(|voice| voice.looped || voice.ends_at > now);
    }

    fn stop_matching_voice(&mut self, clip: AudioClipId) {
        let mut retained = Vec::with_capacity(self.active_voices.len());
        for voice in self.active_voices.drain(..) {
            if voice.clip == clip {
                let _ = voice.source.stop();
            } else {
                retained.push(voice);
            }
        }
        self.active_voices = retained;
    }

    fn stop_all_voices(&mut self) {
        for voice in self.active_voices.drain(..) {
            let _ = voice.source.stop();
        }
    }

    fn release_all_held_keys(&mut self) {
        let mut state = self.input.borrow_mut();
        let held = std::mem::take(&mut state.held);
        for code in held {
            if let Some((player, slot)) = key_binding(&code) {
                state.released[player] |= 1_u32 << slot;
            }
        }
    }

    fn input_snapshot(&mut self) -> InputSnapshot {
        let mut state = self.input.borrow_mut();
        let players = std::array::from_fn(|player| PlayerInputSnapshot {
            direction: direction_for(&state.held, player),
            held_buttons: held_mask(&state.held, player),
            pressed_buttons: std::mem::take(&mut state.pressed[player]),
            released_buttons: std::mem::take(&mut state.released[player]),
        });
        let p1 = players[0];
        let vertical_up = matches!(p1.direction, PadDirection::Up | PadDirection::UpLeft | PadDirection::UpRight);
        let vertical_down = matches!(p1.direction, PadDirection::Down | PadDirection::DownLeft | PadDirection::DownRight);
        let horizontal_left = matches!(p1.direction, PadDirection::Left | PadDirection::UpLeft | PadDirection::DownLeft);
        let horizontal_right = matches!(p1.direction, PadDirection::Right | PadDirection::UpRight | PadDirection::DownRight);
        let button = |slot: u8| ButtonState {
            pressed: p1.pressed_buttons & (1_u32 << slot) != 0,
            held: p1.held_buttons & (1_u32 << slot) != 0,
            released: p1.released_buttons & (1_u32 << slot) != 0,
        };
        InputSnapshot {
            up: ButtonState { pressed: false, held: vertical_up, released: false },
            down: ButtonState { pressed: false, held: vertical_down, released: false },
            left: ButtonState { pressed: false, held: horizontal_left, released: false },
            right: ButtonState { pressed: false, held: horizontal_right, released: false },
            confirm: button(0),
            cancel: button(1),
            quit_requested: state.quit_requested,
            players,
            ..Default::default()
        }
    }
}

impl crate::TextureRegistrar for Canvas2dHost {
    fn register_texture(&mut self, image: &RgbaImage) -> Result<TextureId, RuntimeError> {
        Canvas2dHost::register_texture(self, image)
    }
}

impl RuntimeHost for Canvas2dHost {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            display: CapabilityStatus::Available,
            keyboard: CapabilityStatus::Available,
            pointer: CapabilityStatus::Unavailable,
            gamepad: CapabilityStatus::Unavailable,
            audio: if self.audio_context.is_some() {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Unavailable
            },
            storage: CapabilityStatus::Unavailable,
        }
    }

    fn now(&self) -> Duration {
        let elapsed = web_sys::window()
            .and_then(|window| window.performance())
            .map(|performance| (performance.now() - self.started_at_ms).max(0.0))
            .unwrap_or(0.0);
        Duration::from_secs_f64(elapsed / 1_000.0)
    }

    fn poll_input(&mut self) -> Result<InputSnapshot, RuntimeError> {
        Ok(self.input_snapshot())
    }

    fn reset_input_after_pause(&mut self) {
        self.release_all_held_keys();
    }

    fn present(&mut self, output: &FrameOutput) -> Result<(), RuntimeError> {
        let width = f64::from(self.canvas.width());
        let height = f64::from(self.canvas.height());
        let logical_width = f64::from(self.config.width);
        let logical_height = f64::from(self.config.height);
        let scale = (width / logical_width).min(height / logical_height);
        let offset_x = (width - logical_width * scale) / 2.0;
        let offset_y = (height - logical_height * scale) / 2.0;

        self.context
            .set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            .map_err(js_error)?;
        self.context.set_fill_style_str("#000");
        self.context.fill_rect(0.0, 0.0, width, height);
        self.context
            .set_transform(scale, 0.0, 0.0, scale, offset_x, offset_y)
            .map_err(js_error)?;
        self.context.set_image_smoothing_enabled(!self.config.pixel_art);

        for command in &output.draw_commands {
            match command {
                DrawCommand::Clear { rgba } => {
                    self.context.set_fill_style_str(&rgba_css(*rgba));
                    self.context.fill_rect(0.0, 0.0, logical_width, logical_height);
                }
                DrawCommand::Rect { rect, rgba } => {
                    self.context.set_fill_style_str(&rgba_css(*rgba));
                    self.context.fill_rect(
                        f64::from(rect.x),
                        f64::from(rect.y),
                        f64::from(rect.width),
                        f64::from(rect.height),
                    );
                }
                DrawCommand::Texture {
                    texture,
                    source,
                    destination,
                    flip_horizontal,
                } => {
                    let image = self.textures.get(texture).ok_or_else(|| {
                        RuntimeError::Host(format!("texture {} is not registered", texture.0))
                    })?;
                    let source = source.unwrap_or(Rect::new(0, 0, image.width(), image.height()));
                    let dx = f64::from(destination.x);
                    let dy = f64::from(destination.y);
                    let dw = f64::from(destination.width);
                    let dh = f64::from(destination.height);

                    if *flip_horizontal {
                        self.context.save();
                        self.context.translate(dx + dw, 0.0).map_err(js_error)?;
                        self.context.scale(-1.0, 1.0).map_err(js_error)?;
                        let drawn = self.context
                            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                image,
                                f64::from(source.x),
                                f64::from(source.y),
                                f64::from(source.width),
                                f64::from(source.height),
                                0.0,
                                dy,
                                dw,
                                dh,
                            );
                        self.context.restore();
                        drawn.map_err(js_error)?;
                    } else {
                        self.context
                            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                image,
                                f64::from(source.x),
                                f64::from(source.y),
                                f64::from(source.width),
                                f64::from(source.height),
                                dx,
                                dy,
                                dw,
                                dh,
                            )
                            .map_err(js_error)?;
                    }
                }
            }
        }
        self.context
            .set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            .map_err(js_error)?;
        Ok(())
    }

    fn audio(&mut self, request: &AudioRequest) -> Result<(), RuntimeError> {
        match request {
            AudioRequest::Beep { frequency_hz, duration_ms } => {
                let buffer = self.create_beep_buffer(*frequency_hz, *duration_ms)?;
                let ephemeral_id = AudioClipId(self.next_audio_clip_id);
                self.next_audio_clip_id = self.next_audio_clip_id.saturating_add(1);
                self.play_buffer(ephemeral_id, buffer, false)
            }
            AudioRequest::PlayClip { clip, looped } => {
                let buffer = self.audio_buffers.get(clip).cloned().ok_or_else(|| {
                    RuntimeError::Host(format!("audio clip {} is not loaded", clip.0))
                })?;
                self.play_buffer(*clip, buffer, *looped)
            }
            AudioRequest::StopClip { clip } => {
                self.stop_matching_voice(*clip);
                Ok(())
            }
            AudioRequest::StopAll => {
                self.stop_all_voices();
                Ok(())
            }
        }
    }
}

impl Drop for Canvas2dHost {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            "keydown",
            self.key_down.as_ref().unchecked_ref(),
        );
        let _ = self.window.remove_event_listener_with_callback(
            "keyup",
            self.key_up.as_ref().unchecked_ref(),
        );
        let _ = self.window.remove_event_listener_with_callback(
            "blur",
            self.blur.as_ref().unchecked_ref(),
        );
        self.stop_all_voices();
    }
}


fn decode_image_bytes(bytes: &[u8]) -> Result<RgbaImage, RuntimeError> {
    let decoded = image::load_from_memory(bytes)
        .map_err(|error| RuntimeError::Host(format!("image decode failed: {error}")))?
        .to_rgba8();
    RgbaImage::new(decoded.width(), decoded.height(), decoded.into_raw())
}

async fn fetch_bytes(window: &Window, url: &str) -> Result<Vec<u8>, RuntimeError> {
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(js_error)?
        .dyn_into::<Response>()
        .map_err(js_error)?;
    if !response.ok() {
        return Err(RuntimeError::Host(format!(
            "asset request returned HTTP {} for {url}",
            response.status()
        )));
    }
    let buffer: ArrayBuffer = JsFuture::from(response.array_buffer().map_err(js_error)?)
        .await
        .map_err(js_error)?
        .dyn_into()
        .map_err(js_error)?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

fn key_binding(code: &str) -> Option<(usize, u8)> {
    Some(match code {
        "KeyZ" => (0, 0),
        "KeyX" => (0, 1),
        "KeyC" => (0, 2),
        "KeyV" => (0, 3),
        "F1" => (0, 8),
        "F2" => (0, 9),
        "F3" => (0, 10),
        "F4" => (0, 11),
        "F5" => (0, 12),
        "KeyR" => (1, 0),
        "KeyT" => (1, 1),
        "KeyY" => (1, 2),
        "KeyU" => (1, 3),
        "Numpad1" => (1, 8),
        "Numpad2" => (1, 9),
        "Numpad3" => (1, 10),
        "Numpad4" => (1, 11),
        "Numpad5" => (1, 12),
        _ => return None,
    })
}

fn held_mask(held: &HashSet<String>, player: usize) -> u32 {
    held.iter()
        .filter_map(|key| key_binding(key))
        .filter(|(owner, _)| *owner == player)
        .fold(0, |mask, (_, slot)| mask | (1_u32 << slot))
}

fn direction_for(held: &HashSet<String>, player: usize) -> PadDirection {
    let (up, down, left, right) = if player == 0 {
        ("ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight")
    } else {
        ("KeyW", "KeyS", "KeyA", "KeyD")
    };
    match (
        held.contains(up),
        held.contains(down),
        held.contains(left),
        held.contains(right),
    ) {
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

fn rgba_css(rgba: [u8; 4]) -> String {
    format!(
        "rgba({}, {}, {}, {:.4})",
        rgba[0],
        rgba[1],
        rgba[2],
        f32::from(rgba[3]) / 255.0
    )
}

fn js_error(error: JsValue) -> RuntimeError {
    RuntimeError::Host(error.as_string().unwrap_or_else(|| format!("{error:?}")))
}
