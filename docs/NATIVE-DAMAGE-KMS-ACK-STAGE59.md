# Stage59 — Ritiro dei dirty rectangles solo dopo CRTC ACK

Il compositore di prova QEMU marca le regioni di danno della scena come presentate **soltanto** dopo che `ContinuousRedraw::presented` ha accettato il ticket del frame verificato tramite `CRTC FB_ID`. Non vengono eliminate quando una richiesta DRM viene inviata, quando fallisce o quando arriva un ACK obsoleto. Un nuovo commit Wayland non perde mai la propria invalidazione mentre il frame precedente è in volo.

La composizione attuale rimane full-frame: il partial rendering sarà oggetto di un passo separato. `NATIVE_SESSION_READY=false`.
