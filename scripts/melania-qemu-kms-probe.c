/* Read-only DRM/KMS capability probe, compiled for the disposable QEMU guest.
 *
 * Opening a card with O_RDWR is required by some DRM drivers but this process
 * never issues modesets, atomic commits, framebuffer creation, property
 * writes or page flips. Only an isolated QEMU guest may execute this binary.
 */
#include <drm.h>
#include <drm_mode.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

static int probe_card(const char *path) {
    int fd = open(path, O_RDWR | O_CLOEXEC);
    if (fd < 0) {
        return -1;
    }

    struct drm_get_cap dumb = { .capability = DRM_CAP_DUMB_BUFFER };
    struct drm_set_client_cap atomic = {
        .capability = DRM_CLIENT_CAP_ATOMIC,
        .value = 1
    };
    struct drm_mode_card_res resources = {0};
    int has_dumb = ioctl(fd, DRM_IOCTL_GET_CAP, &dumb) == 0 && dumb.value != 0;
    int has_atomic = ioctl(fd, DRM_IOCTL_SET_CLIENT_CAP, &atomic) == 0;
    int has_resources = ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &resources) == 0;

    printf("MELANIA_QEMU_DRM_CARD=%s\n", path);
    printf("MELANIA_QEMU_DRM_DUMB_BUFFER=%s\n", has_dumb ? "true" : "false");
    printf("MELANIA_QEMU_KMS_ATOMIC=%s\n", has_atomic ? "true" : "false");
    if (has_resources) {
        printf("MELANIA_QEMU_KMS_CRTCS=%u\n", resources.count_crtcs);
        printf("MELANIA_QEMU_KMS_CONNECTORS=%u\n", resources.count_connectors);
        printf("MELANIA_QEMU_KMS_PLANES_NOT_MODIFIED=true\n");
    } else {
        printf("MELANIA_QEMU_KMS_RESOURCES_ERROR=%s\n", strerror(errno));
    }
    fflush(stdout);
    close(fd);
    return has_resources && has_atomic &&
           resources.count_crtcs > 0 && resources.count_connectors > 0
           ? 0 : 1;
}

int main(void) {
    for (int i = 0; i < 8; i++) {
        char path[64];
        snprintf(path, sizeof(path), "/dev/dri/card%d", i);
        if (access(path, F_OK) == 0) {
            int status = probe_card(path);
            printf("MELANIA_QEMU_KMS_READ_ONLY_PASS=%s\n",
                   status == 0 ? "true" : "false");
            return status;
        }
    }
    fputs("MELANIA_QEMU_KMS_READ_ONLY_PASS=false\n", stdout);
    fputs("MELANIA_QEMU_KMS_ERROR=no_DRM_card\n", stdout);
    return 2;
}
