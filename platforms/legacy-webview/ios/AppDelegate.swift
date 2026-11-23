import UIKit
@main final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        let window=UIWindow(frame:UIScreen.main.bounds);window.rootViewController=OmenViewController();window.makeKeyAndVisible();self.window=window;return true
    }
    func applicationDidEnterBackground(_ application: UIApplication) {(window?.rootViewController as? OmenViewController)?.setForeground(false)}
    func applicationDidBecomeActive(_ application: UIApplication) {(window?.rootViewController as? OmenViewController)?.setForeground(true)}
    func applicationWillTerminate(_ application: UIApplication) {(window?.rootViewController as? OmenViewController)?.close()}
}
