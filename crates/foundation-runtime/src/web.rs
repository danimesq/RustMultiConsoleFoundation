//! Browser Canvas 2D and WebAudio host for Foundation game consumers.
//!
//! The host owns the canvas, browser input listeners, decoded image resources and
//! WebAudio output. Game code exchanges only Foundation IDs and draw/audio requests.
//! This adapter intentionally does not include WebGL or SDL.

use crate::audio::{decode_wav_pcm, generate_beep_pcm, MIXER_RATE_HZ};
use crate::{
    AudioClipId, AudioRequest, ButtonState, Capabilities, CapabilityStatus, DrawCommand,
    FrameOutput, InputSnapshot, PadDirection, PhysicalKey, PlayerInputSnapshot, PlayerOneKeyMap,
    Rect, RgbaImage, RuntimeConfig, RuntimeError, RuntimeHost, TextureId,
};
use js_sys::{Array, ArrayBuffer, Function, Int16Array, Object, Reflect, Uint8Array};
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
    pending_physical_key: Option<PhysicalKey>,
    p1_key_map: PlayerOneKeyMap,
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

/// Resources fetched and decoded from a Foundation Web export manifest.
#[derive(Default)]
pub struct LoadedWebAssets {
    /// Stable game-facing image IDs mapped to registered Foundation textures.
    pub textures: HashMap<String, TextureId>,
    /// Decoded image pixels retained for consumers that need the source dimensions/data.
    pub images: HashMap<String, RgbaImage>,
    /// Stable game-facing audio IDs mapped to registered Foundation audio clips.
    pub audio_clips: HashMap<String, AudioClipId>,
    /// Unmodified bytes for game-specific configuration and data files.
    pub data: HashMap<String, Vec<u8>>,
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
    audio_pcm: HashMap<AudioClipId, Vec<i16>>,
    active_voices: Vec<ActiveVoice>,
    audio_worklet_node: Option<JsValue>,
    audio_worklet_port: Option<JsValue>,
    audio_worklet_attempted: bool,
    audio_worklet_ready: bool,
    pending_audio_requests: Vec<AudioRequest>,
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
        let audio_context = create_audio_context();
        let input = Rc::new(RefCell::new(BrowserInput::default()));

