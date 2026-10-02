# Android Environment Setup

This project requires a functional Android toolchain to build and run the thin client. The current host environment lacks these tools. 

Please install the following to proceed:

## 1. Java Development Kit (JDK)
Android Gradle Plugin (AGP) version `8.1.1` requires **JDK 17**.
- Install OpenJDK 17 or Oracle JDK 17.
- Ensure `JAVA_HOME` environment variable points to the JDK installation directory.
- Verify: `java -version` and `javac -version` should return `17.x.x`.

## 2. Android SDK Command Line Tools
If Android Studio is not installed, install the standalone [Android command-line tools](https://developer.android.com/studio#command-tools).

Set the `ANDROID_HOME` (or `ANDROID_SDK_ROOT`) environment variable to point to your SDK directory.

## 3. Required SDK Packages
The project configuration specifies:
- `compileSdk = 34`
- `targetSdk = 34`
- `minSdk = 26`

Use `sdkmanager` to install exactly what is needed:
```bash
sdkmanager "platform-tools" "platforms;android-34" "build-tools;34.0.0"
```

## 4. Local Properties
Once the SDK is installed, create `android/local.properties` (do not commit this file) with the following line:
```properties
sdk.dir=C:\\Path\\To\\Your\\Android\\sdk
```

## 5. Gradle
You do not need to install Gradle globally if you use the Gradle wrapper. Once Java and the Android SDK are configured, execute the build via:
```bash
./gradlew assembleDebug
```
