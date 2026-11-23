use futures::{FutureExt, StreamExt, executor::block_on};
use omen_plugin::*;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Serialize, Deserialize)]
struct Echo {
    text: String,
}
impl Operation for Echo {
    type Output = String;
    const PLUGIN: &str = "test.echo";
    const NAME: &str = "echo";
    const PERMISSIONS: &[&str] = &["test.use"];
}
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Changed {
    text: String,
}
impl PluginEvent for Changed {
    const PLUGIN: &str = "test.echo";
    const NAME: &str = "changed";
    const PERMISSIONS: &[&str] = &["test.use"];
}
#[derive(Serialize, Deserialize)]
struct Slow;
impl Operation for Slow {
    type Output = ();
    const PLUGIN: &str = "test.echo";
    const NAME: &str = "slow";
    const PERMISSIONS: &[&str] = &["test.use"];
}
fn setup() -> (PluginRuntime, PermissionStore) {
    let permissions = PermissionStore::default();
    permissions.set("test.use", PermissionStatus::Granted);
    let runtime = PluginRuntime::new(["test.use"], Arc::new(permissions.clone()));
    let plugin = TypedPlugin::new("test.echo", "1.0.0")
        .operation::<Echo, _, _>(|request, context| async move {
            context.require("test.use")?;
            context.emit(&Changed {
                text: request.text.clone(),
            })?;
            Ok(request.text)
        })
        .unwrap()
        .operation::<Slow, _, _>(|_, _| async { futures::future::pending().await })
        .unwrap()
        .event::<Changed>()
        .unwrap();
    runtime.register(plugin).unwrap();
    (runtime, permissions)
}
#[test]
fn typed_calls_require_ready_state_and_both_policy_layers() {
    block_on(async {
        let (runtime, permissions) = setup();
        assert_eq!(
            runtime
                .call(Echo {
                    text: "before".into()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::Unavailable
        );
        runtime.start("test.echo").await.unwrap();
        assert_eq!(
            runtime
                .call(Echo {
                    text: "hello".into()
                })
                .await
                .unwrap(),
            "hello"
        );
        permissions.set("test.use", PermissionStatus::Denied);
        assert_eq!(
            runtime
                .call(Echo {
                    text: "secret".into()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied
        );
        assert_eq!(
            runtime
                .request_permission("unlisted")
                .await
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied
        );
        assert_eq!(runtime.discover()[0].1, PluginState::Ready);
    });
}
#[test]
fn subscriptions_deliver_typed_events_close_on_revoke_and_bound_overflow() {
    block_on(async {
        let (runtime, permissions) = setup();
        runtime.start("test.echo").await.unwrap();
        let mut events = runtime.subscribe::<Changed>(1).unwrap();
        runtime.call(Echo { text: "one".into() }).await.unwrap();
        assert_eq!(
            events.next().await.unwrap().unwrap(),
            Changed { text: "one".into() }
        );
        for _ in 0..8 {
            runtime
                .call(Echo {
                    text: "overflow".into(),
                })
                .await
                .unwrap();
        }
        assert_eq!(
            events.next().await.unwrap().unwrap_err().code,
            ErrorCode::Lagged
        );
        assert!(events.next().await.is_none());
        let mut events = runtime.subscribe::<Changed>(2).unwrap();
        permissions.set("test.use", PermissionStatus::Denied);
        assert_eq!(
            events.next().await.unwrap().unwrap_err().code,
            ErrorCode::PermissionDenied
        );
        assert!(events.next().await.is_none());
    });
}
#[test]
fn explicit_cancellation_and_shutdown_abort_pending_calls() {
    block_on(async {
        let (runtime, _) = setup();
        runtime.start("test.echo").await.unwrap();
        let mut call = runtime.call(Slow);
        assert!((&mut call).now_or_never().is_none());
        call.cancel();
        assert_eq!(call.await.unwrap_err().code, ErrorCode::Cancelled);
        let mut call = runtime.call(Slow);
        assert!((&mut call).now_or_never().is_none());
        runtime.shutdown().await.unwrap();
        assert_eq!(call.await.unwrap_err().code, ErrorCode::Cancelled);
        assert_eq!(runtime.discover()[0].1, PluginState::Stopped);
        assert_eq!(
            runtime.call(Slow).await.unwrap_err().code,
            ErrorCode::Unavailable
        );
    });
}
#[test]
fn duplicate_registration_and_incompatible_major_versions_are_rejected() {
    let (runtime, _) = setup();
    assert_eq!(
        runtime
            .register(TypedPlugin::new("test.echo", "1"))
            .unwrap_err()
            .code,
        ErrorCode::InvalidRequest
    );
    let descriptor = PluginDescriptor {
        id: "old".into(),
        version: "1".into(),
        api_version: 99,
        operations: vec![],
        events: vec![],
    };
    let bridge = NativeBridge::new(|_: &[u8]| Ok(()));
    assert_eq!(
        runtime
            .register(bridge.plugin(descriptor))
            .unwrap_err()
            .code,
        ErrorCode::IncompatibleVersion
    );
}
#[test]
fn bridge_handles_immediate_responses_events_and_stale_response_rejection() {
    block_on(async {
        let slot = Arc::new(Mutex::new(None::<NativeBridge>));
        let responder = slot.clone();
        let bridge = NativeBridge::new(move |bytes: &[u8]| {
            let message: HostRequest = serde_json::from_slice(bytes).unwrap();
            if let HostRequest::Call { id, payload, .. } = message {
                let message = HostMessage::Response {
                    api_version: 1,
                    id,
                    payload: payload["text"].clone(),
                };
                responder
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .receive(&serde_json::to_vec(&message).unwrap())?;
            }
            Ok(())
        });
        *slot.lock().unwrap() = Some(bridge.clone());
        let permissions = PermissionStore::default();
        permissions.set("test.use", PermissionStatus::Granted);
        let runtime = PluginRuntime::new(["test.use"], Arc::new(permissions));
        runtime
            .register(bridge.plugin(PluginDescriptor {
                id: "test.echo".into(),
                version: "1".into(),
                api_version: 1,
                operations: vec![Capability::operation::<Echo>()],
                events: vec![Capability::event::<Changed>()],
            }))
            .unwrap();
        runtime.start("test.echo").await.unwrap();
        assert_eq!(
            runtime
                .call(Echo {
                    text: "through bridge".into()
                })
                .await
                .unwrap(),
            "through bridge"
        );
        let mut events = runtime.subscribe::<Changed>(2).unwrap();
        bridge.receive(br#"{"type":"event","api_version":1,"plugin":"test.echo","topic":"changed","payload":{"text":"native event"}}"#).unwrap();
        assert_eq!(events.next().await.unwrap().unwrap().text, "native event");
        assert_eq!(
            bridge
                .receive(br#"{"type":"response","api_version":1,"id":"1","payload":null}"#)
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            bridge
                .receive(br#"{"type":"response","api_version":2,"id":"1","payload":null}"#)
                .unwrap_err()
                .code,
            ErrorCode::IncompatibleVersion
        );
        assert!(bridge.receive(&vec![b' '; MAX_MESSAGE_BYTES + 1]).is_err());
        runtime.shutdown().await.unwrap();
        bridge.close();
        *slot.lock().unwrap() = None;
    });
}
#[test]
fn dropping_bridge_calls_sends_cancellation_and_detaching_fails_pending_work() {
    block_on(async {
        let messages = Arc::new(Mutex::new(Vec::<HostRequest>::new()));
        let out = messages.clone();
        let bridge = NativeBridge::new(move |bytes: &[u8]| {
            out.lock()
                .unwrap()
                .push(serde_json::from_slice(bytes).unwrap());
            Ok(())
        });
        let permissions = PermissionStore::default();
        permissions.set("test.use", PermissionStatus::Granted);
        let runtime = PluginRuntime::new(["test.use"], Arc::new(permissions));
        runtime
            .register(bridge.plugin(PluginDescriptor {
                id: "test.echo".into(),
                version: "1".into(),
                api_version: 1,
                operations: vec![Capability::operation::<Echo>()],
                events: vec![],
            }))
            .unwrap();
        runtime.start("test.echo").await.unwrap();
        let mut call = runtime.call(Echo {
            text: "pending".into(),
        });
        assert!((&mut call).now_or_never().is_none());
        drop(call);
        assert!(matches!(
            messages.lock().unwrap().last(),
            Some(HostRequest::Cancel { .. })
        ));
        let mut call = runtime.call(Echo {
            text: "detaching".into(),
        });
        assert!((&mut call).now_or_never().is_none());
        bridge.close();
        assert_eq!(call.await.unwrap_err().code, ErrorCode::Unavailable);
    });
}
struct Startup {
    starts: Arc<AtomicUsize>,
}
impl Plugin for Startup {
    fn descriptor(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: "startup".into(),
            version: "1".into(),
            api_version: 1,
            operations: vec![],
            events: vec![],
        }
    }
    fn start(&self, _: PluginContext) -> futures::future::BoxFuture<'static, PluginResult<()>> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        Box::pin(futures::future::pending())
    }
    fn invoke(
        &self,
        _: &str,
        _: Json,
        _: PluginContext,
    ) -> futures::future::BoxFuture<'static, PluginResult<Json>> {
        Box::pin(async { Ok(Json::Null) })
    }
}
#[test]
fn cancelled_initialization_cannot_publish_a_ready_plugin() {
    let runtime = PluginRuntime::new(Vec::<String>::new(), Arc::new(PermissionStore::default()));
    let starts = Arc::new(AtomicUsize::new(0));
    runtime
        .register(Startup {
            starts: starts.clone(),
        })
        .unwrap();
    let mut startup = Box::pin(runtime.start("startup"));
    assert!((&mut startup).now_or_never().is_none());
    drop(startup);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.discover()[0].1, PluginState::Failed);
}

#[derive(Serialize, Deserialize)]
struct Unprivileged;
impl Operation for Unprivileged {
    type Output = ();
    const PLUGIN: &str = "test.echo";
    const NAME: &str = "unprivileged";
    const PERMISSIONS: &[&str] = &[];
}
#[test]
fn operation_context_cannot_borrow_another_operations_permission() {
    block_on(async {
        let permissions = PermissionStore::default();
        permissions.set("test.use", PermissionStatus::Granted);
        let runtime = PluginRuntime::new(["test.use"], Arc::new(permissions));
        runtime
            .register(
                TypedPlugin::new("test.echo", "1")
                    .operation::<Echo, _, _>(|request, _| async { Ok(request.text) })
                    .unwrap()
                    .operation::<Unprivileged, _, _>(|_, context| async move {
                        context.require("test.use")
                    })
                    .unwrap(),
            )
            .unwrap();
        runtime.start("test.echo").await.unwrap();
        assert_eq!(
            runtime.call(Unprivileged).await.unwrap_err().code,
            ErrorCode::PermissionDenied
        );
    });
}
#[test]
fn an_os_grant_does_not_override_the_application_allowlist() {
    block_on(async {
        let permissions = PermissionStore::default();
        permissions.set("test.use", PermissionStatus::Granted);
        let runtime = PluginRuntime::new(Vec::<String>::new(), Arc::new(permissions));
        runtime
            .register(
                TypedPlugin::new("test.echo", "1")
                    .operation::<Echo, _, _>(|request, _| async { Ok(request.text) })
                    .unwrap(),
            )
            .unwrap();
        runtime.start("test.echo").await.unwrap();
        assert_eq!(
            runtime
                .call(Echo {
                    text: "not sent".into()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::PermissionDenied
        );
    });
}
