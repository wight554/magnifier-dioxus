use ndk_sys::*;
use std::ffi::{c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

macro_rules! ck {
    ($name:expr, $status:expr) => {{
        let status = $status;
        if status != camera_status_t::ACAMERA_OK {
            anyhow::bail!("{} failed: {}", $name, status.0);
        }
    }};
}

#[derive(Debug, Clone, Copy)]
pub struct CamInfo {
    pub max_zoom: f32,
    pub has_torch: bool,
    pub has_macro: bool,
    pub active_w: i32,
    pub active_h: i32,
    pub preview_w: i32,
    pub preview_h: i32,
}

pub struct Cam2 {
    manager: *mut ACameraManager,
    device: *mut ACameraDevice,
    session: *mut ACameraCaptureSession,
    request: *mut ACaptureRequest,
    output_container: *mut ACaptureSessionOutputContainer,
    session_output: *mut ACaptureSessionOutput,
    output_target: *mut ACameraOutputTarget,
    info: CamInfo,
    disconnected: *const AtomicBool,
}
unsafe impl Send for Cam2 {}

unsafe extern "C" fn on_disconnected(context: *mut c_void, _device: *mut ACameraDevice) {
    let flag = unsafe { &*(context as *const AtomicBool) };
    flag.store(true, Ordering::SeqCst);
}

unsafe extern "C" fn on_error(context: *mut c_void, _device: *mut ACameraDevice, _error: i32) {
    let flag = unsafe { &*(context as *const AtomicBool) };
    flag.store(true, Ordering::SeqCst);
}

impl Cam2 {
    pub fn open_back_camera(want_macro: bool) -> anyhow::Result<Cam2> {
        unsafe {
            let manager = ACameraManager_create();
            anyhow::ensure!(!manager.is_null(), "ACameraManager_create returned null");

            let mut id_list: *mut ACameraIdList = std::ptr::null_mut();
            ck!(
                "ACameraManager_getCameraIdList",
                ACameraManager_getCameraIdList(manager, &mut id_list)
            );
            let ids = &*id_list;

            let mut candidates: Vec<(CString, *mut ACameraMetadata, bool)> = Vec::new();

            for i in 0..ids.numCameras {
                let id_ptr = *ids.cameraIds.offset(i as isize);
                let mut metadata: *mut ACameraMetadata = std::ptr::null_mut();
                ck!(
                    "ACameraManager_getCameraCharacteristics",
                    ACameraManager_getCameraCharacteristics(manager, id_ptr, &mut metadata)
                );

                let mut entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
                ck!(
                    "ACameraMetadata_getConstEntry(LENS_FACING)",
                    ACameraMetadata_getConstEntry(
                        metadata,
                        acamera_metadata_tag::ACAMERA_LENS_FACING.0,
                        &mut entry
                    )
                );
                let facing = *entry.data.u8_;

                if facing == acamera_metadata_enum_acamera_lens_facing::ACAMERA_LENS_FACING_BACK.0 as u8 {
                    let is_macro = Self::is_macro_lens(metadata);
                    let id = CStr::from_ptr(id_ptr).to_owned();
                    log::info!("magnifier: back camera {id:?} is_macro={is_macro}");
                    candidates.push((id, metadata, is_macro));
                } else {
                    ACameraMetadata_free(metadata);
                }
            }

            ACameraManager_deleteCameraIdList(id_list);

            anyhow::ensure!(!candidates.is_empty(), "no back-facing camera found");

            let has_macro = candidates.iter().any(|(_, _, is_macro)| *is_macro);
            let chosen_index = candidates
                .iter()
                .position(|(_, _, is_macro)| *is_macro == want_macro)
                .unwrap_or(0);

            let (id, metadata, _) = candidates.remove(chosen_index);
            for (_, leftover_metadata, _) in candidates {
                ACameraMetadata_free(leftover_metadata);
            }

            let mut info = Self::read_characteristics(metadata)?;
            info.has_macro = has_macro;

            let disconnected = Arc::new(AtomicBool::new(false));
            let ctx = Arc::into_raw(disconnected.clone()) as *mut c_void;

            let mut callbacks = ACameraDevice_StateCallbacks {
                context: ctx,
                onDisconnected: Some(on_disconnected),
                onError: Some(on_error),
            };

            let mut device: *mut ACameraDevice = std::ptr::null_mut();
            let open_status =
                ACameraManager_openCamera(manager, id.as_ptr(), &mut callbacks, &mut device);
            if open_status != camera_status_t::ACAMERA_OK {
                // reclaim the leaked Arc ref before bailing
                drop(Arc::from_raw(ctx as *const AtomicBool));
                ACameraMetadata_free(metadata);
                ACameraManager_delete(manager);
                anyhow::bail!("ACameraManager_openCamera failed: {}", open_status.0);
            }

            ACameraMetadata_free(metadata);

            Ok(Cam2 {
                manager,
                device,
                session: std::ptr::null_mut(),
                request: std::ptr::null_mut(),
                output_container: std::ptr::null_mut(),
                session_output: std::ptr::null_mut(),
                output_target: std::ptr::null_mut(),
                info,
                disconnected: Arc::into_raw(disconnected),
            })
        }
    }

    unsafe fn is_macro_lens(metadata: *const ACameraMetadata) -> bool {
        unsafe {
            let mut focal_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            let focal_status = ACameraMetadata_getConstEntry(
                metadata,
                acamera_metadata_tag::ACAMERA_LENS_INFO_AVAILABLE_FOCAL_LENGTHS.0,
                &mut focal_entry,
            );
            let min_focal = if focal_status == camera_status_t::ACAMERA_OK && focal_entry.count > 0 {
                std::slice::from_raw_parts(focal_entry.data.f, focal_entry.count as usize)
                    .iter()
                    .cloned()
                    .fold(f32::MAX, f32::min)
            } else {
                f32::MAX
            };

            let mut dist_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            let dist_status = ACameraMetadata_getConstEntry(
                metadata,
                acamera_metadata_tag::ACAMERA_LENS_INFO_MINIMUM_FOCUS_DISTANCE.0,
                &mut dist_entry,
            );
            let min_focus_distance = if dist_status == camera_status_t::ACAMERA_OK {
                *dist_entry.data.f
            } else {
                0.0
            };

            log::info!(
                "magnifier: lens characteristics focal_length={min_focal}mm min_focus_distance={min_focus_distance}diopters"
            );

            crate::camera::macro_lens::is_macro(min_focal, min_focus_distance)
        }
    }

    unsafe fn read_characteristics(metadata: *const ACameraMetadata) -> anyhow::Result<CamInfo> {
        unsafe {
            let mut zoom_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            let zoom_status = ACameraMetadata_getConstEntry(
                metadata,
                acamera_metadata_tag::ACAMERA_SCALER_AVAILABLE_MAX_DIGITAL_ZOOM.0,
                &mut zoom_entry,
            );
            let max_zoom = if zoom_status == camera_status_t::ACAMERA_OK {
                *zoom_entry.data.f
            } else {
                1.0
            };

            let mut flash_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            ck!(
                "ACameraMetadata_getConstEntry(FLASH_INFO_AVAILABLE)",
                ACameraMetadata_getConstEntry(
                    metadata,
                    acamera_metadata_tag::ACAMERA_FLASH_INFO_AVAILABLE.0,
                    &mut flash_entry
                )
            );
            let has_torch = *flash_entry.data.u8_
                == acamera_metadata_enum_acamera_flash_info_available::ACAMERA_FLASH_INFO_AVAILABLE_TRUE.0
                    as u8;

            let mut array_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            ck!(
                "ACameraMetadata_getConstEntry(SENSOR_INFO_ACTIVE_ARRAY_SIZE)",
                ACameraMetadata_getConstEntry(
                    metadata,
                    acamera_metadata_tag::ACAMERA_SENSOR_INFO_ACTIVE_ARRAY_SIZE.0,
                    &mut array_entry
                )
            );
            anyhow::ensure!(array_entry.count == 4, "unexpected active array size entry count");
            let array = std::slice::from_raw_parts(array_entry.data.i32_, 4);
            let active_w = array[2];
            let active_h = array[3];

            let mut stream_entry = std::mem::zeroed::<ACameraMetadata_const_entry>();
            ck!(
                "ACameraMetadata_getConstEntry(SCALER_AVAILABLE_STREAM_CONFIGURATIONS)",
                ACameraMetadata_getConstEntry(
                    metadata,
                    acamera_metadata_tag::ACAMERA_SCALER_AVAILABLE_STREAM_CONFIGURATIONS.0,
                    &mut stream_entry
                )
            );
            let configs =
                std::slice::from_raw_parts(stream_entry.data.i32_, stream_entry.count as usize);
            const AIMAGE_FORMAT_PRIVATE: i32 = 34;
            const INPUT: i32 = 1;
            let mut best: Option<(i32, i32)> = None;
            for chunk in configs.chunks_exact(4) {
                let [format, width, height, is_input] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                if format != AIMAGE_FORMAT_PRIVATE || is_input == INPUT {
                    continue;
                }
                if width <= 1920 && height <= 1080 {
                    let better = match best {
                        Some((bw, bh)) => width * height > bw * bh,
                        None => true,
                    };
                    if better {
                        best = Some((width, height));
                    }
                }
            }
            let (preview_w, preview_h) = best.unwrap_or((1280, 720));

            Ok(CamInfo {
                max_zoom,
                has_torch,
                has_macro: false,
                active_w,
                active_h,
                preview_w,
                preview_h,
            })
        }
    }

    pub fn characteristics(&self) -> CamInfo {
        self.info
    }

    pub fn is_disconnected(&self) -> bool {
        unsafe { (*self.disconnected).load(Ordering::SeqCst) }
    }

    pub fn start_preview(&mut self, window: *mut ANativeWindow) -> anyhow::Result<()> {
        unsafe {
            let mut container: *mut ACaptureSessionOutputContainer = std::ptr::null_mut();
            ck!(
                "ACaptureSessionOutputContainer_create",
                ACaptureSessionOutputContainer_create(&mut container)
            );

            let mut output: *mut ACaptureSessionOutput = std::ptr::null_mut();
            ck!(
                "ACaptureSessionOutput_create",
                ACaptureSessionOutput_create(window.cast(), &mut output)
            );
            ck!(
                "ACaptureSessionOutputContainer_add",
                ACaptureSessionOutputContainer_add(container, output)
            );

            let mut session_callbacks = ACameraCaptureSession_stateCallbacks {
                context: std::ptr::null_mut(),
                onClosed: None,
                onReady: None,
                onActive: None,
            };
            let mut session: *mut ACameraCaptureSession = std::ptr::null_mut();
            ck!(
                "ACameraDevice_createCaptureSession",
                ACameraDevice_createCaptureSession(
                    self.device,
                    container,
                    &mut session_callbacks,
                    &mut session
                )
            );

            let mut request: *mut ACaptureRequest = std::ptr::null_mut();
            ck!(
                "ACameraDevice_createCaptureRequest",
                ACameraDevice_createCaptureRequest(
                    self.device,
                    ACameraDevice_request_template::TEMPLATE_PREVIEW,
                    &mut request
                )
            );

            let mut target: *mut ACameraOutputTarget = std::ptr::null_mut();
            ck!(
                "ACameraOutputTarget_create",
                ACameraOutputTarget_create(window.cast(), &mut target)
            );
            ck!(
                "ACaptureRequest_addTarget",
                ACaptureRequest_addTarget(request, target)
            );

            let af_mode =
                acamera_metadata_enum_acamera_control_af_mode::ACAMERA_CONTROL_AF_MODE_CONTINUOUS_PICTURE
                    .0 as u8;
            ck!(
                "ACaptureRequest_setEntry_u8(AF_MODE)",
                ACaptureRequest_setEntry_u8(
                    request,
                    acamera_metadata_tag::ACAMERA_CONTROL_AF_MODE.0,
                    1,
                    &af_mode
                )
            );

            let mut seq_id: i32 = 0;
            let mut requests = [request];
            ck!(
                "ACameraCaptureSession_setRepeatingRequest",
                ACameraCaptureSession_setRepeatingRequest(
                    session,
                    std::ptr::null_mut(),
                    1,
                    requests.as_mut_ptr(),
                    &mut seq_id
                )
            );

            self.output_container = container;
            self.session_output = output;
            self.session = session;
            self.request = request;
            self.output_target = target;
            Ok(())
        }
    }

    pub fn apply(&mut self, crop: (i32, i32, i32, i32), torch: bool) -> anyhow::Result<()> {
        anyhow::ensure!(!self.request.is_null(), "apply called before start_preview");
        unsafe {
            let crop_arr = [crop.0, crop.1, crop.2, crop.3];
            ck!(
                "ACaptureRequest_setEntry_i32(CROP_REGION)",
                ACaptureRequest_setEntry_i32(
                    self.request,
                    acamera_metadata_tag::ACAMERA_SCALER_CROP_REGION.0,
                    4,
                    crop_arr.as_ptr()
                )
            );

            let mode = if torch {
                acamera_metadata_enum_acamera_flash_mode::ACAMERA_FLASH_MODE_TORCH.0
            } else {
                acamera_metadata_enum_acamera_flash_mode::ACAMERA_FLASH_MODE_OFF.0
            } as u8;
            ck!(
                "ACaptureRequest_setEntry_u8(FLASH_MODE)",
                ACaptureRequest_setEntry_u8(
                    self.request,
                    acamera_metadata_tag::ACAMERA_FLASH_MODE.0,
                    1,
                    &mode
                )
            );

            let mut seq_id: i32 = 0;
            let mut requests = [self.request];
            ck!(
                "ACameraCaptureSession_setRepeatingRequest",
                ACameraCaptureSession_setRepeatingRequest(
                    self.session,
                    std::ptr::null_mut(),
                    1,
                    requests.as_mut_ptr(),
                    &mut seq_id
                )
            );
        }
        Ok(())
    }

    pub fn stop_repeating(&mut self) {
        if !self.session.is_null() {
            unsafe {
                ACameraCaptureSession_stopRepeating(self.session);
            }
        }
    }

    pub fn resume_repeating(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.session.is_null(), "resume_repeating before start_preview");
        unsafe {
            let mut seq_id: i32 = 0;
            let mut requests = [self.request];
            ck!(
                "ACameraCaptureSession_setRepeatingRequest",
                ACameraCaptureSession_setRepeatingRequest(
                    self.session,
                    std::ptr::null_mut(),
                    1,
                    requests.as_mut_ptr(),
                    &mut seq_id
                )
            );
        }
        Ok(())
    }
}

impl Drop for Cam2 {
    fn drop(&mut self) {
        unsafe {
            if !self.session.is_null() {
                ACameraCaptureSession_close(self.session);
            }
            if !self.request.is_null() {
                ACaptureRequest_free(self.request);
            }
            if !self.output_target.is_null() {
                ACameraOutputTarget_free(self.output_target);
            }
            if !self.session_output.is_null() {
                ACaptureSessionOutput_free(self.session_output);
            }
            if !self.output_container.is_null() {
                ACaptureSessionOutputContainer_free(self.output_container);
            }
            if !self.device.is_null() {
                ACameraDevice_close(self.device);
            }
            if !self.manager.is_null() {
                ACameraManager_delete(self.manager);
            }
            drop(Arc::from_raw(self.disconnected));
        }
    }
}
