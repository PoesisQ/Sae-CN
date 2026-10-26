/* Bounded per-thread native parsing. No persistent allocation or worker. */
#define SAE_ALLOC_IMPLEMENTATION
#include "kindle_alloc.h"
#include <string.h>
#ifdef _MSC_VER
#define SAE_TLS __declspec(thread)
#else
#define SAE_TLS _Thread_local
#endif
#define SAE_KINDLE_LIMIT (96u * 1024u * 1024u)
typedef union Header Header;
union Header { struct { size_t size; Header *previous, *next; } info; _Alignas(16) char alignment[16]; };
static SAE_TLS size_t allocated;
static SAE_TLS int limited;
static SAE_TLS Header *first;
size_t sae_kindle_allocated(void) { return allocated; }
int sae_kindle_limited(void) { return limited; }
void sae_kindle_begin(void) { limited = 0; }
void sae_kindle_end(void) {
    // Also reclaim allocations abandoned by upstream on malformed-input paths.
    while (first) { Header *next = first->info.next; free(first); first = next; }
    allocated = 0;
}
void *sae_kindle_malloc(size_t size) {
    if (size > SIZE_MAX - sizeof(Header) || size + sizeof(Header) > SAE_KINDLE_LIMIT - allocated) {
        limited = 1; return NULL;
    }
    Header *p = malloc(sizeof(Header) + size);
    if (!p) return NULL;
    p->info.size = size; p->info.previous = NULL; p->info.next = first;
    if (first) first->info.previous = p;
    first = p; allocated += size + sizeof(Header);
    return p + 1;
}
void sae_kindle_free(void *pointer) {
    if (pointer) {
        Header *p = (Header *)pointer - 1;
        if (p->info.previous) p->info.previous->info.next = p->info.next;
        else first = p->info.next;
        if (p->info.next) p->info.next->info.previous = p->info.previous;
        allocated -= p->info.size + sizeof(Header); free(p);
    }
}
void *sae_kindle_calloc(size_t count, size_t size) {
    if (count && size > SIZE_MAX / count) { limited = 1; return NULL; }
    void *p = sae_kindle_malloc(count * size);
    if (p) memset(p, 0, count * size);
    return p;
}
void *sae_kindle_realloc(void *pointer, size_t size) {
    if (!pointer) return sae_kindle_malloc(size);
    if (!size) { sae_kindle_free(pointer); return NULL; }
    Header *old = (Header *)pointer - 1;
    if (size > SAE_KINDLE_LIMIT - allocated + old->info.size || size > SIZE_MAX - sizeof(Header)) {
        limited = 1; return NULL;
    }
    const size_t old_size = old->info.size;
    Header *p = realloc(old, sizeof(Header) + size);
    if (!p) return NULL;
    if (p->info.previous) p->info.previous->info.next = p;
    else first = p;
    if (p->info.next) p->info.next->info.previous = p;
    p->info.size = size; allocated = allocated - old_size + size;
    return p + 1;
}
