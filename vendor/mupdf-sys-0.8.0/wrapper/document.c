#include "internal.h"

/* Sae Reader native extensions, AGPL-3.0-or-later, 2026-10-25.
 * The native 1.27.2 library predates fz_style_document. Invalidate its
 * document-specific HTML cache after changing the serialized worker's CSS.
 * All potentially throwing operations remain inside C fz_try/fz_catch.
 */
void fz_purge_stored_html(fz_context *ctx, void *doc);
uint64_t sae_epub_anchor_make(fz_context *ctx, fz_document *doc, fz_location loc);
fz_location sae_epub_anchor_lookup(fz_context *ctx, fz_document *doc, uint64_t anchor);
uint64_t sae_htdoc_anchor_make(fz_context *ctx, fz_document *doc, fz_location loc);
fz_location sae_htdoc_anchor_lookup(fz_context *ctx, fz_document *doc, uint64_t anchor);

void mupdf_sae_set_style(fz_context *ctx, fz_document *doc, const char *css, bool publisher_css, mupdf_error_t **errptr)
{
    fz_try(ctx)
    {
        fz_set_user_css(ctx, css);
        fz_set_use_document_css(ctx, publisher_css);
        fz_purge_stored_html(ctx, doc);
    }
    fz_catch(ctx) { mupdf_save_error(ctx, errptr); }
}

uint64_t mupdf_sae_make_anchor(fz_context *ctx, fz_document *doc, int page, mupdf_error_t **errptr)
{
    uint64_t result = 0;
    fz_try(ctx)
    {
        fz_location loc = fz_location_from_page_number(ctx, doc, page);
        result = sae_epub_anchor_make(ctx, doc, loc);
        if (result == 0) result = sae_htdoc_anchor_make(ctx, doc, loc);
        if (result == 0) result = (uint64_t)page + 1;
    }
    fz_catch(ctx) { mupdf_save_error(ctx, errptr); }
    return result;
}

int mupdf_sae_lookup_anchor(fz_context *ctx, fz_document *doc, uint64_t anchor, mupdf_error_t **errptr)
{
    int result = -1;
    fz_try(ctx)
    {
        if (anchor >> 32) {
            fz_location loc = sae_epub_anchor_lookup(ctx, doc, anchor);
            if (loc.chapter < 0) loc = sae_htdoc_anchor_lookup(ctx, doc, anchor);
            if (loc.chapter >= 0 && loc.page >= 0)
                result = fz_page_number_from_location(ctx, doc, loc);
        } else if (anchor > 0 && anchor <= (uint64_t)fz_count_pages(ctx, doc)) {
            result = (int)anchor - 1;
        }
    }
    fz_catch(ctx) { mupdf_save_error(ctx, errptr); }
    return result;
}

int mupdf_sae_chapter(fz_context *ctx, fz_document *doc, int page, mupdf_error_t **errptr)
{
    int result = -1;
    fz_try(ctx) { result = fz_location_from_page_number(ctx, doc, page).chapter; }
    fz_catch(ctx) { mupdf_save_error(ctx, errptr); }
    return result;
}

void sae_epub_set_fixed_layout(fz_context *ctx, fz_document *doc);
void mupdf_sae_set_fixed_layout(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH_VOID(sae_epub_set_fixed_layout(ctx, doc));
}

/* Document */
fz_document *mupdf_open_document(fz_context *ctx, const char *filename, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_document*, NULL, fz_open_document(ctx, filename));
}

fz_document *mupdf_open_document_from_bytes(fz_context *ctx, fz_buffer *bytes, const char *magic, mupdf_error_t **errptr)
{
    if (!magic)
    {
        return NULL;
    }
    fz_document *doc = NULL;
    fz_stream *stream = NULL;
    fz_var(stream);
    fz_try(ctx)
    {
        stream = fz_open_buffer(ctx, bytes);
        doc = fz_open_document_with_stream(ctx, magic, stream);
    }
    fz_always(ctx)
    {
        fz_drop_stream(ctx, stream);
    }
    fz_catch(ctx)
    {
        mupdf_save_error(ctx, errptr);
    }
    return doc;
}

bool mupdf_recognize_document(fz_context *ctx, const char *magic, mupdf_error_t **errptr)
{
    if (!magic)
    {
        return false;
    }
    TRY_CATCH(bool, false, fz_recognize_document(ctx, magic) != NULL);
}

bool mupdf_needs_password(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH(bool, false, fz_needs_password(ctx, doc));
}

bool mupdf_authenticate_password(fz_context *ctx, fz_document *doc, const char *password, mupdf_error_t **errptr)
{
    TRY_CATCH(bool, false, fz_authenticate_password(ctx, doc, password));
}

int mupdf_document_page_count(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH(int, 0, fz_count_pages(ctx, doc));
}

