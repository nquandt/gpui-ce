import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var displayLink: CADisplayLink?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        gpui_ios_initialize()
        gpui_ios_counter_main()
        gpui_ios_did_finish_launching(nil)

        let link = CADisplayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        displayLink = link

        return true
    }

    @objc func tick() {
        gpui_ios_request_frame(gpui_ios_get_window())
    }

    func applicationWillEnterForeground(_ application: UIApplication) {
        gpui_ios_will_enter_foreground(nil)
    }

    func applicationDidBecomeActive(_ application: UIApplication) {
        gpui_ios_did_become_active(nil)
    }

    func applicationWillResignActive(_ application: UIApplication) {
        gpui_ios_will_resign_active(nil)
    }

    func applicationDidEnterBackground(_ application: UIApplication) {
        gpui_ios_did_enter_background(nil)
    }

    func applicationWillTerminate(_ application: UIApplication) {
        gpui_ios_will_terminate(nil)
    }
}
