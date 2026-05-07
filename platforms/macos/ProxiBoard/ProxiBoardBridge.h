// ClipRelayBridge.h
// Auto-generated C header for the Rust cliprelay-core FFI.
// Add to your Xcode project's "Objective-C Bridging Header" setting.

#pragma once
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque types
typedef struct ClipRelayHandle ClipRelayHandle;
typedef struct PbEvent PbEvent;

// Event codes
#define PB_EVENT_NONE              0
#define PB_EVENT_CLIPBOARD_TEXT    1
#define PB_EVENT_CLIPBOARD_IMAGE   2
#define PB_EVENT_CLIPBOARD_FILE    3
#define PB_EVENT_TOFU_PROMPT       4
#define PB_EVENT_PEER_CONNECTED    5
#define PB_EVENT_PEER_DISCONNECTED 6
#define PB_EVENT_WARNING           7

/// Start the ClipRelay engine. Returns NULL on failure.
/// @param device_name  UTF-8 device name, or NULL for auto-detection.
/// @param port         TCP port (0 = default 47823).
ClipRelayHandle *cliprelay_start(const char *device_name, uint16_t port);

/// Stop and free the engine.
void cliprelay_stop(ClipRelayHandle *handle);

/// Push UTF-8 text to all peers. Returns peer count or -1 on error.
int32_t cliprelay_push_text(ClipRelayHandle *handle, const char *text);

/// Push image bytes to all peers.
int32_t cliprelay_push_image(
    ClipRelayHandle *handle,
    const char *mime_type,
    const uint8_t *data,
    size_t len
);

/// Push a file to all peers.
int32_t cliprelay_push_file(
    ClipRelayHandle *handle,
    const char *filename,
    const uint8_t *data,
    size_t len
);

/// Non-blocking poll for the next engine event. Returns NULL if none.
/// Caller must free with cliprelay_free_event().
PbEvent *cliprelay_poll_event(ClipRelayHandle *handle);

/// Returns the event type code.
int32_t cliprelay_event_type(const PbEvent *event);

/// Returns the text payload (TEXT events). Valid until cliprelay_free_event().
const char *cliprelay_event_text(PbEvent *event);

/// Returns the image data pointer (IMAGE events). Valid until cliprelay_free_event().
const uint8_t *cliprelay_event_image_data(PbEvent *event, size_t *out_len, const char **out_mime);

/// Returns the file data pointer (FILE events). Valid until cliprelay_free_event().
const uint8_t *cliprelay_event_file_data(PbEvent *event, size_t *out_len, const char **out_name);

/// Returns the device name associated with the event.
const char *cliprelay_event_device_name(PbEvent *event);

/// Returns the fingerprint display string (TOFU_PROMPT events).
const char *cliprelay_event_fingerprint(PbEvent *event);

/// Free an event returned by cliprelay_poll_event().
void cliprelay_free_event(PbEvent *event);

#ifdef __cplusplus
}
#endif
