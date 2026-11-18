package dev.horizon.android.launcher

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import dev.horizon.android.R

/** True when this install can read and write the shared storage folder. */
fun hasAllFilesAccess(context: Context): Boolean =
    if (Build.VERSION.SDK_INT >= 30) {
        Environment.isExternalStorageManager()
    } else {
        ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.WRITE_EXTERNAL_STORAGE,
        ) == PackageManager.PERMISSION_GRANTED
    }

/**
 * Opens the per-app all-files-access page (Android 11+) or the application list
 * fallback for OEM skins that do not host it. Returns false if nothing could be
 * opened. Android 9/10 use the legacy permission instead (see the gate below).
 */
fun openAllFilesSettings(context: Context): Boolean {
    if (Build.VERSION.SDK_INT < 30) return true
    val intent = Intent(
        Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
        Uri.parse("package:${context.packageName}"),
    )
    if (runCatching { context.startActivity(intent) }.isFailure) {
        return runCatching {
            context.startActivity(Intent(Settings.ACTION_MANAGE_APPLICATIONS_SETTINGS))
        }.isSuccess
    }
    return true
}

/** The one shared folder, for every build: the dev/release distinction is gone. */
fun sharedStorageDir(): java.io.File =
    java.io.File(Environment.getExternalStorageDirectory(), "Horizon-Android")

/** Full-screen explanation shown until all-files access is granted. */
@Composable
fun StorageAccessGate(onRecheck: () -> Unit) {
    val context = LocalContext.current
    var error by remember { mutableStateOf(false) }
    val requestLegacy = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) onRecheck() else error = true
    }
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        Column(
            Modifier.fillMaxSize().padding(horizontal = 28.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            HorizonMark(Modifier.size(72.dp))
            Spacer(Modifier.height(20.dp))
            Text(
                stringResource(R.string.gate_title),
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(12.dp))
            Text(
                stringResource(R.string.gate_body),
                style = MaterialTheme.typography.bodyLarge,
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.widthIn(max = 520.dp),
            )
            Spacer(Modifier.height(8.dp))
            Text(
                stringResource(R.string.gate_step),
                style = MaterialTheme.typography.bodyMedium,
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.widthIn(max = 520.dp),
            )
            Spacer(Modifier.height(24.dp))
            FilledTonalButton(onClick = {
                if (Build.VERSION.SDK_INT >= 30) {
                    error = !openAllFilesSettings(context)
                } else {
                    requestLegacy.launch(Manifest.permission.WRITE_EXTERNAL_STORAGE)
                }
            }) {
                Text(stringResource(R.string.gate_button))
            }
            if (error) {
                Spacer(Modifier.height(12.dp))
                Text(
                    stringResource(R.string.gate_error),
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center,
                    color = MaterialTheme.colorScheme.error,
                )
            }
            Spacer(Modifier.height(16.dp))
            Text(
                stringResource(R.string.gate_status),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
            )
        }
    }
}
