plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

val coreOutput = providers.environmentVariable("HUSHPEN_MOBILE_CORE")
    .orElse(providers.environmentVariable("CARGO_TARGET_DIR").map { "$it/mobile" })
    .orElse("/Volumes/External1TB/data/_custom/droid/hushpen/target/macos-arm64/mobile")
val signingPath = providers.environmentVariable("HUSHPEN_ANDROID_KEYSTORE")
val bundledLicenses = tasks.register<Copy>("bundleLicenses") {
    from(rootProject.file("../../assets/THIRD-PARTY-NOTICES.html"))
    from(rootProject.file("../../LICENSE"))
    into(layout.buildDirectory.dir("generated/licenses"))
}

android {
    namespace = "com.mandalsuraj.hushpen"
    compileSdk = 36
    defaultConfig {
        applicationId = "com.mandalsuraj.hushpen"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = providers.environmentVariable("HUSHPEN_BUILD_VERSION").orElse("2.5.0").get()
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    signingConfigs {
        getByName("debug") {
            storeFile = rootProject.layout.buildDirectory.file("signing/debug.keystore").get().asFile
        }
        if (signingPath.isPresent) {
            create("release") {
                storeFile = file(signingPath.get())
                storePassword = providers.environmentVariable("HUSHPEN_ANDROID_STORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("HUSHPEN_ANDROID_KEY_ALIAS").orElse("hushpen").get()
                keyPassword = providers.environmentVariable("HUSHPEN_ANDROID_KEY_PASSWORD").get()
            }
        }
    }
    buildTypes {
        release {
            if (signingPath.isPresent) signingConfig = signingConfigs.getByName("release")
        }
    }
    sourceSets.getByName("main") {
        java.srcDir("${coreOutput.get()}/bindings/kotlin")
        jniLibs.srcDir("${coreOutput.get()}/android/jniLibs")
        assets.srcDir(bundledLicenses.map { it.destinationDir })
    }
    buildFeatures { compose = true; buildConfig = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    packaging { resources.excludes += listOf("META-INF/AL2.0", "META-INF/LGPL2.1") }
    testOptions { unitTests.isReturnDefaultValues = true }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

tasks.configureEach {
    if ((name.startsWith("merge") && name.endsWith("Assets")) || name.contains("lint", ignoreCase = true)) dependsOn(bundledLicenses)
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.service)
    implementation(libs.coroutines.android)
    implementation(libs.jna) { artifact { type = "aar"; extension = "aar" } }
    implementation(libs.camera.core)
    implementation(libs.camera.camera2)
    implementation(libs.camera.lifecycle)
    implementation(libs.camera.view)
    implementation(libs.mlkit.barcode)
    implementation(libs.mlkit.prompt)
    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.rules)
    androidTestImplementation(libs.androidx.test.junit)
    androidTestImplementation(libs.espresso.core)
    debugImplementation(libs.compose.ui.test.manifest)
}
