#include <jni.h>
#include <cstdint>
#include <mutex>
#include <unordered_map>
#include <memory>
#include "omen_bridge.h"
namespace {
JavaVM* vm = nullptr;
struct Host { jobject instance; jmethodID dispatch; };
std::mutex hosts_mutex;
std::unordered_map<uint64_t, std::shared_ptr<Host>> hosts;
uint64_t next_context = 1;
bool send_to_kotlin(uint64_t context, const uint8_t* data, size_t len) {
    if (!vm || len > 1048576) return false;
    JNIEnv* env = nullptr; bool attached = false;
    if (vm->GetEnv(reinterpret_cast<void**>(&env), JNI_VERSION_1_6) != JNI_OK) {
        if (vm->AttachCurrentThread(&env, nullptr) != JNI_OK) return false;
        attached = true;
    }
    jobject instance = nullptr; jmethodID method = nullptr;
    { std::lock_guard<std::mutex> guard(hosts_mutex);
      auto found = hosts.find(context);
      if (found != hosts.end()) { instance = env->NewLocalRef(found->second->instance); method = found->second->dispatch; } }
    bool success = false;
    if (instance) {
        jbyteArray bytes = env->NewByteArray(static_cast<jsize>(len));
        if (bytes) {
            env->SetByteArrayRegion(bytes, 0, static_cast<jsize>(len), reinterpret_cast<const jbyte*>(data));
            env->CallVoidMethod(instance, method, bytes);
            success = !env->ExceptionCheck();
            if (env->ExceptionCheck()) env->ExceptionClear();
            env->DeleteLocalRef(bytes);
        }
        env->DeleteLocalRef(instance);
    }
    if (attached) vm->DetachCurrentThread();
    return success;
}
}
extern "C" JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM* machine, void*) { vm = machine; return JNI_VERSION_1_6; }
extern "C" JNIEXPORT jlong JNICALL Java_dev_omen_nativebridge_OmenBridge_nativeCreate(JNIEnv* env, jobject self) {
    jclass cls = env->GetObjectClass(self);
    jmethodID dispatch = env->GetMethodID(cls, "dispatch", "([B)V"); env->DeleteLocalRef(cls);
    if (!dispatch || env->ExceptionCheck()) return 0;
    jobject reference = env->NewGlobalRef(self); if (!reference) return 0;
    uint64_t context;
    { std::lock_guard<std::mutex> guard(hosts_mutex);
      context = next_context++; if (!context) { env->DeleteGlobalRef(reference); return 0; }
      hosts.emplace(context, std::make_shared<Host>(Host{reference, dispatch})); }
    uint64_t handle = omen_bridge_create(send_to_kotlin, context);
    if (!handle) { std::lock_guard<std::mutex> guard(hosts_mutex); hosts.erase(context); env->DeleteGlobalRef(reference); return 0; }
    // Kotlin stores context as a separate field for deterministic global-reference cleanup.
    jclass type = env->GetObjectClass(self); auto field = env->GetFieldID(type, "nativeContext", "J"); env->DeleteLocalRef(type);
    if (!field) { omen_bridge_destroy(handle); std::lock_guard<std::mutex> guard(hosts_mutex); hosts.erase(context); env->DeleteGlobalRef(reference); return 0; }
    env->SetLongField(self, field, static_cast<jlong>(context)); return static_cast<jlong>(handle);
}
extern "C" JNIEXPORT jboolean JNICALL Java_dev_omen_nativebridge_OmenBridge_nativeReceive(JNIEnv* env, jobject, jlong handle, jbyteArray bytes) {
    if (!bytes) return JNI_FALSE; jsize len = env->GetArrayLength(bytes); if (len <= 0 || len > 1048576) return JNI_FALSE;
    jbyte* data = env->GetByteArrayElements(bytes, nullptr); if (!data) return JNI_FALSE;
    bool result = omen_bridge_receive(static_cast<uint64_t>(handle), reinterpret_cast<uint8_t*>(data), static_cast<size_t>(len));
    env->ReleaseByteArrayElements(bytes, data, JNI_ABORT); return result ? JNI_TRUE : JNI_FALSE;
}
extern "C" JNIEXPORT void JNICALL Java_dev_omen_nativebridge_OmenBridge_nativeDestroy(JNIEnv* env, jobject, jlong handle, jlong context) {
    { std::lock_guard<std::mutex> guard(hosts_mutex); auto found = hosts.find(static_cast<uint64_t>(context));
      if (found != hosts.end()) { env->DeleteGlobalRef(found->second->instance); hosts.erase(found); } }
    omen_bridge_destroy(static_cast<uint64_t>(handle));
}
