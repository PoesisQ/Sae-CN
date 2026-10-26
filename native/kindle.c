/* A narrow streaming bridge to the vendored LGPL libmobi parser.
 * Rust owns the temporary archive. No native structs cross the FFI boundary.
 * Encryption, document writing, multimedia playback and tools are not built.
 */
#include <stdio.h>
#include <wchar.h>
#include "mobi.h"
#include "kindle_alloc.h"
typedef int (*Emit)(void *, const char *, const unsigned char *, size_t);
static int emit_parts(Emit emit, void *user, MOBIPart *part, const char *prefix) {
    size_t count = 0;
    for (; part; part = part->next) {
        if (++count > 10000 || part->size > 32u * 1024u * 1024u) return -3;
        if (!part->size) continue;
        MOBIFileMeta meta = mobi_get_filemeta_by_type(part->type);
        char name[96];
        if (part->type == T_OPF) snprintf(name, sizeof(name), "content.opf");
        else snprintf(name, sizeof(name), "%s%05zu.%s", prefix, part->uid, meta.extension);
        if (emit(user, name, part->data, part->size)) return -4;
    }
    return 0;
}
#ifdef _WIN32
int sae_kindle_convert(const wchar_t *path, Emit emit, void *user) {
    FILE *file = _wfopen(path, L"rb");
#else
int sae_kindle_convert(const char *path, Emit emit, void *user) {
    FILE *file = fopen(path, "rb");
#endif
    if (!file) return -1;
    // Parsing and destruction stay on the calling document thread.
    if (sae_kindle_allocated()) { fclose(file); return -5; }
    sae_kindle_begin();
    MOBIData *mobi = mobi_init();
    if (!mobi) { fclose(file); return -3; }
    int result = mobi_load_file(mobi, file);
    fclose(file);
    MOBIRawml *raw = NULL;
    if (result == MOBI_SUCCESS && mobi_is_encrypted(mobi)) result = -2;
    if (result == MOBI_SUCCESS) {
        raw = mobi_init_rawml(mobi);
        result = raw ? mobi_parse_rawml(raw, mobi) : MOBI_MALLOC_FAILED;
    }
    if (!result) result = emit_parts(emit, user, raw->markup, "part");
    if (!result && raw->flow) result = emit_parts(emit, user, raw->flow->next, "flow");
    if (!result) result = emit_parts(emit, user, raw->resources, "resource");
    mobi_free_rawml(raw);
    mobi_free(mobi);
    if (sae_kindle_limited()) result = -3;
    sae_kindle_end();
    return result;
}
