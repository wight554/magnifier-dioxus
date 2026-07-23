[English](README.md)

# Лупа

Android-застосунок для читання дрібного тексту. Тап по іконці — одразу отримуєте
збільшене живе зображення з задньої камери; вмикайте ліхтарик; заморожуйте кадр,
щоб зручно тримати його нерухомо під час читання. Жодних меню на критичному шляху —
усе керується дотиком по самому накладенню.

Створено на [Dioxus](https://dioxuslabs.com) 0.7: прозорий webview рендерить
накладення керування поверх нативного `SurfaceView`, який напряму живиться NDK
Camera2 API (`ndk-sys`). Жодного Kotlin/Java, жодних Gradle-залежностей — увесь
конвеєр камери написаний на Rust.

Нотатки з дизайну та реалізації: `docs/superpowers/specs/2026-07-23-magnifier-design.md`
та `docs/superpowers/plans/2026-07-23-magnifier.md`.

## Встановлення

Найпростіший спосіб встановити застосунок і отримувати оновлення —
[Obtainium](https://github.com/ImranR98/Obtainium):

1. Встановіть Obtainium з його [сторінки релізів](https://github.com/ImranR98/Obtainium/releases) або F-Droid.
2. В Obtainium натисніть "Add App" і вставте: `https://github.com/wight554/magnifier-dioxus`
3. Obtainium автоматично знайде останній підписаний реліз APK і встановить його.
4. Майбутні оновлення з'являться в Obtainium як для будь-якого іншого застосунку.

Альтернативно, завантажте APK напряму зі [сторінки релізів](https://github.com/wight554/magnifier-dioxus/releases)
і встановіть вручну (потрібно дозволити встановлення з цього джерела в налаштуваннях Android).

## Вимоги

- Android 10 (API 29) або новіше, телефон із задньою камерою.
- Rust, `dx` CLI (`cargo install dioxus-cli`), версія якого відповідає версії `dioxus`
  у `Cargo.toml` (невідповідність версій виводить попередження, але зазвичай усе одно
  працює).
- Для збірок під Android: Android SDK (platform 29+, platform-tools) та NDK 27+, а
  також JDK. Встановіть змінні `ANDROID_HOME`/`NDK_HOME`/`JAVA_HOME` і додайте
  `$ANDROID_HOME/platform-tools` до `PATH`.
- `rustup target add aarch64-linux-android`.

## Розробка (десктоп)

Лише робота над UI — камера тут заглушка (сіра рамка, фіксовані можливості), тому
ітерація не потребує пристрою:

```sh
dx serve --desktop
```

## Android

Збірка для налагодження + встановлення на підключений пристрій:

```sh
dx serve --android          # збирає, встановлює і транслює логи
# або, щоб самостійно керувати встановленням/запуском/логами:
dx build --android
adb install -r target/dx/magnifier/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.magnifier.app/dev.dioxus.main.MainActivity
```

Реліз APK:

```sh
dx bundle --platform android --release
```

## Тестування

```sh
cargo test          # серіалізація налаштувань + математика масштабування, лише десктоп
```

Код камери/JNI не має автоматичних тестів (камери в емуляторі поводяться інакше, ніж
справжнє обладнання) — він перевіряється вручну на фізичному пристрої. `adb logcat | grep magnifier`
показує власне трасування застосунку через потік камери та процес видачі дозволів.
</content>
