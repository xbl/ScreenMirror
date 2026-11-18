#ifndef SM_VIRTUAL_DISPLAY_H
#define SM_VIRTUAL_DISPLAY_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Check whether CGVirtualDisplay is available in the current macOS runtime.
bool sm_virtual_display_is_supported(void);

/// Create a new virtual display with the specified dimensions, refresh rate, and HiDPI setting.
/// Returns the CGDirectDisplayID (> 0) on success, or 0 on failure.
uint32_t sm_virtual_display_create(const char *name, uint32_t width, uint32_t height, double refresh_rate, uint32_t hi_dpi);

/// Destroy the virtual display with the given CGDirectDisplayID.
/// Returns true if found and destroyed, false otherwise.
bool sm_virtual_display_destroy(uint32_t display_id);

/// Destroy all active virtual displays created by this process.
void sm_virtual_display_destroy_all(void);

/// Get the count of active virtual displays created by this process, writing up to `max_count` IDs into `out_ids`.
uint32_t sm_virtual_display_get_active_ids(uint32_t *out_ids, uint32_t max_count);

#ifdef __cplusplus
}
#endif

#endif // SM_VIRTUAL_DISPLAY_H
