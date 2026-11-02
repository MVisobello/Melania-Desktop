/*
 * Disposable QEMU-only visible KMS scanout diagnostic.
 *
 * NOT the Melania compositor: this is a deliberately separate, short-lived
 * libdrm experiment with the original CRTC state restored before exit.
 * The current melania-native Rust rollback gate must remain closed.
 *
 * Never run on a host GPU or an everyday VM. Requires QEMU DMI, dedicated
 * /dev/ttyN, explicit opt-in, no graphical session and exactly one CRTC.
 */
#define _GNU_SOURCE
#include <drm.h>
#include <drm_mode.h>
#include <xf86drm.h>
#include <xf86drmMode.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t stop = 0;
static void request_stop(int signal_number) { (void) signal_number; stop = 1; }

static int is_qemu(void) {
    const char *files[] = {"/sys/class/dmi/id/sys_vendor",
                           "/sys/class/dmi/id/product_name"};
    for (size_t i=0;i<2;i++) {
        FILE *f=fopen(files[i],"r");
        if (!f) continue;
        char buf[256]={0};
        int match=fgets(buf,sizeof buf,f)!=NULL &&
            (strstr(buf,"QEMU") || strstr(buf,"Bochs") || strstr(buf,"KVM"));
        fclose(f);
        if (match) return 1;
    }
    return 0;
}
static int approved_guest(void) {
    const char *yes=getenv("MELANIA_QEMU_VISIBLE_FRAME");
    if (!yes || strcmp(yes,"YES_DISPOSABLE_QEMU") ||
        !is_qemu() || getenv("DISPLAY") || getenv("WAYLAND_DISPLAY") ||
        geteuid()!=0 || !isatty(STDIN_FILENO))
        return 0;
    char *tty=ttyname(STDIN_FILENO);
    if (!tty || strncmp(tty,"/dev/tty",8)) return 0;
    if (tty[8]<'1' || tty[8]>'9') return 0;
    for (char *p=tty+8;*p;p++) if (*p<'0'||*p>'9') return 0;
    return 1;
}
static int same_crtc(const drmModeCrtc *a, const drmModeCrtc *b) {
    return a && b && a->crtc_id==b->crtc_id &&
        a->buffer_id==b->buffer_id && a->x==b->x && a->y==b->y &&
        a->mode_valid==b->mode_valid &&
        a->mode.hdisplay==b->mode.hdisplay &&
        a->mode.vdisplay==b->mode.vdisplay &&
        a->mode.vrefresh==b->mode.vrefresh;
}

