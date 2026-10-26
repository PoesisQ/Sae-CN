#include "internal.h"

#ifdef _WIN32
static void sae_unmap_font(void *opaque, unsigned char *data)
{
    (void)opaque;
    UnmapViewOfFile(data);
}
#endif

/* Read-only mapping belongs to the fz_buffer, including every native font clone. */
fz_font *mupdf_new_font_from_file_mapped(fz_context *ctx, const char *name, int index, const char *path, mupdf_error_t **errptr)
{
    fz_font *font = NULL;
    fz_buffer *buffer = NULL;
#ifdef _WIN32
    HANDLE file = INVALID_HANDLE_VALUE, mapping = NULL;
    unsigned char *data = NULL;
    wchar_t *wide = NULL;
    LARGE_INTEGER length;
    fz_var(file); fz_var(mapping); fz_var(data); fz_var(wide); fz_var(buffer);
    fz_try(ctx)
    {
        int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
        if (!count) fz_throw(ctx, FZ_ERROR_ARGUMENT, "invalid UTF-8 font path");
        wide = fz_malloc_array(ctx, count, wchar_t);
        if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, count))
            fz_throw(ctx, FZ_ERROR_ARGUMENT, "invalid UTF-8 font path");
        file = CreateFileW(wide, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
        if (file == INVALID_HANDLE_VALUE || !GetFileSizeEx(file, &length) || length.QuadPart <= 0 || length.QuadPart > 128 * 1024 * 1024)
            fz_throw(ctx, FZ_ERROR_SYSTEM, "cannot open mapped font");
        mapping = CreateFileMappingW(file, NULL, PAGE_READONLY, 0, 0, NULL);
        if (!mapping || !(data = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0)))
            fz_throw(ctx, FZ_ERROR_SYSTEM, "cannot map font");
        buffer = fz_new_buffer_from_shared_data(ctx, data, (size_t)length.QuadPart);
        buffer->drop_storage = sae_unmap_font;
        data = NULL;
        font = fz_new_font_from_buffer(ctx, name, buffer, index, 0);
    }
    fz_always(ctx)
    {
        fz_drop_buffer(ctx, buffer);
        if (data) UnmapViewOfFile(data);
        if (mapping) CloseHandle(mapping);
        if (file != INVALID_HANDLE_VALUE) CloseHandle(file);
        fz_free(ctx, wide);
    }
    fz_catch(ctx) { mupdf_save_error(ctx, errptr); }
#else
    TRY_CATCH(fz_font*, NULL, fz_new_font_from_file(ctx, name, path, index, 0));
#endif
    return font;
}

/* Font */
fz_font *mupdf_new_font(fz_context *ctx, const char *name, int index, mupdf_error_t **errptr)
{
    fz_font *font = NULL;
    fz_try(ctx)
    {
        const unsigned char *data;
        int size;

        data = fz_lookup_base14_font(ctx, name, &size);
        if (data)
        {
            font = fz_new_font_from_memory(ctx, name, data, size, index, 0);
        }
        else
        {
            font = fz_new_font_from_file(ctx, name, name, index, 0);
        }
    }
    fz_catch(ctx)
    {
        mupdf_save_error(ctx, errptr);
    }
    return font;
}

fz_font *mupdf_new_font_from_buffer(fz_context *ctx, const char *name, int index, fz_buffer *buffer, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_font*, NULL, fz_new_font_from_buffer(ctx, name, buffer, index, 0));
}

fz_font *mupdf_new_font_from_memory(fz_context *ctx, const char *name, int index, const unsigned char *data, int len, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_font*, NULL, fz_new_font_from_memory(ctx, name, data, len, index, 0));
}

fz_font *mupdf_new_cjk_font(fz_context *ctx, int ordering, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_font*, NULL, fz_new_cjk_font(ctx, ordering));
}

int mupdf_encode_character(fz_context *ctx, fz_font *font, int unicode, mupdf_error_t **errptr)
{
    TRY_CATCH(int, 0, fz_encode_character(ctx, font, unicode));
}

float mupdf_advance_glyph(fz_context *ctx, fz_font *font, int glyph, bool wmode, mupdf_error_t **errptr)
{
    TRY_CATCH(float, 0.0, fz_advance_glyph(ctx, font, glyph, wmode));
}

fz_path *mupdf_outline_glyph(fz_context *ctx, fz_font *font, int glyph, fz_matrix ctm, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_path*, NULL, fz_outline_glyph(ctx, font, glyph, ctm));
}
