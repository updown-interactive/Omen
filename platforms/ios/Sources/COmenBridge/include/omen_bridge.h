#ifndef OMEN_BRIDGE_H
#define OMEN_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Callback must copy data synchronously. Context is a host-owned integer token. */
typedef bool (*omen_send_callback)(uint64_t context, const uint8_t *data, size_t len);
uint64_t omen_bridge_create(omen_send_callback send, uint64_t context);
bool omen_bridge_receive(uint64_t handle, const uint8_t *data, size_t len);
void omen_bridge_destroy(uint64_t handle);
#ifdef __cplusplus
}
#endif
#endif
