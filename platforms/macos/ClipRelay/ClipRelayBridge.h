// ClipRelayBridge.h
// C header for the Rust cliprelay-core FFI — add to Xcode bridging header.

#pragma once
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// ── Opaque types ──────────────────────────────────────────────────────────────
typedef struct ClipRelayHandle ClipRelayHandle;
typedef struct PbEvent         PbEvent;

// ── Event type codes ──────────────────────────────────────────────────────────
#define PB_EVENT_NONE                    0
#define PB_EVENT_CLIPBOARD_TEXT          1   // auto-applied to local clipboard
#define PB_EVENT_CLIPBOARD_IMAGE         2   // auto-applied
#define PB_EVENT_CLIPBOARD_FILE          3   // auto-applied (legacy)
#define PB_EVENT_TOFU_PROMPT             4
#define PB_EVENT_PEER_CONNECTED          5
#define PB_EVENT_PEER_DISCONNECTED       6
#define PB_EVENT_WARNING                 7
#define PB_EVENT_CLIPBOARD_SYNCED        8
// 9, 10 reserved
#define PB_EVENT_CLIPBOARD_AVAILABLE    11   // timeline-first: in feed, NOT auto-applied
#define PB_EVENT_FILE_TRANSFER_INCOMING 12
#define PB_EVENT_FILE_TRANSFER_PROGRESS 13
#define PB_EVENT_FILE_TRANSFER_COMPLETE 14
#define PB_EVENT_FILE_TRANSFER_FAILED   15
#define PB_EVENT_ACTIVITY_UPDATED       16

// ── Engine lifecycle ──────────────────────────────────────────────────────────
/// Start engine. Returns NULL on failure.
/// @param device_name UTF-8 device name, or NULL for auto-detection.
/// @param port        TCP port (0 = default 47823).
ClipRelayHandle *cliprelay_start(const char *device_name, uint16_t port);

/// Stop and free the engine.
void cliprelay_stop(ClipRelayHandle *handle);

// ── Clipboard push ────────────────────────────────────────────────────────────
int32_t cliprelay_push_text(ClipRelayHandle *handle, const char *text);
int32_t cliprelay_push_image(ClipRelayHandle *handle, const char *mime_type,
                             const uint8_t *data, size_t len);
int32_t cliprelay_push_file(ClipRelayHandle *handle, const char *filename,
                            const uint8_t *data, size_t len);

// ── Event poll ────────────────────────────────────────────────────────────────
/// Non-blocking. Returns NULL if no event. Caller must free with cliprelay_free_event().
PbEvent *cliprelay_poll_event(ClipRelayHandle *handle);
/// Returns the event type code for @p event.
int32_t  cliprelay_event_type(const PbEvent *event);
/// Free an event returned by cliprelay_poll_event().
void     cliprelay_free_event(PbEvent *event);

// ── Common event accessors ────────────────────────────────────────────────────
const char    *cliprelay_event_text(PbEvent *event);
const char    *cliprelay_event_device_name(PbEvent *event);
const char    *cliprelay_event_fingerprint(PbEvent *event);
const uint8_t *cliprelay_event_image_data(PbEvent *event, size_t *out_len,
                                          const char **out_mime);
const uint8_t *cliprelay_event_file_data(PbEvent *event, size_t *out_len,
                                         const char **out_name);

// ── Timeline-first clipboard ──────────────────────────────────────────────────
/// 1 if auto-applied; 0 if timeline-first (user must apply manually).
int32_t cliprelay_event_auto_applied(const PbEvent *event);
/// Activity feed entry ID; -1 if not applicable.
int64_t cliprelay_event_activity_id(const PbEvent *event);
/// Apply clipboard item to local clipboard by content hash. Returns 1 on success.
int32_t cliprelay_apply_clipboard(ClipRelayHandle *handle, const char *hash);

// ── File transfer ─────────────────────────────────────────────────────────────
const char *cliprelay_event_transfer_id(PbEvent *event);
const char *cliprelay_event_transfer_file_name(PbEvent *event);
int32_t     cliprelay_event_transfer_percent(const PbEvent *event);
int64_t     cliprelay_event_transfer_total_bytes(const PbEvent *event);
const char *cliprelay_event_transfer_dest_path(PbEvent *event);
int32_t     cliprelay_accept_file_transfer(ClipRelayHandle *handle,
                                           const char *transfer_id_hex);
int32_t     cliprelay_reject_file_transfer(ClipRelayHandle *handle,
                                           const char *transfer_id_hex);

#ifdef __cplusplus
}
#endif
