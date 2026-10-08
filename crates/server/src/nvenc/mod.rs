//! H.264 encoding with NVENC (P3.1, decision 0035).
//!
//! The driver's NVENC and CUDA libraries are loaded at run time (`nvEncodeAPI64.dll` / `nvcuda.dll` on
//! Windows, `libnvidia-encode.so.1` / `libcuda.so.1` on Linux — decisions 0035, 0043), so building needs no
//! NVIDIA SDK or CUDA toolkit. NVENC runs on a CUDA context and owns its input buffers: each frame
//! (tightly packed RGBA8 — the core's capture layout) is copied into a locked input buffer, encoded,
//! and the bitstream is copied out as Annex B. Unsafe code (FFI) is allowed in this module only.
#![allow(unsafe_code)]

mod sys;

use libloading::Library;
use std::ffi::c_void;
use std::time::Instant;
use sys::*;

#[cfg(windows)]
const CUDA_LIB: &str = "nvcuda.dll";
#[cfg(windows)]
const NVENC_LIB: &str = "nvEncodeAPI64.dll";
#[cfg(target_os = "linux")]
const CUDA_LIB: &str = "libcuda.so.1";
#[cfg(target_os = "linux")]
const NVENC_LIB: &str = "libnvidia-encode.so.1";

/// Encoder settings. Bandwidth is not a constraint (INTENT principle 2): quality first.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Constant QP for every frame (lower = better quality, more bytes).
    pub qp: u32,
}

/// One encoded frame and where its time went.
pub struct Frame {
    pub bytes: Vec<u8>,
    pub idr: bool,
    /// Copying the RGBA frame into NVENC's input buffer.
    pub upload_ms: f64,
    /// Encode call + waiting for the bitstream.
    pub encode_ms: f64,
}

pub struct Encoder {
    api: NV_ENCODE_API_FUNCTION_LIST,
    encoder: *mut c_void,
    input: NV_ENC_INPUT_PTR,
    output: NV_ENC_OUTPUT_PTR,
    config: Config,
    frame: u64,
    // Keep the libraries loaded for as long as the function pointers are used.
    _nvenc: Library,
    _cuda: Library,
    cuda_ctx: *mut c_void,
}

fn check(status: NVENCSTATUS, what: &str) -> Result<(), String> {
    if status == NVENCSTATUS::NV_ENC_SUCCESS {
        Ok(())
    } else {
        Err(format!("{what}: {status:?}"))
    }
}

macro_rules! call {
    ($api:expr, $f:ident, $($arg:expr),*) => {{
        let f = $api.$f.ok_or(concat!(stringify!($f), " missing"))?;
        check(f($($arg),*), stringify!($f))
    }};
}

