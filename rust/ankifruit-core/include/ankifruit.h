// AnkiFruit core — C ABI for the Swift shell.
// Licensed AGPL-3.0-or-later (links Anki's rslib).

#ifndef ANKIFRUIT_H
#define ANKIFRUIT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct AnkiFruitHandle AnkiFruitHandle;

/// Start the Anki backend and its loopback HTTP server.
/// preferred_langs: comma-separated (e.g. "en,ja"), or NULL for "en".
/// web_root: directory holding the built web UI, served from the same origin
///           as the API, or NULL to serve the API only.
/// port: 0 to let the OS pick.
/// Returns NULL on failure; if err_out is non-NULL it receives an owned string.
AnkiFruitHandle *ankifruit_start(const char *preferred_langs,
                                 const char *web_root,
                                 uint16_t port,
                                 char **err_out);

/// Port the backend is listening on, or 0 if handle is NULL.
uint16_t ankifruit_port(const AnkiFruitHandle *handle);

/// Bearer token required on every request. Caller must free with
/// ankifruit_string_free.
char *ankifruit_token(const AnkiFruitHandle *handle);

/// Linked rslib build hash. Caller must free with ankifruit_string_free.
char *ankifruit_anki_buildhash(void);

/// Shut down and free the handle. The handle must not be used afterwards.
void ankifruit_stop(AnkiFruitHandle *handle);

/// Free a string returned by this library.
void ankifruit_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* ANKIFRUIT_H */
