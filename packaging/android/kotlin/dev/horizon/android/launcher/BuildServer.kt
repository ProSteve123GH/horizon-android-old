package dev.horizon.android.launcher

import android.content.ClipData
import android.content.ClipboardManager
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/**
 * Settings > Native mods > Host a build server: how to compile `.hnmod` mods with no machine of
 * your own. A Cloudflare Worker (free plan) in front of a GitHub Actions builder repository, so the
 * token that dispatches the runs never reaches the phone.
 */
@Composable
fun BuildServerGuide(state: Snapshot) {
    val context = LocalContext.current
    val copy: (String) -> Unit = { text ->
        context.getSystemService(ClipboardManager::class.java)
            ?.setPrimaryClip(ClipData.newPlainText("Horizon", text))
        Toast.makeText(context, "Copied", Toast.LENGTH_SHORT).show()
    }
    LazyColumn(
        Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        contentPadding = PaddingValues(bottom = 24.dp),
    ) {
        item(key = "intro") {
            GuideCard("Build native mods without a server") {
                Text(
                    "A native mod (.hnmod) is compiled into the game itself, so a phone can't build " +
                        "one. The launcher sends the mods to a build server and downloads the game it " +
                        "makes. This page sets one up with GitHub's machines doing the compile, behind " +
                        "a free Cloudflare Worker that keeps the token off the phone.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "You need a GitHub account and a Cloudflare account, both free, and about 15 " +
                        "minutes. The five steps below go in order: the builder repository first, " +
                        "then its token, then the Worker, then the launcher.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item(key = "status") {
            GuideCard("Your build server") {
                Text(
                    state.native.server.ifBlank { "Not set yet - step 4 below fills this in." },
                    style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                )
                Text(
                    if (state.native.github) {
                        "Compile on GitHub is on. The gateway hands every build to GitHub's runners."
                    } else {
                        "Compile on GitHub is off: a gateway that can build by itself (a machine of " +
                            "your own) uses that machine. The free Worker below always uses GitHub."
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item(key = "step1") {
            GuideCard("1. Make the builds repository") {
                Text(
                    "The compile runs in a separate public repository, never in the game's own. " +
                        "Create one on GitHub, for example <you>/horizon-mod-builds. Public matters: " +
                        "the phone downloads the finished game from its release assets without a token.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Copy everything in packaging/android/mod-builds/ into it, including the hidden " +
                        ".github folder, and commit it to main.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "In .github/workflows/base.yml and build.yml, set HORIZON_REPO to the game " +
                        "repository's URL and HORIZON_REPO_PATH to its owner/name. The template " +
                        "already names this fork.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Add each release to versions.json, for example " +
                        "{\"0.7.7\": \"<the release's full commit>\"}. The launcher normally sends the " +
                        "commit it was built from; this file is the fallback.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Then make the clean cache once: Actions > base > Run workflow. Set version to " +
                        "0.7.7 and commit to the release's full commit (or leave it empty to use " +
                        "versions.json). It takes about 15 minutes. Every player build starts from " +
                        "that cache; without it, the build stops on the cache restore.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        item(key = "step2") {
            GuideCard("2. Make the token") {
                Text(
                    "Create a fine-grained GitHub token for the builds repository only, with " +
                        "Actions: write. Nothing else is needed: the workflow reads the builds " +
                        "repository with its own token.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Keep it for step 3. It is a secret: the Worker stores it, the phone never sees it.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item(key = "step3") {
            GuideCard("3. Deploy the Worker") {
                Text(
                    "packaging/android/mod-build-worker/ is a Cloudflare Worker: free plan, no card, " +
                        "nothing to keep running. From that folder, with the wrangler in your PATH " +
                        "(npx wrangler runs the same commands):",
                    style = MaterialTheme.typography.bodyMedium,
                )
                GuideCode(
                    "wrangler login\n" +
                        "wrangler kv namespace create STORE",
                    copy,
                )
                Text(
                    "Put the printed id in wrangler.toml under [[kv_namespaces]] (binding STORE), " +
                        "and set GITHUB_REPO = \"<you>/horizon-mod-builds\" in [vars]. Then:",
                    style = MaterialTheme.typography.bodyMedium,
                )
                GuideCode(
                    "wrangler secret put GITHUB_TOKEN\n" +
                        "wrangler deploy",
                    copy,
                )
                Text(
                    "Paste the token from step 2 at the secret prompt. deploy prints the URL, " +
                        "https://<worker>.<account>.workers.dev: that is the build server. On Termux, " +
                        "use the wrangler in PATH for login and deploy (its local dev server needs " +
                        "workerd, which has no Android build), or run deploy.sh with " +
                        "CLOUDFLARE_API_TOKEN.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        item(key = "step4") {
            GuideCard("4. Point the launcher at it") {
                Text(
                    "Set Settings > Native mods > Build server to the URL wrangler deploy printed. " +
                        "This fork's launcher otherwise names its own Worker " +
                        "(horizon-mod-builds.steve-horizon-android-hnmod.workers.dev).",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Leave Compile on GitHub on: the Worker always uses GitHub's runners.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Check it without building anything: curl -X POST " +
                        "'https://<worker>.<account>.workers.dev/v1/build?version=bad' should answer " +
                        "400 with {\"error\":\"That is not a version of the game.\"}.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item(key = "step5") {
            GuideCard("5. Build a mod") {
                Text(
                    "On Mods, a native (.hnmod) card has Build: it downloads the mod, turns it on and " +
                        "asks for a game that carries it.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "For several at once, press Select native, tick them, then Build N. One build " +
                        "carries every native mod that is on, so it is one compile, not one each.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "The result appears in Settings > Native mods > Builds; press Use to play it. " +
                        "The launcher waits up to 90 minutes for the runners.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        item(key = "trouble") {
            GuideCard("If it does not work") {
                Text(
                    "GitHub refused the build: the token lacks Actions: write on the builds " +
                        "repository, or GITHUB_REPO names another repository.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "GitHub did not start the build: Actions are disabled on the builds repository, " +
                        "or GITHUB_REF is not the branch that holds build.yml.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "The job stops on the cache restore: run base once for that commit (step 1), " +
                        "then build again.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "400 That is not a version of the game: the app sent a version name the Worker " +
                        "does not know. Update the app, or check its version name.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "403 from Cloudflare: the URL is not the one wrangler deploy printed. Opening it " +
                        "in a browser should answer {\"error\":\"No such thing here.\"}.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        item(key = "direct") {
            GuideCard("Without GitHub") {
                Text(
                    "A Linux machine that is always on can build directly: run " +
                        "packaging/android/mod_build_server.py with the toolchain the documentation " +
                        "describes (a clone of this repository at the release commit, Rust with the " +
                        "aarch64-linux-android target, and the Android NDK), then set Build server to " +
                        "its address, for example http://192.168.1.20:8787.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    "Each build runs in a sandbox with no network. A gateway in front " +
                        "(mod_build_gateway.py) adds limits and a queue for a machine other people use.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item(key = "more") {
            Text(
                "The full guide, including the sandbox, the gateway protocol and troubleshooting, is " +
                    "docs/android-native-mods.md in the repository.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 4.dp),
            )
        }
    }
}

@Composable
private fun GuideCard(title: String, content: @Composable ColumnScope.() -> Unit) {
    Card(
        shape = RoundedCornerShape(24.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(
            Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            content()
        }
    }
}

@Composable
private fun GuideCode(code: String, copy: (String) -> Unit) {
    Surface(
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            Modifier.padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                code,
                style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = { copy(code) }) {
                Icon(Icons.Rounded.ContentCopy, "Copy", Modifier.size(18.dp))
            }
        }
    }
}
