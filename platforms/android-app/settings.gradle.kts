pluginManagement {
    resolutionStrategy { eachPlugin { if (requested.id.id.startsWith("com.android.") && requested.version != null) useModule("com.android.tools.build:gradle:${requested.version}") } }
    repositories { google(); mavenCentral(); gradlePluginPortal() }
}
dependencyResolutionManagement { repositories { google(); mavenCentral() } }
rootProject.name = "android-app"
