package dev.horizon.android;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import java.io.FileNotFoundException;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Scoped, read-only access to gallery screenshots.
 *
 * <p>URIs use an opaque gallery ID ({@code content://…/p0/name.png}); the Rust
 * side validates that ID against the instance's screenshots folder. Sharing
 * uses the URI with a temporary read grant, so no private path is exposed.
 */
public final class ScreenshotProvider extends ContentProvider {
    private static final String[] COLUMNS = {
        "_display_name", "_size", "last_modified", "mime_type"
    };

    private static native String nativeCall(String home, String operation, String id)
            throws FileNotFoundException;
    private static native int nativeOpen(String home, String id)
            throws FileNotFoundException;

    @Override public boolean onCreate() {
        System.loadLibrary("horizon_android");
        return true;
    }

    private String home() {
        return getContext().getFilesDir().getAbsolutePath();
    }

    static String id(Uri uri) {
        String path = uri.getPath();
        if (path == null || path.length() < 2) {
            throw new IllegalArgumentException("Invalid screenshot URI");
        }
        return path.substring(1);
    }

    private JSONObject inspect(String id) throws FileNotFoundException {
        String text = nativeCall(home(), "inspect", id);
        if (text == null) throw new FileNotFoundException("Screenshot is no longer available");
        try {
            JSONObject data = new JSONObject(text);
            String error = data.optString("error", "");
            if (!error.isEmpty()) throw new FileNotFoundException(error);
            return data;
        } catch (JSONException error) {
            throw new FileNotFoundException("Invalid screenshot metadata");
        }
    }

    private JSONObject inspect(Uri uri) throws FileNotFoundException {
        return inspect(id(uri));
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection,
            String[] selectionArgs, String sortOrder) {
        String[] columns = projection == null ? COLUMNS : projection;
        MatrixCursor cursor = new MatrixCursor(columns);
        try {
            JSONObject data = inspect(uri);
            MatrixCursor.RowBuilder row = cursor.newRow();
            for (String column : columns) {
                Object value = data.opt(column);
                row.add(value == JSONObject.NULL ? null : value);
            }
            return cursor;
        } catch (FileNotFoundException | IllegalArgumentException error) {
            // A share target may still query after the player deleted the file.
            cursor.close();
            return new MatrixCursor(columns);
        }
    }

    @Override public String getType(Uri uri) {
        try {
            return inspect(uri).optString("mime_type", "application/octet-stream");
        } catch (FileNotFoundException | IllegalArgumentException error) {
            return null;
        }
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode)
            throws FileNotFoundException {
        if (!"r".equals(mode)) {
            throw new FileNotFoundException("Screenshots are read-only");
        }
        final int fd;
        try {
            fd = nativeOpen(home(), id(uri));
        } catch (IllegalArgumentException error) {
            throw new FileNotFoundException("Invalid screenshot URI");
        }
        if (fd < 0) throw new FileNotFoundException("Screenshot is no longer available");
        return ParcelFileDescriptor.adoptFd(fd);
    }

    @Override public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException("Screenshots are read-only");
    }

    @Override public int delete(Uri uri, String selection, String[] selectionArgs) {
        return 0;
    }

    @Override public int update(Uri uri, ContentValues values, String selection,
            String[] selectionArgs) {
        return 0;
    }
}
