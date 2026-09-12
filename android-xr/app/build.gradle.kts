plugins {
    id("com.android.application")
}

android {
    namespace = "com.kzahel.mclone.xr"
    compileSdk = 35
    ndkVersion = "27.0.12077973"

    defaultConfig {
        applicationId = "com.kzahel.mclone.xr"
        minSdk = 28
        targetSdk = 32
        ndk { abiFilters += "arm64-v8a" }
        versionCode = System.getenv("MCLONE_ANDROID_VERSION_CODE")?.toInt() ?: 1
        versionName = System.getenv("MCLONE_ANDROID_VERSION_NAME") ?: "0.1.0-dev"
    }

    buildFeatures {
        prefab = true
    }

    signingConfigs {
        if (System.getenv("MCLONE_ANDROID_KEYSTORE") != null) {
            create("nightly") {
                storeFile = file(System.getenv("MCLONE_ANDROID_KEYSTORE"))
                storePassword = System.getenv("MCLONE_ANDROID_STORE_PASSWORD")
                keyAlias = "mclone-nightly"
                keyPassword = System.getenv("MCLONE_ANDROID_STORE_PASSWORD")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.findByName("nightly") ?: signingConfigs.getByName("debug")
            isDebuggable = false
            isMinifyEnabled = false
        }
    }

    lint {
        disable += "ExpiredTargetSdkVersion"
    }

    sourceSets["main"].jniLibs.srcDirs("../jniLibs")
    sourceSets["main"].assets.srcDir(rootProject.file("../generated-assets/first-party-stage"))
    sourceSets["main"].java.srcDir(rootProject.file("../android-common/src/main/java"))
}

dependencies {
    implementation("org.khronos.openxr:openxr_loader_for_android:1.1.57")
}
