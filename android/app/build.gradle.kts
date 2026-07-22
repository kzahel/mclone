plugins {
    id("com.android.application")
}

android {
    namespace = "com.kzahel.mclone"
    compileSdk = 35
    ndkVersion = "27.0.12077973"

    defaultConfig {
        applicationId = "com.kzahel.mclone"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets["main"].jniLibs.srcDirs("../jniLibs")
    sourceSets["main"].assets.srcDir(rootProject.file("../generated-assets/first-party-stage"))
    sourceSets["main"].java.srcDir(rootProject.file("../android-common/src/main/java"))
}