/* Sae Reader: bounded chapter-at-a-time pagination, cancel between chapters. */
int mupdf_sae_chapter_count(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH(int, 0, fz_count_chapters(ctx, doc));
}
int mupdf_sae_chapter_pages(fz_context *ctx, fz_document *doc, int chapter, mupdf_error_t **errptr)
{
    TRY_CATCH(int, 0, fz_count_chapter_pages(ctx, doc, chapter));
}

char *mupdf_lookup_metadata(fz_context *ctx, fz_document *doc, const char *key, mupdf_error_t **errptr)
{
    int len;
    char *value = NULL;
    fz_try(ctx)
    {
        len = fz_lookup_metadata(ctx, doc, key, NULL, 0) + 1;
        if (len > 1)
        {
            value = calloc(len, sizeof(char));
            fz_lookup_metadata(ctx, doc, key, value, len);
        }
    }
    fz_catch(ctx)
    {
        mupdf_save_error(ctx, errptr);
    }
    return value;
}

bool mupdf_is_document_reflowable(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH(bool, false, fz_is_document_reflowable(ctx, doc));
}

void mupdf_layout_document(fz_context *ctx, fz_document *doc, float w, float h, float em, mupdf_error_t **errptr)
{
    fz_try(ctx)
    {
        if (!fz_is_document_reflowable(ctx, doc))
        {
            return;
        }
        if (w <= 0.0f || h <= 0.0f)
        {
            *errptr = mupdf_new_error_from_str("invalid width or height");
            return;
        }
        fz_layout_document(ctx, doc, w, h, em);
    }
    fz_catch(ctx)
    {
        mupdf_save_error(ctx, errptr);
    }
}

fz_page *mupdf_load_page(fz_context *ctx, fz_document *doc, int page_no, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_page*, NULL, fz_load_page(ctx, doc, page_no));
}

static pdf_document *mupdf_convert_to_pdf_internal(fz_context *ctx, fz_document *doc, int fp, int tp, int rotate, fz_cookie *cookie)
{
    pdf_document *pdfout = pdf_create_document(ctx);
    int i, incr = 1, s = fp, e = tp;
    if (fp > tp) // revert page sequence
    {
        incr = -1;
        s = tp;
        e = fp;
    }
    fz_rect mediabox;
    fz_device *dev = NULL;
    fz_buffer *contents = NULL;
    pdf_obj *resources = NULL;
    fz_page *page;
    fz_var(dev);
    fz_var(contents);
    fz_var(resources);
    fz_var(page);
    for (i = fp; i >= s && i <= e; i += incr)
    {
        fz_try(ctx)
        {
            page = fz_load_page(ctx, doc, i);
            mediabox = fz_bound_page(ctx, page);
            dev = pdf_page_write(ctx, pdfout, mediabox, &resources, &contents);
            fz_run_page(ctx, page, dev, fz_identity, cookie);
            fz_close_device(ctx, dev);
            fz_drop_device(ctx, dev);
            dev = NULL;
            pdf_obj *page_obj = pdf_add_page(ctx, pdfout, mediabox, rotate, resources, contents);
            pdf_insert_page(ctx, pdfout, -1, page_obj);
            pdf_drop_obj(ctx, page_obj);
        }
        fz_always(ctx)
        {
            pdf_drop_obj(ctx, resources);
            fz_drop_buffer(ctx, contents);
            fz_drop_device(ctx, dev);
            fz_drop_page(ctx, page);
        }
        fz_catch(ctx)
        {
            fz_rethrow(ctx);
        }
    }
    return pdfout;
}

pdf_document *mupdf_convert_to_pdf(fz_context *ctx, fz_document *doc, int fp, int tp, int rotate, fz_cookie *cookie, mupdf_error_t **errptr)
{
    if (rotate % 90)
    {
        *errptr = mupdf_new_error_from_str("rotation not multiple of 90");
        return NULL;
    }

    TRY_CATCH(pdf_document*, NULL, mupdf_convert_to_pdf_internal(ctx, doc, fp, tp, rotate, cookie));
}

fz_location mupdf_resolve_link(fz_context *ctx, fz_document *doc, const char *uri, float *xp, float *yp, mupdf_error_t **errptr)
{
    fz_location ret = { -1, -1 };
    fz_try(ctx)
    {
        ret = fz_resolve_link(ctx, doc, uri, xp, yp);
    }
    fz_catch(ctx)
    {
        mupdf_save_error(ctx, errptr);
    }
    return ret;
}


fz_link_dest mupdf_resolve_link_dest(fz_context *ctx, fz_document *doc, const char *uri, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_link_dest, {}, fz_resolve_link_dest(ctx, doc, uri));
}

fz_colorspace *mupdf_document_output_intent(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_colorspace*, NULL, fz_document_output_intent(ctx, doc));
}

fz_outline *mupdf_load_outline(fz_context *ctx, fz_document *doc, mupdf_error_t **errptr)
{
    TRY_CATCH(fz_outline*, NULL, fz_load_outline(ctx, doc));
}