int main(void) {
    if (!approved_guest()) {
        fputs("MELANIA_QEMU_VISIBLE_BLOCKED=QEMU_DMI_TTY_OPTIN_REQUIRED\n",stderr);
        return 78;
    }
    struct sigaction sa={0};
    sa.sa_handler=request_stop;
    sigemptyset(&sa.sa_mask);
    sigaction(SIGINT,&sa,NULL);
    sigaction(SIGTERM,&sa,NULL);
    sigaction(SIGHUP,&sa,NULL);

    int fd=-1, success=0, presented=0, restored=0;
    drmModeRes *resources=NULL;
    drmModeCrtc *before=NULL,*after=NULL;
    uint32_t connectors[8]={0};
    int count=0;
    struct drm_mode_create_dumb dumb={0};
    uint32_t new_fb=0;
    void *pixels=MAP_FAILED;
    fd=open("/dev/dri/card0",O_RDWR|O_CLOEXEC|O_NOCTTY);
    if (fd<0) {perror("open guest DRM");goto done;}
    resources=drmModeGetResources(fd);
    if (!resources || resources->count_crtcs!=1 ||
        resources->count_connectors<1 || resources->count_connectors>8) {
        fputs("MELANIA_QEMU_VISIBLE_FAIL=ONE_CRTC_GUEST_REQUIRED\n",stderr);
        goto done;
    }
    before=drmModeGetCrtc(fd,resources->crtcs[0]);
    if (!before || !before->mode_valid || !before->buffer_id ||
        before->mode.hdisplay<640 || before->mode.hdisplay>1920 ||
        before->mode.vdisplay<480 || before->mode.vdisplay>1080) {
        fputs("MELANIA_QEMU_VISIBLE_FAIL=ACTIVE_ORIGINAL_FB_MODE_REQUIRED\n",stderr);
        goto done;
    }
    for (int i=0;i<resources->count_connectors;i++) {
        drmModeConnector *connector=drmModeGetConnector(fd,resources->connectors[i]);
        if (!connector) goto done;
        if (connector->encoder_id) {
            drmModeEncoder *encoder=drmModeGetEncoder(fd,connector->encoder_id);
            if (encoder) {
                if (encoder->crtc_id==before->crtc_id)
                    connectors[count++]=connector->connector_id;
                drmModeFreeEncoder(encoder);
            }
        }
        drmModeFreeConnector(connector);
    }
    if (count!=1) {
        fputs("MELANIA_QEMU_VISIBLE_FAIL=EXACTLY_ONE_ACTIVE_OUTPUT_REQUIRED\n",stderr);
        goto done;
    }

    /* Create a genuinely scanned-out XRGB8888 dumb framebuffer in the VM. */
    dumb.width=before->mode.hdisplay;
    dumb.height=before->mode.vdisplay;
    dumb.bpp=32;
    if (ioctl(fd,DRM_IOCTL_MODE_CREATE_DUMB,&dumb)<0) {
        perror("DRM create dumb");goto done;
    }
    if (drmModeAddFB(fd,dumb.width,dumb.height,24,32,dumb.pitch,
                     dumb.handle,&new_fb)<0) {
        perror("DRM add FB");goto done;
    }
    struct drm_mode_map_dumb map={0};
    map.handle=dumb.handle;
    if (ioctl(fd,DRM_IOCTL_MODE_MAP_DUMB,&map)<0) {
        perror("DRM map dumb");goto done;
    }
    pixels=mmap(NULL,dumb.size,PROT_READ|PROT_WRITE,MAP_SHARED,fd,map.offset);
    if (pixels==MAP_FAILED) {perror("mmap");goto done;}
    for (uint32_t y=0;y<dumb.height;y++) {
        uint32_t *row=(uint32_t*)((unsigned char*)pixels+y*dumb.pitch);
        for (uint32_t x=0;x<dumb.width;x++) {
            /* Bright magenta left / cyan right, distinctive QMP screenshot. */
            row[x]=(x<dumb.width/2) ? 0x00f020b0u : 0x0010e8e0u;
        }
    }
    /* GEM dumb-buffer mmap is not a regular file mapping: msync may return
       EINVAL on virtio_gpu. The KMS set-crtc follows these CPU writes. */
    __sync_synchronize();
    if (stop) goto done;

    /* Uses the existing mode/CRTC/connector, no host or other output. */
    if (drmModeSetCrtc(fd,before->crtc_id,new_fb,before->x,before->y,
                       connectors,count,&before->mode)<0) {
        perror("VM KMS set CRTC");goto done;
    }
    presented=1;
    after=drmModeGetCrtc(fd,before->crtc_id);
    if (!after || after->buffer_id!=new_fb || !after->mode_valid) {
        fputs("MELANIA_QEMU_VISIBLE_FAIL=SCANOUT_STATE_NOT_OBSERVED\n",stderr);
        goto done;
    }
    drmModeFreeCrtc(after);
    after=NULL;
    puts("MELANIA_QEMU_VISIBLE_SCANOUT_ACTIVE=true");
    fflush(stdout);
    /* Bounded interval for QMP screendump; graceful TERM still restores. */
    for (int tick=0;tick<80 && !stop;tick++) {
        struct timespec pause={.tv_sec=0,.tv_nsec=100000000};
        nanosleep(&pause,NULL);
    }
    success=1;

done:
    /* Always attempt rollback once an actual modeset was submitted. Keep the
       old FB bound until restoration has been verified. */
    if (presented) {
        if (drmModeSetCrtc(fd,before->crtc_id,before->buffer_id,
                           before->x,before->y,connectors,count,
                           &before->mode)==0) {
            drmModeCrtc *original=drmModeGetCrtc(fd,before->crtc_id);
            restored=same_crtc(before,original);
            if (original) drmModeFreeCrtc(original);
        }
        printf("MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED=%s\n",
               restored?"true":"false");
    }
    if (pixels!=MAP_FAILED) munmap(pixels,dumb.size);
    if (new_fb) drmModeRmFB(fd,new_fb);
    if (dumb.handle) {
        struct drm_mode_destroy_dumb destroy={.handle=dumb.handle};
        if (ioctl(fd,DRM_IOCTL_MODE_DESTROY_DUMB,&destroy)<0)
            fputs("MELANIA_QEMU_VISIBLE_WARN=DUMB_DESTROY_FAILED\n",stderr);
    }
    if (after) drmModeFreeCrtc(after);
    if (before) drmModeFreeCrtc(before);
    if (resources) drmModeFreeResources(resources);
    if (fd>=0) close(fd);
    if (!success || !restored) {
        fputs("MELANIA_QEMU_VISIBLE_RESULT=FAIL\n",stderr);
        return 1;
    }
    puts("MELANIA_QEMU_VISIBLE_RESULT=PASS");
    puts("MELANIA_NATIVE_COMPOSITOR_LOGIN_READY=false");
    return 0;
}
