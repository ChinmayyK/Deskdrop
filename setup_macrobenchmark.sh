#!/bin/bash

# Ensure we're in the Android platform directory
cd platforms/android || { echo "Run this script from the root Deskdrop directory."; exit 1; }

echo "Setting up Macrobenchmark module..."

# 1. Modify ActivityFeedManager.kt to remove the 100 item limit
sed -i '' 's/const val ACTIVITY_FEED_MAX = 100/var ACTIVITY_FEED_MAX = 100/g' app/src/main/java/com/deskdrop/ActivityFeedManager.kt

# 2. Modify MainActivity.kt to inject 10,000 items if the benchmark intent extra is true
# We use awk to insert the snippet right after super.onCreate(savedInstanceState)
awk '/super\.onCreate\(savedInstanceState\)/ {
    print $0
    print "        if (intent?.getBooleanExtra(\"benchmark\", false) == true) {"
    print "            ActivityFeedManager.ACTIVITY_FEED_MAX = 20000"
    print "            for (i in 1..10000) {"
    print "                ActivityFeedManager.addToFeed(ActivityEntry(id = i.toLong(), deviceName = \"TestDevice\", kind = ActivityKind.CLIPBOARD_TEXT, preview = \"Test item $i\", contentHash = \"hash$i\"))"
    print "            }"
    print "        }"
    next
}1' app/src/main/java/com/deskdrop/MainActivity.kt > tmp.kt && mv tmp.kt app/src/main/java/com/deskdrop/MainActivity.kt


# 3. Create benchmark module directory structure
mkdir -p benchmark/src/main/java/com/deskdrop/benchmark
mkdir -p benchmark/src/main/res

# 4. Create benchmark/build.gradle
cat << 'EOF' > benchmark/build.gradle
plugins {
    id 'com.android.test'
    id 'org.jetbrains.kotlin.android'
}

android {
    namespace 'com.deskdrop.benchmark'
    compileSdk 34

    defaultConfig {
        minSdk 28
        targetSdk 34
        testInstrumentationRunner "androidx.test.runner.AndroidJUnitRunner"
    }

    targetProjectPath ':app'
    
    buildTypes {
        release {
            debuggable true
            signingConfig signingConfigs.debug
        }
    }
}

dependencies {
    implementation 'androidx.test.ext:junit:1.1.5'
    implementation 'androidx.test.espresso:espresso-core:3.5.1'
    implementation 'androidx.test.uiautomator:uiautomator:2.2.0'
    implementation 'androidx.benchmark:benchmark-macro-junit4:1.2.3'
}
EOF

# 5. Create benchmark/src/main/AndroidManifest.xml
cat << 'EOF' > benchmark/src/main/AndroidManifest.xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.deskdrop.benchmark">
    
    <!-- Required to query the app package for the benchmark -->
    <queries>
        <package android:name="com.deskdrop" />
    </queries>
</manifest>
EOF

# 6. Create benchmark test class
cat << 'EOF' > benchmark/src/main/java/com/deskdrop/benchmark/FeedScrollBenchmark.kt
package com.deskdrop.benchmark

import android.content.Intent
import androidx.benchmark.macro.CompilationMode
import androidx.benchmark.macro.FrameTimingMetric
import androidx.benchmark.macro.StartupMode
import androidx.benchmark.macro.junit4.MacrobenchmarkRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.uiautomator.By
import androidx.test.uiautomator.Direction
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class FeedScrollBenchmark {

    @get:Rule
    val benchmarkRule = MacrobenchmarkRule()

    @Test
    fun scrollFeed() {
        benchmarkRule.measureRepeated(
            packageName = "com.deskdrop",
            metrics = listOf(FrameTimingMetric()),
            compilationMode = CompilationMode.DEFAULT,
            startupMode = StartupMode.COLD,
            iterations = 5,
            setupBlock = {
                pressHome()
            }
        ) {
            val intent = Intent("android.intent.action.MAIN")
            intent.setPackage("com.deskdrop")
            intent.setClassName("com.deskdrop", "com.deskdrop.MainActivity")
            intent.putExtra("benchmark", true)
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            
            startActivityAndWait(intent)

            // Find the list by its scrollable state and scroll it
            val list = device.findObject(By.scrollable(true))
            if (list != null) {
                list.setGestureMargin(device.displayWidth / 5)
                list.scroll(Direction.DOWN, 1f)
                list.scroll(Direction.UP, 1f)
            } else {
                throw AssertionError("Could not find a scrollable list on screen")
            }
        }
    }
}
EOF

# 7. Add :benchmark to settings.gradle if not already there
if ! grep -q "include ':benchmark'" settings.gradle; then
    echo "include ':benchmark'" >> settings.gradle
fi

echo "Macrobenchmark module created successfully!"
echo "To run the benchmark, connect a device and run:"
echo "cd platforms/android && ./gradlew :benchmark:connectedCheck"