        let down_state = Rc::clone(&input);
        let key_down = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            if let Some(keyboard) = event.dyn_ref::<KeyboardEvent>() {
                let code = keyboard.code();
                if code.starts_with("Key")
                    || code.starts_with("Digit")
                    || code.starts_with("Numpad")
                    || code.starts_with("Arrow")
                    || code.starts_with("F")
                    || matches!(code.as_str(), "Escape" | "Space" | "Enter" | "Tab" | "Backspace")
                {
                    keyboard.prevent_default();
                }
                let mut state = down_state.borrow_mut();
                if code == "Escape" && !keyboard.repeat() {
                    state.quit_requested = true;
                } else {
                    let is_new = state.held.insert(code.clone());
                    if is_new && !keyboard.repeat() {
                        state.pending_physical_key = physical_key(&code);
                        if let Some((player, slot)) = key_binding(&code, state.p1_key_map) {
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
                    if let Some((player, slot)) = key_binding(&code, state.p1_key_map) {
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
                if let Some((player, slot)) = key_binding(&code, state.p1_key_map) {
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
            audio_pcm: HashMap::new(),
            active_voices: Vec::new(),
            audio_worklet_node: None,
            audio_worklet_port: None,
            audio_worklet_attempted: false,
            audio_worklet_ready: false,
            pending_audio_requests: Vec::new(),
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
    pub async fn resume_audio(&mut self) -> Result<(), RuntimeError> {
        let context = self
            .audio_context
            .clone()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        JsFuture::from(context.resume().map_err(js_error)?)
            .await
            .map_err(js_error)?;

        if !self.audio_worklet_attempted {
            self.audio_worklet_attempted = true;
            self.audio_worklet_ready = self.initialize_audio_worklet().await.unwrap_or(false);
            if self.audio_worklet_ready {
                let registered = self.audio_pcm.iter()
                    .map(|(clip, pcm)| (*clip, pcm.clone()))
                    .collect::<Vec<_>>();
                for (clip, pcm) in registered {
                    self.post_worklet_registration(clip, &pcm)?;
                }
            }
            let pending = std::mem::take(&mut self.pending_audio_requests);
            for request in pending {
                self.dispatch_audio_request(&request)?;
            }
        }
        Ok(())
    }

    pub fn audio_backend(&self) -> &'static str {
        if self.audio_worklet_ready {
            "AudioWorklet deterministic S16 PCM mixer"
        } else if self.audio_worklet_attempted {
            "WebAudio AudioBuffer fallback"
        } else {
            "WebAudio awaiting user gesture"
        }
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
        if self.audio_context.is_none() {
            return Err(RuntimeError::Unsupported("WebAudio is unavailable"));
        }
        let pcm = decode_wav_pcm(bytes, volume_percent).map_err(RuntimeError::Host)?;
        self.register_pcm_samples(pcm)
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

    async fn initialize_audio_worklet(&mut self) -> Result<bool, RuntimeError> {
        let context = self.audio_context.clone()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        let global = js_sys::global();
        let worklet = Reflect::get(context.as_ref(), &JsValue::from_str("audioWorklet"))
            .map_err(js_error)?;
        if worklet.is_null() || worklet.is_undefined() { return Ok(false); }

        let blob_constructor = Reflect::get(&global, &JsValue::from_str("Blob"))
            .map_err(js_error)?.dyn_into::<Function>().map_err(js_error)?;
        let parts = Array::new();
        parts.push(&JsValue::from_str(include_str!("../assets/audio_worklet.js")));
        let blob_options = Object::new();
        Reflect::set(&blob_options, &JsValue::from_str("type"),
            &JsValue::from_str("application/javascript")).map_err(js_error)?;
        let blob_args = Array::new();
        blob_args.push(&parts);
        blob_args.push(&blob_options);
        let blob = Reflect::construct(&blob_constructor, &blob_args).map_err(js_error)?;

        let url_constructor = Reflect::get(&global, &JsValue::from_str("URL")).map_err(js_error)?;
        let create_url = Reflect::get(&url_constructor, &JsValue::from_str("createObjectURL"))
            .map_err(js_error)?.dyn_into::<Function>().map_err(js_error)?;
        let module_url = create_url.call1(&url_constructor, &blob).map_err(js_error)?;
        let add_module = Reflect::get(&worklet, &JsValue::from_str("addModule"))
            .map_err(js_error)?.dyn_into::<Function>().map_err(js_error)?;
        let promise = add_module.call1(&worklet, &module_url).map_err(js_error)?
            .dyn_into::<js_sys::Promise>().map_err(js_error)?;
        let module_result = JsFuture::from(promise).await.map_err(js_error);
        if let Ok(revoke) = Reflect::get(&url_constructor, &JsValue::from_str("revokeObjectURL")) {
            if let Ok(revoke) = revoke.dyn_into::<Function>() {
                let _ = revoke.call1(&url_constructor, &module_url);
            }
        }
        module_result?;

        let node_constructor = Reflect::get(&global, &JsValue::from_str("AudioWorkletNode"))
            .map_err(js_error)?.dyn_into::<Function>().map_err(js_error)?;
        let options = Object::new();
        Reflect::set(&options, &JsValue::from_str("numberOfInputs"), &JsValue::from_f64(0.0))
            .map_err(js_error)?;
        Reflect::set(&options, &JsValue::from_str("numberOfOutputs"), &JsValue::from_f64(1.0))
            .map_err(js_error)?;
        let output_channels = Array::new();
        output_channels.push(&JsValue::from_f64(2.0));
        Reflect::set(&options, &JsValue::from_str("outputChannelCount"), &output_channels)
            .map_err(js_error)?;
        Reflect::set(&options, &JsValue::from_str("channelCount"), &JsValue::from_f64(2.0))
            .map_err(js_error)?;
        Reflect::set(&options, &JsValue::from_str("channelCountMode"), &JsValue::from_str("explicit"))
            .map_err(js_error)?;

        let args = Array::new();
        args.push(context.as_ref());
        args.push(&JsValue::from_str("foundation-pcm-mixer"));
        args.push(&options);
        let node = Reflect::construct(&node_constructor, &args).map_err(js_error)?;
        let connect = Reflect::get(&node, &JsValue::from_str("connect"))
            .map_err(js_error)?.dyn_into::<Function>().map_err(js_error)?;
        let destination = context.destination();
        connect.call1(&node, destination.as_ref()).map_err(js_error)?;
        let port = Reflect::get(&node, &JsValue::from_str("port")).map_err(js_error)?;
        self.audio_worklet_node = Some(node);
        self.audio_worklet_port = Some(port);
        Ok(true)
    }

    fn post_worklet_message(&self, message: &JsValue) -> Result<(), RuntimeError> {
        let port = self.audio_worklet_port.as_ref()
            .ok_or_else(|| RuntimeError::Host("AudioWorklet message port is unavailable".to_owned()))?;
        let post = Reflect::get(port, &JsValue::from_str("postMessage"))
            .map_err(js_error)?.dyn_into::<Function>().map_err(js_error)?;
        post.call1(port, message).map_err(js_error)?;
        Ok(())
    }

    fn post_worklet_registration(&self, clip: AudioClipId, pcm: &[i16]) -> Result<(), RuntimeError> {
        let message = Object::new();
        Reflect::set(&message, &JsValue::from_str("type"), &JsValue::from_str("register"))
            .map_err(js_error)?;
        Reflect::set(&message, &JsValue::from_str("clip"),
            &JsValue::from_str(&clip.0.to_string())).map_err(js_error)?;
        let samples = Int16Array::from(pcm);
        Reflect::set(&message, &JsValue::from_str("pcm"), samples.as_ref()).map_err(js_error)?;
        self.post_worklet_message(message.as_ref())
    }

    fn create_pcm_audio_buffer(&self, pcm: &[i16]) -> Result<AudioBuffer, RuntimeError> {
        let context = self.audio_context.as_ref()
            .ok_or(RuntimeError::Unsupported("WebAudio is unavailable"))?;
        let frames = (pcm.len() / 2).max(1);
        let buffer = context.create_buffer(2, frames as u32, MIXER_RATE_HZ as f32).map_err(js_error)?;
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for frame in 0..frames {
            left.push(pcm.get(frame * 2).copied().unwrap_or(0) as f32 / 32768.0);
            right.push(pcm.get(frame * 2 + 1).copied().unwrap_or(0) as f32 / 32768.0);
        }
        buffer.copy_to_channel(&left, 0).map_err(js_error)?;
        buffer.copy_to_channel(&right, 1).map_err(js_error)?;
        Ok(buffer)
    }

    fn register_pcm_samples(&mut self, pcm: Vec<i16>) -> Result<AudioClipId, RuntimeError> {
        if pcm.len() < 2 || pcm.len() % 2 != 0 {
            return Err(RuntimeError::InvalidConfig("audio PCM must contain interleaved stereo frames"));
        }
        let buffer = self.create_pcm_audio_buffer(&pcm)?;
        let id = AudioClipId(self.next_audio_clip_id);
        self.next_audio_clip_id = self.next_audio_clip_id.saturating_add(1);
        self.audio_buffers.insert(id, buffer);
        self.audio_volumes.insert(id, 1.0);
        self.audio_pcm.insert(id, pcm.clone());
        if self.audio_worklet_ready { self.post_worklet_registration(id, &pcm)?; }
        Ok(id)
    }

    fn dispatch_audio_request(&mut self, request: &AudioRequest) -> Result<(), RuntimeError> {
        if self.audio_worklet_ready {
            let message = Object::new();
            match request {
                AudioRequest::Beep { .. } => return Err(RuntimeError::Host(
                    "Beep must be canonicalized before dispatch".to_owned())),
                AudioRequest::PlayClip { clip, looped } => {
                    Reflect::set(&message, &JsValue::from_str("type"), &JsValue::from_str("play"))
                        .map_err(js_error)?;
                    Reflect::set(&message, &JsValue::from_str("clip"),
                        &JsValue::from_str(&clip.0.to_string())).map_err(js_error)?;
                    Reflect::set(&message, &JsValue::from_str("looped"), &JsValue::from_bool(*looped))
                        .map_err(js_error)?;
                }
                AudioRequest::StopClip { clip } => {
                    Reflect::set(&message, &JsValue::from_str("type"), &JsValue::from_str("stop"))
                        .map_err(js_error)?;
                    Reflect::set(&message, &JsValue::from_str("clip"),
                        &JsValue::from_str(&clip.0.to_string())).map_err(js_error)?;
                }
                AudioRequest::StopAll => {
                    Reflect::set(&message, &JsValue::from_str("type"), &JsValue::from_str("stop-all"))
                        .map_err(js_error)?;
                }
            }
            return self.post_worklet_message(message.as_ref());
        }
        if !self.audio_worklet_attempted {
            self.pending_audio_requests.push(request.clone());
            return Ok(());
        }
        match request {
            AudioRequest::Beep { .. } => Err(RuntimeError::Host(
                "Beep must be canonicalized before dispatch".to_owned())),
            AudioRequest::PlayClip { clip, looped } => {
                let buffer = self.audio_buffers.get(clip).cloned().ok_or_else(|| {
                    RuntimeError::Host(format!("audio clip {} is not loaded", clip.0))
                })?;
                self.play_buffer(*clip, buffer, *looped)
            }
            AudioRequest::StopClip { clip } => { self.stop_matching_voice(*clip); Ok(()) }
            AudioRequest::StopAll => { self.stop_all_voices(); Ok(()) }
        }
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
            if let Some((player, slot)) = key_binding(&code, state.p1_key_map) {
                state.released[player] |= 1_u32 << slot;
            }
        }
    }

    fn input_snapshot(&mut self) -> InputSnapshot {
        let mut state = self.input.borrow_mut();
        let players = std::array::from_fn(|player| PlayerInputSnapshot {
            direction: direction_for(&state.held, player, state.p1_key_map),
            held_buttons: held_mask(&state.held, player, state.p1_key_map),
            pressed_buttons: std::mem::take(&mut state.pressed[player]),
            released_buttons: std::mem::take(&mut state.released[player]),
        });
        let p1 = players[0];
        let physical_key_pressed = state.pending_physical_key.take();
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
            physical_key_pressed,
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
    fn set_player_one_key_map(&mut self, map: PlayerOneKeyMap) {
        self.release_all_held_keys();
        self.input.borrow_mut().p1_key_map = map;
    }

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
                let clip = self.register_pcm_samples(generate_beep_pcm(*frequency_hz, *duration_ms))?;
                self.dispatch_audio_request(&AudioRequest::PlayClip { clip, looped: false })
            }
            AudioRequest::PlayClip { clip, looped } => {
                if !self.audio_pcm.contains_key(clip) {
                    return Err(RuntimeError::Host(format!("audio clip {} is not loaded", clip.0)));
                }
                self.dispatch_audio_request(&AudioRequest::PlayClip { clip: *clip, looped: *looped })
            }
            AudioRequest::StopClip { clip } => {
                self.dispatch_audio_request(&AudioRequest::StopClip { clip: *clip })
            }
            AudioRequest::StopAll => self.dispatch_audio_request(&AudioRequest::StopAll),
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
        if let Some(port) = self.audio_worklet_port.as_ref() {
            if let Ok(post_value) = Reflect::get(port, &JsValue::from_str("postMessage")) {
                if let Ok(post) = post_value.dyn_into::<Function>() {
                    let message = Object::new();
                    let _ = Reflect::set(
                        &message,
                        &JsValue::from_str("type"),
                        &JsValue::from_str("stop-all"),
                    );
                    let _ = post.call1(port, message.as_ref());
                }
            }
        }
        self.stop_all_voices();
    }
}


fn create_audio_context() -> Option<AudioContext> {
    let global = js_sys::global();
    let preferred = (|| {
        let constructor = Reflect::get(&global, &JsValue::from_str("AudioContext"))
            .ok()?.dyn_into::<Function>().ok()?;
        let options = Object::new();
        Reflect::set(&options, &JsValue::from_str("sampleRate"),
            &JsValue::from_f64(f64::from(MIXER_RATE_HZ))).ok()?;
        let args = Array::new();
        args.push(&options);
        Reflect::construct(&constructor, &args).ok()
            .and_then(|value| value.dyn_into::<AudioContext>().ok())
    })();
    preferred.or_else(|| AudioContext::new().ok())
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

fn physical_key(code: &str) -> Option<PhysicalKey> {
    Some(match code {
        "KeyA" => PhysicalKey::A, "KeyB" => PhysicalKey::B,
        "KeyC" => PhysicalKey::C, "KeyD" => PhysicalKey::D,
        "KeyE" => PhysicalKey::E, "KeyF" => PhysicalKey::F,
        "KeyG" => PhysicalKey::G, "KeyH" => PhysicalKey::H,
        "KeyI" => PhysicalKey::I, "KeyJ" => PhysicalKey::J,
        "KeyK" => PhysicalKey::K, "KeyL" => PhysicalKey::L,
        "KeyM" => PhysicalKey::M, "KeyN" => PhysicalKey::N,
        "KeyO" => PhysicalKey::O, "KeyP" => PhysicalKey::P,
        "KeyQ" => PhysicalKey::Q, "KeyR" => PhysicalKey::R,
        "KeyS" => PhysicalKey::S, "KeyT" => PhysicalKey::T,
        "KeyU" => PhysicalKey::U, "KeyV" => PhysicalKey::V,
        "KeyW" => PhysicalKey::W, "KeyX" => PhysicalKey::X,
        "KeyY" => PhysicalKey::Y, "KeyZ" => PhysicalKey::Z,
        "Digit0" => PhysicalKey::Digit0, "Digit1" => PhysicalKey::Digit1,
        "Digit2" => PhysicalKey::Digit2, "Digit3" => PhysicalKey::Digit3,
        "Digit4" => PhysicalKey::Digit4, "Digit5" => PhysicalKey::Digit5,
        "Digit6" => PhysicalKey::Digit6, "Digit7" => PhysicalKey::Digit7,
        "Digit8" => PhysicalKey::Digit8, "Digit9" => PhysicalKey::Digit9,
        "ArrowUp" => PhysicalKey::ArrowUp, "ArrowDown" => PhysicalKey::ArrowDown,
        "ArrowLeft" => PhysicalKey::ArrowLeft, "ArrowRight" => PhysicalKey::ArrowRight,
        "Numpad0" => PhysicalKey::Numpad0, "Numpad1" => PhysicalKey::Numpad1,
        "Numpad2" => PhysicalKey::Numpad2, "Numpad3" => PhysicalKey::Numpad3,
        "Numpad4" => PhysicalKey::Numpad4, "Numpad5" => PhysicalKey::Numpad5,
        "Numpad6" => PhysicalKey::Numpad6, "Numpad7" => PhysicalKey::Numpad7,
        "Numpad8" => PhysicalKey::Numpad8, "Numpad9" => PhysicalKey::Numpad9,
        "F1" => PhysicalKey::F1, "F2" => PhysicalKey::F2, "F3" => PhysicalKey::F3,
        "F4" => PhysicalKey::F4, "F5" => PhysicalKey::F5, "F6" => PhysicalKey::F6,
        "F7" => PhysicalKey::F7, "F8" => PhysicalKey::F8, "F9" => PhysicalKey::F9,
        "F10" => PhysicalKey::F10, "F11" => PhysicalKey::F11, "F12" => PhysicalKey::F12,
        _ => return None,
    })
}

fn key_binding(code: &str, map: PlayerOneKeyMap) -> Option<(usize, u8)> {
    let key = physical_key(code)?;
    if let Some(action) = map.action_index(key) {
        if action >= 4 {
            return Some((0, (action - 4) as u8));
        }
    }
    Some(match code {
        "F1" => (0, 8), "F2" => (0, 9), "F3" => (0, 10),
        "F4" => (0, 11), "F5" => (0, 12),
        "KeyR" => (1, 0), "KeyT" => (1, 1), "KeyY" => (1, 2), "KeyU" => (1, 3),
        "Numpad1" => (1, 8), "Numpad2" => (1, 9), "Numpad3" => (1, 10),
        "Numpad4" => (1, 11), "Numpad5" => (1, 12),
        _ => return None,
    })
}

fn held_mask(held: &HashSet<String>, player: usize, map: PlayerOneKeyMap) -> u32 {
    held.iter()
        .filter_map(|key| key_binding(key, map))
        .filter(|(owner, _)| *owner == player)
        .fold(0, |mask, (_, slot)| mask | (1_u32 << slot))
}

fn direction_for(held: &HashSet<String>, player: usize, map: PlayerOneKeyMap) -> PadDirection {
    let is_down = |key: PhysicalKey| held.iter().any(|code| physical_key(code) == Some(key));
    let (up, down, left, right) = if player == 0 {
        (is_down(map.keys[0]), is_down(map.keys[1]), is_down(map.keys[2]), is_down(map.keys[3]))
    } else {
        (
            held.contains("KeyW"), held.contains("KeyS"),
            held.contains("KeyA"), held.contains("KeyD"),
        )
    };
    match (
        up,
        down,
        left,
        right,
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

#[cfg(test)]
mod keyboard_mapping_tests {
    use super::*;

    #[test]
    fn browser_physical_codes_match_shared_key_vocabulary() {
        assert_eq!(physical_key("KeyQ"), Some(PhysicalKey::Q));
        assert_eq!(physical_key("Digit5"), Some(PhysicalKey::Digit5));
        assert_eq!(physical_key("Numpad2"), Some(PhysicalKey::Numpad2));
        assert_eq!(physical_key("ArrowLeft"), Some(PhysicalKey::ArrowLeft));
    }

    #[test]
    fn custom_player_one_map_drives_attack_slots_and_directions() {
        let map = PlayerOneKeyMap {
            keys: [
                PhysicalKey::W, PhysicalKey::S, PhysicalKey::A, PhysicalKey::D,
                PhysicalKey::J, PhysicalKey::K, PhysicalKey::U, PhysicalKey::I,
            ],
        };
        assert_eq!(key_binding("KeyJ", map), Some((0, 0)));
        assert_eq!(key_binding("KeyI", map), Some((0, 3)));
        assert_eq!(direction_for(&HashSet::from(["KeyW".to_owned()]), 0, map), PadDirection::Up);
        assert_eq!(direction_for(&HashSet::from(["KeyW".to_owned()]), 1, map), PadDirection::Up);
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
