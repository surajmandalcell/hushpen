plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.compose) apply false
}

val output = providers.environmentVariable("HUSHPEN_ANDROID_BUILD_DIR")
    .orElse(providers.environmentVariable("AGENT_DATA_DIR").map { "$it/hushpen/android-build" })
    .orElse("/Volumes/External1TB/data/_custom/droid/hushpen/android-build")
layout.buildDirectory.set(file(output.get()))
subprojects {
    layout.buildDirectory.set(rootProject.layout.buildDirectory.dir(name))
}
