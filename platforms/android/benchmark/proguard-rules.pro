# androidx.benchmark:benchmark-macro-junit4 and its test-only transitive deps
# reference these optional classes conditionally (profile installer, App
# Startup, error-prone annotations). None of them are on this module's
# runtime classpath and none are needed to run macrobenchmarks, so R8 only
# needs to stop treating their absence as an error.
-dontwarn androidx.profileinstaller.ProfileInstallReceiver
-dontwarn androidx.startup.Initializer
-dontwarn com.google.errorprone.annotations.MustBeClosed