impl Encoder {
    pub fn new(config: Config) -> Result<Self, String> {
        unsafe {
            // CUDA driver API: a context for NVENC on the first GPU.
            let cuda = Library::new(CUDA_LIB).map_err(|e| format!("{CUDA_LIB}: {e}"))?;
            let cu_init: libloading::Symbol<unsafe extern "C" fn(u32) -> i32> =
                cuda.get(b"cuInit\0").map_err(|e| e.to_string())?;
            let cu_device_get: libloading::Symbol<unsafe extern "C" fn(*mut i32, i32) -> i32> =
                cuda.get(b"cuDeviceGet\0").map_err(|e| e.to_string())?;
            let cu_ctx_create: libloading::Symbol<
                unsafe extern "C" fn(*mut *mut c_void, u32, i32) -> i32,
            > = cuda.get(b"cuCtxCreate_v2\0").map_err(|e| e.to_string())?;
            let r = cu_init(0);
            if r != 0 {
                return Err(format!("cuInit: {r}"));
            }
            let mut dev = 0;
            let r = cu_device_get(&mut dev, 0);
            if r != 0 {
                return Err(format!("cuDeviceGet: {r}"));
            }
            let mut cuda_ctx = std::ptr::null_mut();
            let r = cu_ctx_create(&mut cuda_ctx, 0, dev);
            if r != 0 {
                return Err(format!("cuCtxCreate: {r}"));
            }
            let nvenc = match Library::new(NVENC_LIB) {
                Ok(l) => l,
                Err(e) => {
                    if let Ok(destroy) =
                        cuda.get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"cuCtxDestroy_v2\0")
                    {
                        let _ = destroy(cuda_ctx);
                    }
                    return Err(format!("{NVENC_LIB}: {e}"));
                }
            };
            // From here on `enc` owns everything created so far: an early return drops it and
            // Drop releases only the handles that exist (PR #24 review).
            let mut enc = Self {
                api: NV_ENCODE_API_FUNCTION_LIST {
                    version: NV_ENCODE_API_FUNCTION_LIST_VER,
                    ..Default::default()
                },
                encoder: std::ptr::null_mut(),
                input: std::ptr::null_mut(),
                output: std::ptr::null_mut(),
                config,
                frame: 0,
                _nvenc: nvenc,
                _cuda: cuda,
                cuda_ctx,
            };
            let create: libloading::Symbol<
                unsafe extern "C" fn(*mut NV_ENCODE_API_FUNCTION_LIST) -> NVENCSTATUS,
            > = enc
                ._nvenc
                .get(b"NvEncodeAPICreateInstance\0")
                .map_err(|e| e.to_string())?;
            check(create(&mut enc.api), "NvEncodeAPICreateInstance")?;
            let api = enc.api;

            let mut open = NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS {
                version: NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER,
                deviceType: NV_ENC_DEVICE_TYPE::NV_ENC_DEVICE_TYPE_CUDA,
                device: enc.cuda_ctx,
                apiVersion: NVENCAPI_VERSION,
                ..Default::default()
            };
            call!(api, nvEncOpenEncodeSessionEx, &mut open, &mut enc.encoder)?;

            // P1 (fastest) with the ultra-low-latency tuning: no B frames, no lookahead.
            let preset = NV_ENC_PRESET_P1_GUID;
            let tuning = NV_ENC_TUNING_INFO::NV_ENC_TUNING_INFO_ULTRA_LOW_LATENCY;
            let mut preset_cfg = NV_ENC_PRESET_CONFIG {
                version: NV_ENC_PRESET_CONFIG_VER,
                presetCfg: NV_ENC_CONFIG {
                    version: NV_ENC_CONFIG_VER,
                    ..Default::default()
                },
                ..Default::default()
            };
            call!(
                api,
                nvEncGetEncodePresetConfigEx,
                enc.encoder,
                NV_ENC_CODEC_H264_GUID,
                preset,
                tuning,
                &mut preset_cfg
            )?;
            let mut cfg = preset_cfg.presetCfg;
            // One IDR at the start, then P frames only; SPS/PPS repeated on every IDR so a
            // client can join at any IDR.
            cfg.gopLength = NVENC_INFINITE_GOPLENGTH;
            cfg.frameIntervalP = 1;
            cfg.rcParams.rateControlMode = NV_ENC_PARAMS_RC_MODE::NV_ENC_PARAMS_RC_CONSTQP;
            cfg.rcParams.constQP = NV_ENC_QP {
                qpInterP: config.qp,
                qpInterB: config.qp,
                qpIntra: config.qp,
            };
            cfg.encodeCodecConfig.h264Config.idrPeriod = NVENC_INFINITE_GOPLENGTH;
            cfg.encodeCodecConfig.h264Config.set_repeatSPSPPS(1);
            // VUI bitstream_restriction: with no B frames NVENC writes max_num_reorder_frames = 0, so a
            // decoder can output each frame at once instead of holding frames for reordering (P3.3:
            // without it the browser decoder added about four frames of delay).
            cfg.encodeCodecConfig
                .h264Config
                .h264VUIParameters
                .bitstreamRestrictionFlag = 1;

            let mut init = NV_ENC_INITIALIZE_PARAMS {
                version: NV_ENC_INITIALIZE_PARAMS_VER,
                encodeGUID: NV_ENC_CODEC_H264_GUID,
                presetGUID: preset,
                encodeWidth: config.width,
                encodeHeight: config.height,
                darWidth: config.width,
                darHeight: config.height,
                frameRateNum: config.fps,
                frameRateDen: 1,
                enablePTD: 1,
                encodeConfig: &mut cfg,
                tuningInfo: tuning,
                ..Default::default()
            };
            call!(api, nvEncInitializeEncoder, enc.encoder, &mut init)?;

            let mut input = NV_ENC_CREATE_INPUT_BUFFER {
                version: NV_ENC_CREATE_INPUT_BUFFER_VER,
                width: config.width,
                height: config.height,
                bufferFmt: NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ABGR,
                ..Default::default()
            };
            call!(api, nvEncCreateInputBuffer, enc.encoder, &mut input)?;
            enc.input = input.inputBuffer;
            let mut output = NV_ENC_CREATE_BITSTREAM_BUFFER {
                version: NV_ENC_CREATE_BITSTREAM_BUFFER_VER,
                ..Default::default()
            };
            call!(api, nvEncCreateBitstreamBuffer, enc.encoder, &mut output)?;
            enc.output = output.bitstreamBuffer;
            Ok(enc)
        }
    }

    /// Encodes one tightly packed RGBA8 frame (`width × height × 4` bytes, the core's capture
    /// layout; NVENC's ABGR format is R,G,B,A in memory). `force_idr` starts a new GOP.
    pub fn encode(&mut self, rgba: &[u8], force_idr: bool) -> Result<Frame, String> {
        let (w, h) = (self.config.width as usize, self.config.height as usize);
        if rgba.len() != w * h * 4 {
            return Err(format!(
                "frame is {} bytes, expected {}",
                rgba.len(),
                w * h * 4
            ));
        }
        let t0 = Instant::now();
        unsafe {
            let mut lock = NV_ENC_LOCK_INPUT_BUFFER {
                version: NV_ENC_LOCK_INPUT_BUFFER_VER,
                inputBuffer: self.input,
                ..Default::default()
            };
            call!(self.api, nvEncLockInputBuffer, self.encoder, &mut lock)?;
            let pitch = lock.pitch as usize;
            if pitch < w * 4 {
                // A shorter row than the frame would overrun the buffer (PR #24 review).
                let _ = self
                    .api
                    .nvEncUnlockInputBuffer
                    .map(|f| f(self.encoder, self.input));
                return Err(format!(
                    "NVENC input pitch {pitch} < {} bytes per row",
                    w * 4
                ));
            }
            let dst = lock.bufferDataPtr.cast::<u8>();
            for y in 0..h {
                std::ptr::copy_nonoverlapping(
                    rgba.as_ptr().add(y * w * 4),
                    dst.add(y * pitch),
                    w * 4,
                );
            }
            call!(self.api, nvEncUnlockInputBuffer, self.encoder, self.input)?;
            let t1 = Instant::now();

            let idr = force_idr || self.frame == 0;
            let mut pic = NV_ENC_PIC_PARAMS {
                version: NV_ENC_PIC_PARAMS_VER,
                inputWidth: self.config.width,
                inputHeight: self.config.height,
                inputPitch: pitch as u32,
                encodePicFlags: if idr {
                    NV_ENC_PIC_FLAGS::NV_ENC_PIC_FLAG_FORCEIDR as u32
                        | NV_ENC_PIC_FLAGS::NV_ENC_PIC_FLAG_OUTPUT_SPSPPS as u32
                } else {
                    0
                },
                frameIdx: self.frame as u32,
                inputTimeStamp: self.frame,
                inputBuffer: self.input,
                outputBitstream: self.output,
                bufferFmt: NV_ENC_BUFFER_FORMAT::NV_ENC_BUFFER_FORMAT_ABGR,
                pictureStruct: NV_ENC_PIC_STRUCT::NV_ENC_PIC_STRUCT_FRAME,
                ..Default::default()
            };
            call!(self.api, nvEncEncodePicture, self.encoder, &mut pic)?;
            let mut bs = NV_ENC_LOCK_BITSTREAM {
                version: NV_ENC_LOCK_BITSTREAM_VER,
                outputBitstream: self.output,
                ..Default::default()
            };
            bs.set_doNotWait(0);
            call!(self.api, nvEncLockBitstream, self.encoder, &mut bs)?;
            let bytes = std::slice::from_raw_parts(
                bs.bitstreamBufferPtr.cast::<u8>(),
                bs.bitstreamSizeInBytes as usize,
            )
            .to_vec();
            call!(self.api, nvEncUnlockBitstream, self.encoder, self.output)?;
            let t2 = Instant::now();
            self.frame += 1;
            let ms = |a: Instant, b: Instant| b.duration_since(a).as_secs_f64() * 1e3;
            Ok(Frame {
                bytes,
                idr,
                upload_ms: ms(t0, t1),
                encode_ms: ms(t1, t2),
            })
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            // Only what exists: `new` may have stopped partway (PR #24 review).
            if !self.encoder.is_null() {
                if !self.input.is_null()
                    && let Some(f) = self.api.nvEncDestroyInputBuffer
                {
                    let _ = f(self.encoder, self.input);
                }
                if !self.output.is_null()
                    && let Some(f) = self.api.nvEncDestroyBitstreamBuffer
                {
                    let _ = f(self.encoder, self.output);
                }
                if let Some(f) = self.api.nvEncDestroyEncoder {
                    let _ = f(self.encoder);
                }
            }
            if let Ok(destroy) = self
                ._cuda
                .get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"cuCtxDestroy_v2\0")
            {
                let _ = destroy(self.cuda_ctx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sys::*;

    /// The vendored Windows bindings are used on Linux too (decision 0043): the two structs whose
    /// fields were Windows `long` types must keep the 32-bit layout of NVIDIA's header on both.
    #[test]
    fn layouts_match_the_header_on_every_os() {
        assert_eq!(std::mem::size_of::<GUID>(), 16);
        assert_eq!(std::mem::size_of::<NVENC_RECT>(), 16);
        assert_eq!(std::mem::align_of::<GUID>(), 4);
    }
}
