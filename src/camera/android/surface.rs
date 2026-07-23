use jni::objects::{GlobalRef, JValue};
use std::sync::mpsc;

pub struct SurfaceHandle {
    _view_ref: GlobalRef,
    _surface_ref: GlobalRef,
    native_window: *mut ndk_sys::ANativeWindow,
}
unsafe impl Send for SurfaceHandle {}

const ANDROID_R_ID_CONTENT: i32 = 0x0102_0002;

pub fn create_surface_view(width: i32, height: i32) -> anyhow::Result<SurfaceHandle> {
    let (tx, rx) = mpsc::channel();

    wry::prelude::dispatch(move |env, activity, _webview| {
        let result = (|| -> jni::errors::Result<(jni::objects::GlobalRef, jni::objects::GlobalRef)> {
            let sv = env.new_object(
                "android/view/SurfaceView",
                "(Landroid/content/Context;)V",
                &[JValue::Object(activity)],
            )?;
            let holder = env
                .call_method(&sv, "getHolder", "()Landroid/view/SurfaceHolder;", &[])?
                .l()?;
            env.call_method(
                &holder,
                "setFixedSize",
                "(II)V",
                &[JValue::Int(width), JValue::Int(height)],
            )?;
            let content = env
                .call_method(
                    activity,
                    "findViewById",
                    "(I)Landroid/view/View;",
                    &[JValue::Int(ANDROID_R_ID_CONTENT)],
                )?
                .l()?;
            // index 0 draws first -> below the webview that setContentView already installed
            env.call_method(
                &content,
                "addView",
                "(Landroid/view/View;I)V",
                &[JValue::Object(&sv), JValue::Int(0)],
            )?;
            Ok((env.new_global_ref(&sv)?, env.new_global_ref(&holder)?))
        })();
        let _ = tx.send(result);
    });

    let (view_ref, holder_ref) = rx.recv_timeout(std::time::Duration::from_secs(5))??;

    // No SurfaceHolder.Callback is possible from pure JNI (can't implement a Java
    // interface without a real class), so poll from this thread until valid instead.
    for _ in 0..100 {
        let valid = super::jni_glue::with_jni(|env, _| {
            let surface = env
                .call_method(
                    holder_ref.as_obj(),
                    "getSurface",
                    "()Landroid/view/Surface;",
                    &[],
                )?
                .l()?;
            if surface.is_null() {
                return Ok(None);
            }
            let is_valid = env.call_method(&surface, "isValid", "()Z", &[])?.z()?;
            if is_valid {
                Ok(Some(env.new_global_ref(&surface)?))
            } else {
                Ok(None)
            }
        })?;
        if let Some(surface_ref) = valid {
            let native_window = super::jni_glue::with_jni(|env, _| unsafe {
                Ok(ndk_sys::ANativeWindow_fromSurface(
                    env.get_native_interface().cast(),
                    surface_ref.as_obj().as_raw().cast(),
                ))
            })?;
            anyhow::ensure!(
                !native_window.is_null(),
                "ANativeWindow_fromSurface returned null"
            );
            return Ok(SurfaceHandle {
                _view_ref: view_ref,
                _surface_ref: surface_ref,
                native_window,
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    anyhow::bail!("surface never became valid")
}

impl SurfaceHandle {
    pub fn native_window(&self) -> *mut ndk_sys::ANativeWindow {
        self.native_window
    }
}

impl Drop for SurfaceHandle {
    fn drop(&mut self) {
        unsafe { ndk_sys::ANativeWindow_release(self.native_window) };
    }
}
