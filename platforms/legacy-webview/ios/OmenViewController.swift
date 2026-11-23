import UIKit
import WebKit
import OmenNative

private final class BundleAssets:NSObject,WKURLSchemeHandler {
    let root:URL
    init(root:URL){self.root=root.standardizedFileURL}
    func webView(_ webView:WKWebView,start task:WKURLSchemeTask) {
        guard let url=task.request.url,url.scheme=="omen",url.host=="app" else {task.didFailWithError(URLError(.unsupportedURL));return}
        let path=url.path=="/" ? "index.html" : String(url.path.dropFirst())
        let file=root.appendingPathComponent(path).standardizedFileURL
        guard file.path.hasPrefix(root.path+"/"),let data=try? Data(contentsOf:file,options:.mappedIfSafe) else {task.didFailWithError(URLError(.fileDoesNotExist));return}
        let types=["html":"text/html","mjs":"text/javascript","js":"text/javascript","css":"text/css","json":"application/json","wasm":"application/wasm","svg":"image/svg+xml","png":"image/png","jpg":"image/jpeg"]
        task.didReceive(URLResponse(url:url,mimeType:types[file.pathExtension] ?? "application/octet-stream",expectedContentLength:data.count,textEncodingName:["html","mjs","js","css","json","svg"].contains(file.pathExtension) ? "UTF-8":nil));task.didReceive(data);task.didFinish()
    }
    func webView(_ webView:WKWebView,stop task:WKURLSchemeTask){}
}
private final class WeakMessages:NSObject,WKScriptMessageHandler {
    weak var owner:OmenViewController?
    init(_ owner:OmenViewController){self.owner=owner}
    func userContentController(_ controller:WKUserContentController,didReceive message:WKScriptMessage){owner?.receive(message)}
}
/// Native UIKit/WKWebView host. Rust state lives in the packaged WASM instance.
final class OmenViewController:UIViewController,WKNavigationDelegate {
    private var web:WKWebView!
    private var native:OmenNativeBridge?
    private var observers:[NSObjectProtocol]=[]
    private var nativeFrame: CGRect?
    private var generation=0
    private var ready=false
    private var foreground=true
    private var queued:[String]=[]
    override func viewDidLoad(){
        super.viewDidLoad();view.backgroundColor=UIColor(red:16/255,green:24/255,blue:39/255,alpha:1)
        guard let root=Bundle.main.resourceURL?.appendingPathComponent("Assets") else {return}
        let config=(try? Data(contentsOf:root.appendingPathComponent("app.json"))).flatMap{try? JSONSerialization.jsonObject(with:$0) as? [String:Any]}
        let allowed=Set(config?["permissions"] as? [String] ?? [])
        let configuration=WKWebViewConfiguration();configuration.setURLSchemeHandler(BundleAssets(root:root),forURLScheme:"omen")
        configuration.userContentController.add(WeakMessages(self),name:"omenNative")
        configuration.defaultWebpagePreferences.allowsContentJavaScript=true
        web=WKWebView(frame:.zero,configuration:configuration);web.navigationDelegate=self;web.isOpaque=false;web.backgroundColor=view.backgroundColor
        web.scrollView.contentInsetAdjustmentBehavior = .never;web.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(web)
        NSLayoutConstraint.activate([web.topAnchor.constraint(equalTo:view.topAnchor),web.bottomAnchor.constraint(equalTo:view.bottomAnchor),web.leadingAnchor.constraint(equalTo:view.leadingAnchor),web.trailingAnchor.constraint(equalTo:view.trailingAnchor)])
        let documents=FileManager.default.urls(for:.documentDirectory,in:.userDomainMask)[0]
        native=try? OmenNativeBridge(presenter:self,allowlist:allowed,root:documents,responseSink:{[weak self] data in self?.deliverNative(data)})
        observers.append(NotificationCenter.default.addObserver(forName:UIResponder.keyboardWillChangeFrameNotification,object:nil,queue:.main){[weak self] notification in
            self?.nativeFrame=(notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue;self?.metrics()
        })
        web.load(URLRequest(url:URL(string:"omen://app/index.html")!))
    }
    override func viewSafeAreaInsetsDidChange(){super.viewSafeAreaInsetsDidChange();metrics()}
    override func viewDidLayoutSubviews(){super.viewDidLayoutSubviews();metrics()}
    func setForeground(_ active:Bool){foreground=active;if ready{web?.evaluateJavaScript("window.OmenHost?.lifecycle('\(active ? "foreground":"background")')");if active{flush();metrics()}}}
    private func metrics(){
        guard ready,web != nil else{return}
        let insets=view.safeAreaInsets
        let keyboard=nativeFrame.map{view.convert($0,from:nil)}.map{view.bounds.intersection($0).height} ?? 0
        let height=max(0,view.bounds.height-keyboard)
        let json:[String:Any]=["height":height,"insets":[insets.top,insets.right,keyboard>0 ? 0:insets.bottom,insets.left]]
        guard let data=try? JSONSerialization.data(withJSONObject:json),let text=String(data:data,encoding:.utf8)else{return}
        web.evaluateJavaScript("window.OmenHost?.setNativeMetrics(\(text))")
    }
    func webView(_ webView:WKWebView,didFinish navigation:WKNavigation!){ready=true;web.evaluateJavaScript("window.OmenReady?.then(()=>{window.OmenHost.lifecycle('\(foreground ? "foreground":"background")');window.webkit.messageHandlers.omenNative.postMessage({kind:'ready'});})");metrics();flush()}
    func webView(_ webView:WKWebView,decidePolicyFor action:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void){let url=action.request.url;decisionHandler(url?.scheme=="omen"&&url?.host=="app" ? .allow:.cancel)}
    func webViewWebContentProcessDidTerminate(_ webView:WKWebView){generation+=1;ready=false;native?.cancelPending();web.reload()}
    fileprivate func receive(_ message:WKScriptMessage){
        guard message.frameInfo.isMainFrame,message.frameInfo.securityOrigin.protocol=="omen",message.frameInfo.securityOrigin.host=="app",let body=message.body as? [String:Any],let kind=body["kind"] as? String else{return}
        if kind=="ready" {metrics();flush();return}
        if kind=="plugin",let frame=body["frame"],let bytes=try? JSONSerialization.data(withJSONObject:frame),bytes.count<=1_048_576{native?.dispatchHost(bytes)}
        else if kind=="permission",let id=body["id"] as? String,let permission=body["permission"] as? String,id.count<=20,id.allSatisfy({$0.isASCII&&$0.isNumber}) {
            let token=generation
            Task{let status=await native?.requestPermission(permission) ?? "denied";guard token==generation else{return};let arguments=(try? JSONSerialization.data(withJSONObject:[id,status])).flatMap{String(data:$0,encoding:.utf8)} ?? "[]";deliver("window.OmenHost?.permissionResult(...\(arguments))")}
        }
    }
    private func deliverNative(_ bytes:Data){guard let json=String(data:bytes,encoding:.utf8)else{return};deliver("window.OmenHost?.receiveNative(\(json))")}
    private func deliver(_ script:String){if ready&&foreground{web?.evaluateJavaScript(script)}else if queued.count<32{queued.append(script)}}
    private func flush(){guard ready&&foreground else{return};queued.forEach{web.evaluateJavaScript($0)};queued.removeAll()}
    func close(){ready=false;queued.removeAll();observers.forEach(NotificationCenter.default.removeObserver);observers.removeAll();web?.evaluateJavaScript("window.OmenHost?.dispose()");web?.configuration.userContentController.removeScriptMessageHandler(forName:"omenNative");native?.close();native=nil}
    deinit{observers.forEach(NotificationCenter.default.removeObserver)}
}
