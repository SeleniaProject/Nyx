#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Initialize Nyx mobile layer. Idempotent.
 * Returns 0 on success, 1 if already initialized.
 */
int nyx_mobile_init(void);

/**
 * Shutdown Nyx mobile layer. Safe to call multiple times.
 */
int nyx_mobile_shutdown(void);

/**
 * Set log level: 0=ERROR,1=WARN,2=INFO,3=DEBUG,4=TRACE
 */
int nyx_mobile_set_log_level(int level);

/**
 * Set a telemetry label key/value. Passing a null value removes the key. Passing a null key is invalid.
 * Returns 0 on success.
 */
int nyx_mobile_set_telemetry_label(const char *key,
                                   const char *value);

/**
 * Clear all telemetry labels.
 */
int nyx_mobile_clear_telemetry_labels(void);

/**
 * Get crate version string. Returns length excluding NUL.
 * Writes up to `buf_len-1` bytes and NUL-terminates. If buf_len==0, returns needed length.
 */
int nyx_mobile_version(char *buf, uintptr_t buf_len);

/**
 * Return last error message length (excluding NUL). If a buffer is provided, copy it.
 */
int nyx_mobile_last_error(char *buf, uintptr_t buf_len);

/**
 * Set unified power state. Returns InvalidArgument if state is unknown.
 */
int nyx_power_set_state(uint32_t state);

/**
 * Return current power state value as u32 (Active=0,...). Returns InvalidArgument on null ptr.
 */
int nyx_power_get_state(uint32_t *out_state);

/**
 * Push wake entry point to kick resume controller. Increments a counter.
 */
int nyx_push_wake(void);

/**
 * Explicit resume trigger when OS grants execution window.
 */
int nyx_resume_low_power_session(void);
