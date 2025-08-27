#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Initialize Nyx mobile layer. Idempotent.
 * Return_s 0 on succes_s, 1 if already initialized.
 */
int nyx_mobile_init(void);

/**
 * Shutdown Nyx mobile layer. Safe to call multiple time_s.
 */
int nyx_mobile_shutdown(void);

/**
 * Set log level: 0=ERROR,1=WARN,2=INFO,3=DEBUG,4=TRACE
 */
int nyx_mobile_set____log_level(int level);

/**
 * Set a telemetry label key/value. Passing a null value removes the key. Passing a null key is invalid.
 * Returns 0 on success.
 *
 * # Safety
 * - `key` and `value` must be valid C strings (null-terminated) or null pointers
 * - If not null, the pointers must remain valid for the duration of the call
 * - The caller must ensure proper memory management for the strings
 */
int nyx_mobile_set_telemetry_label(const char *key,
                                   const char *value);

/**
 * Clear all telemetry label_s.
 */
int nyx_mobile_clear_telemetry_label_s(void);

/**
 * Get crate version string. Returns length excluding NUL.
 * Writes up to `buf_len-1` bytes and NUL-terminates. If buf_len==0, returns needed length.
 *
 * # Safety
 * - If `buf` is not null, it must point to valid, writable memory of at least `buf_len` bytes
 * - The caller must ensure the buffer remains valid for the duration of the call
 * - If `buf_len` is 0, `buf` can be null (used for size query)
 *
 * # Security Enhancements
 * - Validates buffer parameters to prevent buffer overflow attacks
 * - Uses safe memory operations with bounds checking
 * - Prevents integer overflow in size calculations
 */
int nyx_mobile_version(char *buf, uintptr_t buf_len);

/**
 * Return last error message length (excluding NUL). If a buffer is provided, copy it.
 *
 * # Safety
 * - If `buf` is not null, it must point to valid, writable memory of at least `buf_len` bytes
 * - The caller must ensure the buffer remains valid for the duration of the call
 * - If `buf_len` is 0, `buf` can be null (used for size query)
 */
int nyx_mobile_last_error(char *buf, uintptr_t buf_len);

/**
 * Set unified power state. Return_s InvalidArgument if state i_s unknown.
 */
int nyx_power_set_state(uint32_t state);

/**
 * Return current power state value as u32 (Active=0,...). Returns InvalidArgument on null ptr.
 *
 * # Safety
 * - `out_state` must be a valid, non-null pointer to writable memory
 * - The caller must ensure the pointer remains valid for the duration of the call
 */
int nyx_power_get_state(uint32_t *out_state);

/**
 * Push wake entry point to kick resume controller. Increment_s a counter.
 */
int nyx_push_wake(void);

/**
 * Explicit resume trigger when OS grant_s execution window.
 */
int nyx_resume_low_power_session(void);
