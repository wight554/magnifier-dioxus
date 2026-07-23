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

