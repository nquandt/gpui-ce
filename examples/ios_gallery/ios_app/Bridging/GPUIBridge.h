#ifndef GPUI_BRIDGE_H
#define GPUI_BRIDGE_H

void *gpui_ios_initialize(void);
void gpui_ios_gallery_main(void);
void gpui_ios_did_finish_launching(void *app_ptr);
void gpui_ios_will_enter_foreground(void *app_ptr);
void gpui_ios_did_become_active(void *app_ptr);
void gpui_ios_will_resign_active(void *app_ptr);
void gpui_ios_did_enter_background(void *app_ptr);
void gpui_ios_will_terminate(void *app_ptr);
void *gpui_ios_get_window(void);
void gpui_ios_request_frame(void *window_ptr);
void gpui_ios_handle_open_url(void *url_ptr);

#endif
