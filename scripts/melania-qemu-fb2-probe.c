/*
 * Stage21: disposable QEMU-only GEM handle acquisition/close audit.
 *
 * drmModeGetFB2 is NOT purely read-only: if DRM master/CAP_SYS_ADMIN it
 * returns newly referenced GEM handles. Each distinct returned handle MUST
 * be GEM_CLOSE'd on the same fd. No framebuffer is created/removed, no mode,
 * plane, property, or VT is ever changed. This is a feasibility probe only.
 */
#define _GNU_SOURCE
#include <drm.h>
#include <xf86drm.h>
#include <xf86drmMode.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

#define MAX_PLANES 128U
#define MAX_FBS 128U
#define MAX_GEM_HANDLES 512U

static int qemu_dmi(void) {
    const char *paths[] = {
        "/sys/class/dmi/id/sys_vendor", "/sys/class/dmi/id/product_name"
    };
    for (size_t i = 0; i < 2; i++) {
        FILE *file = fopen(paths[i], "r");
        if (!file) continue;
        char value[256] = {0};
        const char *read = fgets(value, sizeof(value), file);
        fclose(file);
        if (read && (strstr(value, "QEMU") || strstr(value, "Bochs") ||
                     strstr(value, "KVM"))) return 1;
    }
    return 0;
}

static int gem_close(int fd, uint32_t handle) {
    struct drm_gem_close request = { .handle = handle };
    return ioctl(fd, DRM_IOCTL_GEM_CLOSE, &request);
}

static int close_fb2_handles(int fd, drmModeFB2Ptr fb, unsigned *closed) {
    uint32_t distinct[4] = {0};
    unsigned count = 0;
    int ok = 1;
    /* A single multiplanar framebuffer may refer to the same GEM handle
       more than once. Closing it twice would be a correctness bug. */
    for (size_t i = 0; i < 4; ++i) {
        uint32_t value = fb->handles[i];
        if (!value) continue;
        int duplicate = 0;
        for (unsigned j = 0; j < count; j++)
            if (distinct[j] == value) duplicate = 1;
        if (duplicate) continue;
        distinct[count++] = value;
    }
    for (unsigned i = 0; i < count; i++) {
        if (gem_close(fd, distinct[i]) < 0) {
            fprintf(stderr, "MELANIA_FB2_GEM_CLOSE_ERROR=%s\n", strerror(errno));
            ok = 0;
        } else {
            (*closed)++;
        }
    }
    return ok ? (int)count : -1;
}

static int inspect_fb(int fd, uint32_t fb_id, unsigned *acquired,
                      unsigned *released) {
    drmModeFB2Ptr fb = drmModeGetFB2(fd, fb_id);
    if (!fb) {
        fprintf(stderr, "MELANIA_FB2_GET_ERROR=%s\n", strerror(errno));
        return -1;
    }
    int dimensions_valid = fb->width >= 1 && fb->height >= 1 &&
        fb->width <= 8192 && fb->height <= 8192 && fb->pixel_format != 0;
    unsigned planes = 0;
    for (unsigned i = 0; i < 4; i++) {
        if (fb->pitches[i]) {
            planes++;
            if (fb->pitches[i] > 1024U * 1024U)
                dimensions_valid = 0;
        }
    }
    unsigned count_before = *released;
    int count = close_fb2_handles(fd, fb, released);
    drmModeFreeFB2(fb);
    if (count < 1 || !dimensions_valid || !planes) {
        fprintf(stderr, "MELANIA_FB2_INCOMPLETE_METADATA_OR_GEM_RIGHTS=true\n");
        return -1;
    }
    *acquired += (unsigned)count;
    if (*released != count_before + (unsigned)count)
        return -1;
    printf("MELANIA_FB2_ONE_FB_VALID=true\n");
    return 0;
}

