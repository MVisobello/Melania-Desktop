/*
 * Real libwayland-client probe for the QEMU-only Smithay seat bridge.
 * Creates a genuine wl_surface and waits for real virtual-USB key/button
 * events forwarded from libinput into wl_keyboard and wl_pointer.
 * This does not prove rendering, GTK window management or login readiness.
 */
#include <wayland-client.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

struct state {
    struct wl_display *display;
    struct wl_compositor *compositor;
    struct wl_seat *seat;
    struct wl_keyboard *keyboard;
    struct wl_pointer *pointer;
    struct wl_surface *surface;
    int focused_keyboard;
    int focused_pointer;
    int keys;
    int buttons;
};
static void on_keyboard_keymap(void *data,struct wl_keyboard *obj,
                               uint32_t format,int32_t fd,uint32_t size) {
    (void)data;(void)obj;(void)format;(void)size;
    if(fd>=0) close(fd);
}
static void on_keyboard_enter(void *data,struct wl_keyboard *obj,uint32_t serial,
                              struct wl_surface *surface,struct wl_array *keys) {
    (void)obj;(void)serial;(void)surface;(void)keys;
    ((struct state*)data)->focused_keyboard=1;
}
static void on_keyboard_leave(void *data,struct wl_keyboard *obj,uint32_t serial,
                              struct wl_surface *surface) {
    (void)obj;(void)serial;(void)surface;
    ((struct state*)data)->focused_keyboard=0;
}
static void on_keyboard_key(void *data,struct wl_keyboard *obj,uint32_t serial,
                            uint32_t time,uint32_t key,uint32_t state) {
    (void)obj;(void)serial;(void)time;(void)key;
    if(state==WL_KEYBOARD_KEY_STATE_PRESSED)
        ((struct state*)data)->keys++;
}
static void on_keyboard_modifiers(void *data,struct wl_keyboard *obj,uint32_t serial,
                                  uint32_t depressed,uint32_t latched,
                                  uint32_t locked,uint32_t group) {
    (void)data;(void)obj;(void)serial;(void)depressed;
    (void)latched;(void)locked;(void)group;
}
static void on_keyboard_repeat(void *data,struct wl_keyboard *obj,int32_t rate,int32_t delay) {
    (void)data;(void)obj;(void)rate;(void)delay;
}
static const struct wl_keyboard_listener key_listener={
    .keymap=on_keyboard_keymap,.enter=on_keyboard_enter,
    .leave=on_keyboard_leave,.key=on_keyboard_key,
    .modifiers=on_keyboard_modifiers,.repeat_info=on_keyboard_repeat,
};
static void on_pointer_enter(void *data,struct wl_pointer *obj,uint32_t serial,
                             struct wl_surface *surface,wl_fixed_t x,wl_fixed_t y) {
    (void)obj;(void)serial;(void)surface;(void)x;(void)y;
    ((struct state*)data)->focused_pointer=1;
}
static void on_pointer_leave(void *data,struct wl_pointer *obj,uint32_t serial,
                             struct wl_surface *surface) {
    (void)obj;(void)serial;(void)surface;
    ((struct state*)data)->focused_pointer=0;
}
static void on_pointer_motion(void *data,struct wl_pointer *obj,uint32_t time,
                              wl_fixed_t x,wl_fixed_t y) {
    (void)data;(void)obj;(void)time;(void)x;(void)y;
}
static void on_pointer_button(void *data,struct wl_pointer *obj,uint32_t serial,
                              uint32_t time,uint32_t button,uint32_t state) {
    (void)obj;(void)serial;(void)time;(void)button;
    if(state==WL_POINTER_BUTTON_STATE_PRESSED)
        ((struct state*)data)->buttons++;
}
static void on_pointer_axis(void *data,struct wl_pointer *obj,uint32_t time,
                            uint32_t axis,wl_fixed_t value) {
    (void)data;(void)obj;(void)time;(void)axis;(void)value;
}
static void on_pointer_frame(void *data,struct wl_pointer *obj) {
    (void)data;(void)obj;
}
static void on_pointer_axis_source(void *data,struct wl_pointer *obj,uint32_t source) {
    (void)data;(void)obj;(void)source;
}
static void on_pointer_axis_stop(void *data,struct wl_pointer *obj,uint32_t time,uint32_t axis) {
    (void)data;(void)obj;(void)time;(void)axis;
}
static void on_pointer_axis_discrete(void *data,struct wl_pointer *obj,
                                     uint32_t axis,int32_t discrete) {
    (void)data;(void)obj;(void)axis;(void)discrete;
}
static const struct wl_pointer_listener pointer_listener={
    .enter=on_pointer_enter,.leave=on_pointer_leave,
    .motion=on_pointer_motion,.button=on_pointer_button,.axis=on_pointer_axis,
    .frame=on_pointer_frame,.axis_source=on_pointer_axis_source,
    .axis_stop=on_pointer_axis_stop,.axis_discrete=on_pointer_axis_discrete,
};
static void seat_capabilities(void *data,struct wl_seat *seat,uint32_t caps) {
    struct state *s=data;
    if ((caps & WL_SEAT_CAPABILITY_KEYBOARD) && !s->keyboard) {
        s->keyboard=wl_seat_get_keyboard(seat);
        wl_keyboard_add_listener(s->keyboard,&key_listener,s);
    }
    if ((caps & WL_SEAT_CAPABILITY_POINTER) && !s->pointer) {
        s->pointer=wl_seat_get_pointer(seat);
        wl_pointer_add_listener(s->pointer,&pointer_listener,s);
    }
}
static void seat_name(void *data,struct wl_seat *seat,const char *name) {
    (void)data;(void)seat;(void)name;
}
static const struct wl_seat_listener seat_listener={
    .capabilities=seat_capabilities,.name=seat_name,
};
static void registry_global(void *data,struct wl_registry *registry,uint32_t name,
                            const char *interface,uint32_t version) {
    struct state *s=data;
    if (!strcmp(interface,wl_compositor_interface.name)) {
        s->compositor=wl_registry_bind(registry,name,&wl_compositor_interface,
                                      version<4?version:4);
    } else if (!strcmp(interface,wl_seat_interface.name)) {
        s->seat=wl_registry_bind(registry,name,&wl_seat_interface,
                                version<5?version:5);
        wl_seat_add_listener(s->seat,&seat_listener,s);
    }
}
static void registry_remove(void *data,struct wl_registry *registry,uint32_t name) {
    (void)data;(void)registry;(void)name;
}
static const struct wl_registry_listener registry_listener={
    .global=registry_global,.global_remove=registry_remove,
};
int main(void) {
    struct state s={0};
    s.display=wl_display_connect(NULL);
    if (!s.display) {fputs("MELANIA_NATIVE_WAYLAND_CLIENT_CONNECT_FAIL\n",stderr);return 1;}
    struct wl_registry *registry=wl_display_get_registry(s.display);
    wl_registry_add_listener(registry,&registry_listener,&s);
    if (wl_display_roundtrip(s.display)<0 || wl_display_roundtrip(s.display)<0 ||
        !s.compositor || !s.seat || !s.keyboard || !s.pointer) {
        fputs("MELANIA_NATIVE_WAYLAND_MISSING_GLOBALS\n",stderr);return 1;
    }
    s.surface=wl_compositor_create_surface(s.compositor);
    wl_surface_commit(s.surface);
    if(wl_display_roundtrip(s.display)<0 ||
       !s.focused_keyboard || !s.focused_pointer) {
        fputs("MELANIA_NATIVE_WAYLAND_FOCUS_MISSING\n",stderr);return 1;
    }
    puts("MELANIA_NATIVE_WAYLAND_CLIENT_READY=true");fflush(stdout);
    const int fd=wl_display_get_fd(s.display);
    for (int i=0;i<120 && (!s.keys || !s.buttons);i++) {
        struct pollfd descriptor={.fd=fd,.events=POLLIN,.revents=0};
        if(wl_display_flush(s.display)<0)break;
        int result=poll(&descriptor,1,100);
        if(result<0)break;
        if(result>0 && wl_display_dispatch(s.display)<0)break;
    }
    printf("MELANIA_NATIVE_WAYLAND_CLIENT_KEY_PRESSES=%d\n",s.keys);
    printf("MELANIA_NATIVE_WAYLAND_CLIENT_POINTER_PRESSES=%d\n",s.buttons);
    int ok=s.keys>0 && s.buttons>0;
    printf("MELANIA_NATIVE_WAYLAND_CLIENT_INPUT_PASS=%s\n",ok?"true":"false");
    wl_surface_destroy(s.surface);
    wl_pointer_destroy(s.pointer);
    wl_keyboard_destroy(s.keyboard);
    wl_seat_destroy(s.seat);
    wl_registry_destroy(registry);
    wl_display_disconnect(s.display);
    return ok?0:1;
}
