pluginManagement { plugins { id("com.android.library") version "9.1.0" }; resolutionStrategy { eachPlugin { if (requested.id.id == "com.android.library") useModule("com.android.tools.build:gradle:${requested.version}") } }; repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement { repositories { google(); mavenCentral() } }
rootProject.name = "omen-native-android"
