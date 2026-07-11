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
        versionCode = 1
        versionName = "0.1.0"
    }

    buildFeatures {
        prefab = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("debug")
            isDebuggable = false
            isMinifyEnabled = false
        }
    }

    lint {
        disable += "ExpiredTargetSdkVersion"
    }

    sourceSets["main"].jniLibs.srcDirs("../jniLibs")
    sourceSets["main"].assets.srcDir(rootProject.file("../generated-assets/first-party-stage"))
}

dependencies {
    implementation("org.khronos.openxr:openxr_loader_for_android:1.1.57")
}
