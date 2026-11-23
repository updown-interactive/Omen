import XCTest
final class OmenAppUITests:XCTestCase {
    @MainActor func testWasmRenderingTouchPluginAndTextInput() throws {
        continueAfterFailure=false
        let app=XCUIApplication();app.launch()
        let web=app.webViews.firstMatch
        XCTAssertTrue(web.staticTexts["Count: 0"].waitForExistence(timeout:20))
        let increment=web.buttons["Increment"];increment.tap()
        XCTAssertTrue(web.staticTexts["Count: 1"].waitForExistence(timeout:10))
        let field=web.textFields["Your name"];field.tap();field.typeText("iOS")
        XCTAssertTrue(web.staticTexts["Hello, iOS"].waitForExistence(timeout:10))
        increment.tap()
        let native=web.buttons["Native file round trip"]
        if !native.isHittable {web.swipeUp()};native.tap()
        XCTAssertTrue(web.staticTexts["Native file: Hello from Rust"].waitForExistence(timeout:20))
        let dialog=web.buttons["Open dialog"];if !dialog.isHittable{web.swipeUp()};dialog.tap()
        XCTAssertTrue(web.staticTexts["A cross-platform dialog"].waitForExistence(timeout:10))
        let screenshot=XCTAttachment(screenshot:app.screenshot());screenshot.lifetime = .keepAlways;add(screenshot)
    }
}
