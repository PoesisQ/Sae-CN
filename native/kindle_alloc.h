/* Sae Reader, AGPL-3.0-or-later. Injected only into the Kindle parser. */
#ifndef SAE_KINDLE_ALLOC_H
#define SAE_KINDLE_ALLOC_H
#include <stdlib.h>
#include <stdint.h>
#include <stddef.h>
void *sae_kindle_malloc(size_t size);
void *sae_kindle_calloc(size_t count, size_t size);
void *sae_kindle_realloc(void *pointer, size_t size);
void sae_kindle_free(void *pointer);
size_t sae_kindle_allocated(void);
int sae_kindle_limited(void);
void sae_kindle_begin(void);
void sae_kindle_end(void);
#ifndef SAE_ALLOC_IMPLEMENTATION
#define malloc sae_kindle_malloc
#define calloc sae_kindle_calloc
#define realloc sae_kindle_realloc
#define free sae_kindle_free
#endif
#endif
