use jni::objects::{JObject, JValue};
use jni::JNIEnv;

pub fn with_jni<R>(
    f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>,
) -> jni::errors::Result<R> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast())? };
    let mut guard = vm.attach_current_thread()?;
    let activity = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };
    f(&mut guard, &activity)
}

const CAMERA_PERM: &str = "android.permission.CAMERA";
const PERMISSION_GRANTED: i32 = 0;

pub fn has_camera_permission() -> bool {
    with_jni(|env, activity| {
        let perm = env.new_string(CAMERA_PERM)?;
        let res = env
            .call_method(
                activity,
                "checkSelfPermission",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&perm)],
            )?
            .i()?;
        Ok(res == PERMISSION_GRANTED)
    })
    .unwrap_or(false)
}

pub fn request_camera_permission() {
    let _ = with_jni(|env, activity| {
        let perm = env.new_string(CAMERA_PERM)?;
        let arr = env.new_object_array(1, "java/lang/String", &perm)?;
        env.call_method(
            activity,
            "requestPermissions",
            "([Ljava/lang/String;I)V",
            &[JValue::Object(&arr), JValue::Int(1)],
        )?;
        Ok(())
    });
}

pub fn should_show_rationale() -> bool {
    with_jni(|env, activity| {
        let perm = env.new_string(CAMERA_PERM)?;
        env.call_method(
            activity,
            "shouldShowRequestPermissionRationale",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&perm)],
        )?
        .z()
    })
    .unwrap_or(false)
}

static APP_FILES_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// The app's private, writable internal storage directory (`Context.getFilesDir()`).
///
/// `dirs::data_local_dir()` has no Android support and silently resolves to an
/// unwritable path there (it falls into the generic Unix branch, which relies on
/// `$HOME`, unset for Android app processes) - this is the real thing.
pub fn app_files_dir() -> std::path::PathBuf {
    APP_FILES_DIR
        .get_or_init(|| {
            with_jni(|env, activity| {
                let dir = env
                    .call_method(activity, "getFilesDir", "()Ljava/io/File;", &[])?
                    .l()?;
                let path_str = env
                    .call_method(&dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
                    .l()?;
                let jstring = jni::objects::JString::from(path_str);
                let s: String = env.get_string(&jstring)?.into();
                Ok(std::path::PathBuf::from(s))
            })
            .unwrap_or_else(|e| {
                log::error!("magnifier: getFilesDir failed: {e:?}, falling back");
                std::path::PathBuf::from("/data/local/tmp")
            })
        })
        .clone()
}

fn perm_asked_marker() -> std::path::PathBuf {
    app_files_dir().join("perm_asked")
}

pub fn mark_permission_asked() {
    let marker = perm_asked_marker();
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(marker, b"1");
}

pub fn was_permission_asked_before() -> bool {
    perm_asked_marker().exists()
}

const FLAG_KEEP_SCREEN_ON: i32 = 0x00000080;

pub fn keep_screen_on() {
    let _ = with_jni(|env, activity| {
        let window = env
            .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
            .l()?;
        env.call_method(&window, "addFlags", "(I)V", &[JValue::Int(FLAG_KEEP_SCREEN_ON)])?;
        Ok(())
    });
}