int main(void) {
    if (!qemu_dmi() || getenv("DISPLAY") || getenv("WAYLAND_DISPLAY") ||
        !getenv("MELANIA_FB2_QEMU_ONLY") ||
        strcmp(getenv("MELANIA_FB2_QEMU_ONLY"), "YES_DISPOSABLE_VM") != 0) {
        fputs("MELANIA_FB2_BLOCKED=QEMU_DMI_AND_EXPLICIT_OPT_IN_REQUIRED\n", stderr);
        return 78;
    }
    struct stat tty = {0};
    if (fstat(STDIN_FILENO, &tty) < 0 || !S_ISCHR(tty.st_mode)) {
        fputs("MELANIA_FB2_BLOCKED=DEDICATED_TTY_REQUIRED\n", stderr);
        return 78;
    }
    char *console = ttyname(STDIN_FILENO);
    if (!console || strncmp(console, "/dev/tty", 8) != 0 ||
        console[8] < '1' || console[8] > '9') {
        fputs("MELANIA_FB2_BLOCKED=REAL_LINUX_VT_REQUIRED\n", stderr);
        return 78;
    }

    int fd = open("/dev/dri/card0", O_RDWR | O_CLOEXEC | O_NOCTTY);
    if (fd < 0) {
        fprintf(stderr, "MELANIA_FB2_CARD_OPEN_ERROR=%s\n", strerror(errno));
        return 1;
    }
    if (drmSetClientCap(fd, DRM_CLIENT_CAP_UNIVERSAL_PLANES, 1) != 0) {
        fputs("MELANIA_FB2_UNIVERSAL_PLANES_UNAVAILABLE=true\n", stderr);
        close(fd);
        return 1;
    }

    drmModePlaneResPtr resources = drmModeGetPlaneResources(fd);
    if (!resources || !resources->count_planes ||
        resources->count_planes > MAX_PLANES) {
        fputs("MELANIA_FB2_PLANE_LIST_INCOMPLETE=true\n", stderr);
        if (resources) drmModeFreePlaneResources(resources);
        close(fd);
        return 1;
    }

    uint32_t visited[MAX_FBS] = {0};
    unsigned seen = 0, acquired = 0, released = 0;
    int ok = 1;
    for (uint32_t index = 0; index < resources->count_planes; ++index) {
        drmModePlanePtr plane = drmModeGetPlane(fd, resources->planes[index]);
        if (!plane) { ok = 0; break; }
        uint32_t fb_id = plane->fb_id;
        drmModeFreePlane(plane);
        if (!fb_id) continue;

        int duplicate = 0;
        for (unsigned i = 0; i < seen; ++i)
            if (visited[i] == fb_id) duplicate = 1;
        if (duplicate) continue;
        if (seen >= MAX_FBS || acquired > MAX_GEM_HANDLES - 4U) {
            ok = 0;
            break;
        }
        visited[seen++] = fb_id;
        if (inspect_fb(fd, fb_id, &acquired, &released) != 0) {
            ok = 0;
            break;
        }
    }

    drmModeFreePlaneResources(resources);
    close(fd);
    printf("MELANIA_FB2_OBSERVED_FRAMEBUFFERS=%u\n", seen);
    printf("MELANIA_FB2_GEM_HANDLES_ACQUIRED=%u\n", acquired);
    printf("MELANIA_FB2_GEM_HANDLES_CLOSED=%u\n", released);
    printf("MELANIA_FB2_NO_SCANOUT_MUTATION=true\n");
    /* No kernel FB ID is retained after closing the fd, so this explicitly
       does NOT qualify as a correct atomic rollback implementation. */
    printf("MELANIA_FB2_ROLLBACK_READY=false\n");
    if (!ok || !seen || !acquired || acquired != released) {
        fputs("MELANIA_FB2_ROUNDTRIP=FAIL\n", stderr);
        return 1;
    }
    puts("MELANIA_FB2_ROUNDTRIP=PASS");
    return 0;
}
