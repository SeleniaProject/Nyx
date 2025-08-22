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
 * Set a telemetry label key/value. Passing a null value remove_s the key. Passing a null key i_s invalid.
 * Return_s 0 on succes_s.
 */
int nyx_mobile_set_telemetry_label(const char *key,
                                   const char *value);

/**
 * Clear all telemetry label_s.
 */
int nyx_mobile_clear_telemetry_label_s(void);

/**
 * Get crate version string. Return_s length excluding NUL.
 * Write_s up to `buf_len-1` byte_s and NUL-terminate_s. If buf_len==0, return_s needed length.
 */
int nyx_mobile_version(char *buf, uintptr_t buf_len);

/**
 * Return last error message length (excluding NUL). If a buffer i_s provided, copy it.
 */
int nyx_mobile_last_error(char *buf, uintptr_t buf_len);

/**
 * Set unified power state. Return_s InvalidArgument if state i_s unknown.
 */
int nyx_power_set_state(uint32_t state);

/**
 * Return current power state value as u32 (Active=0,...). Return_s InvalidArgument on null ptr.
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
