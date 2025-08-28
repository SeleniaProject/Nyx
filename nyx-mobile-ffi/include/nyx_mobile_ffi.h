#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Initialize Nyx mobile layer with complete protocol support
 * Returns 0 on success, 1 if already initialized, other codes for errors
 */
int nyx_mobile_init(void);

/**
 * Create and configure a Nyx client with mobile optimizations
 */
int nyx_mobile_create_client(const char *config_json);

/**
 * Connect to the Nyx network with specified endpoint
 *
 * # Safety
 * - `endpoint` は有効なヌル終端 C 文字列でなければならない
 * - `connection_id_out` は有効な書き込み可能ポインタでなければならない
 * - 呼び出し元はこれらポインタのライフタイムと整合性を保証する必要がある
 */
int nyx_mobile_connect(const char *endpoint,
                       unsigned long *connection_id_out);

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

/**
 * Send data over established connection
 */
int nyx_mobile_send_data(unsigned long connection_id,
                         const void *data,
                         uintptr_t data_len,
                         uintptr_t *bytes_sent_out);

/**
 * Receive data from connection (non-blocking)
 */
int nyx_mobile_receive_data(unsigned long connection_id,
                            void *buffer,
                            uintptr_t buffer_len,
                            uintptr_t *bytes_received_out);

/**
 * Disconnect from specific connection
 */
int nyx_mobile_disconnect(unsigned long connection_id);

/**
 * Get connection statistics
 */
int nyx_mobile_get_connection_stats(unsigned long connection_id,
                                    unsigned long *bytes_sent_out,
                                    unsigned long *bytes_received_out,
                                    int *quality_out);

/**
 * Set network type for optimization
 */
int nyx_mobile_set_network_type(int network_type);

/**
 * Get current network type
 */
int nyx_mobile_get_network_type(int *network_type_out);

/**
 * Update mobile configuration at runtime
 */
int nyx_mobile_update_config(const char *config_json);

/**
 * Get global protocol statistics
 */
int nyx_mobile_get_global_stats(unsigned long *total_connections_out,
                                unsigned long *successful_handshakes_out,
                                unsigned long *connection_failures_out,
                                unsigned long *network_changes_out);

/**
 * Enable background mode optimizations
 */
int nyx_mobile_enter_background_mode(void);

/**
 * Disable background mode optimizations
 */
int nyx_mobile_enter_foreground_mode(void);

/**
 * Force connection quality assessment
 */
int nyx_mobile_assess_connection_quality(void);
