Task: Deep Mainstream Horizon Synchronization, Android Port Migration, and Full Error Resolution

You are tasked with bringing my Android fork of Horizon up to date with the latest mainstream Horizon engine, resolving compatibility problems, preserving all Android-specific features, and integrating relevant upstream improvements.

This is a large migration. Work methodically in multiple phases, but you must autonomously proceed from one phase to the next until the entire task is completed or every remaining blocker has been exhausted. Do not stop after producing a report, do not ask me for confirmation between phases, and do not wait for me to supervise you.

1. Repositories and synchronization target

My repository (target):
"https://github.com/ProSteve123GH/horizon-android-old"

My repository's target branch:
"android"

Mainstream upstream source:
"https://github.com/horizon-rust/horizon"

Mainstream branch:
"main"

Important distinctions:

- My repository is an older Android-oriented fork of "starling-client/starling".
- The source of truth for mainstream engine development is "horizon-rust/horizon", NOT the Android port's branch.
- Synchronize engine development from "horizon-rust/horizon:main".
- Do not replace my repository with the latest Starling implementation.
- Do not merge "starling-client/starling" just because it is the original parent repository.
- Existing Android-specific changes are intentional and must be preserved.

The objective is to bring the Android fork closer to the current mainstream engine while retaining its own identity and Android functionality.

2. Critical rules

Preserve the existing Android implementation

Do not destroy, remove, or unnecessarily rewrite:

- Android-specific Rust code and platform integrations.
- Jetpack Compose launcher and its existing UI.
- Existing touch controls and their behavior.
- Touch HUD customization, button layouts, joystick/D-pad handling, and related settings.
- Android input handling, lifecycle handling, JNI bindings, and native activity integration.
- Android Gradle configuration, manifests, permissions, and packaging.
- Existing resource importing, account handling, and Android storage integration.
- Existing ".hmod" support.
- Existing ".hnmod" support or any ongoing implementation of it.
- Previously recovered functionality and changes specific to this fork.

The old touch controls are especially important. Do not replace them with a different control system just because mainstream has changed its input architecture. Adapt the new engine to the existing Android controls instead.

Preserve Git history

Before making changes:

1. Inspect all Git remotes and branches.
2. Verify the current checked-out branch and working tree.
3. Record the current commit SHA.
4. Create a recoverable backup branch or tag for the original "android" branch.
5. Fetch the latest mainstream history.
6. Calculate the actual merge base and divergence.

Keep "origin" pointed at my repository.

Configure a remote for "https://github.com/horizon-rust/horizon.git" named "upstream". If that remote name is already used by another repository, preserve the existing remote under a clearly named reference before configuring the correct mainstream remote.

Never blindly run a destructive reset, rebase, force push, or history rewrite. Never delete existing branches or commits to make synchronization easier.

Use Git's actual ancestry and change history to determine the safest integration strategy. Prefer preserving the existing history through proper merges and focused conflict resolution.

3. Phase 1: Repository investigation and baseline

Before editing anything, thoroughly inspect:

- "AGENTS.md"
- "CONTRIBUTING.md"
- "CONTRIBUTING_ANDROID.md"
- Android build documentation.
- Existing Android-specific workflows.
- Existing native hook definitions.
- Existing ".hmod" and ".hnmod" implementations.
- Android launcher, Compose, JNI, and touch-control implementations.
- Current engine architecture and crate structure.
- Existing tests and CI configuration.

Also inspect the corresponding mainstream files and documentation.

Establish a baseline report containing:

- Current commit and branch.
- Mainstream commit and branch.
- Actual merge base.
- Number of unique commits on both sides.
- Major architecture changes.
- Existing Android-specific changes.
- Current known build errors.
- Current known test failures.
- Important compatibility differences.

Do not assume that every upstream change is relevant to Android. Categorize the changes before integration.

Save this information in a durable progress report.

4. Phase 2: Deep upstream change analysis

Analyze the changes between the current Android fork and mainstream Horizon.

Pay particular attention to:

Engine architecture: Crate reorganizations, moved modules, renamed functions, changed traits, changed public APIs, and removed or replaced implementations.

Performance: Improvements to rendering, chunk loading, memory use, threading, game ticks, networking, resource processing, and other relevant systems.

Vanilla parity: Fixes that make Horizon behave more accurately like Minecraft Java Edition.

Native mod compatibility: Changes to native hooks, hook metadata, game function signatures, mod APIs, mixin targets, and native mod compilation.

New features: Features added to mainstream that could reasonably work on Android.

Bug fixes: Existing bugs in the Android fork that mainstream has already fixed.

Do not blindly copy files from mainstream. Understand the changes and their dependencies.

Create a migration checklist and keep it updated as changes are integrated.

5. Phase 3: Mainstream engine synchronization

Integrate the latest stable mainstream development from "horizon-rust/horizon:main" into my "android" branch.

This is a major synchronization, not a small update.

Handle large architectural changes carefully. If upstream has moved a module or replaced an API, migrate the Android-specific code to the new architecture instead of retaining outdated code solely to avoid conflicts.

When resolving conflicts:

- Mainstream should generally be authoritative for engine behavior.
- Android-specific implementations should remain authoritative for Android platform behavior.
- Combine both where necessary.
- Avoid silently discarding either side's functionality.
- Add compatibility adapters where they are cleaner than duplicating code.
- Do not introduce temporary hacks that will make future synchronization harder.

Preserve attribution and upstream history.

After the synchronization, update relevant fork tracking files, including "docs/android-upstream.json" and any fork-maintenance notes required by the repository.

Do not merge experimental upstream branches automatically. Use the intended mainstream "main" branch.

6. Phase 4: Native hooks and mod API modernization

This is one of the highest-priority parts of the task.

Investigate the existing native hook implementation and compare it with the current mainstream implementation.

I suspect that some native mods are failing because the fork is using an outdated native hook API or outdated game function signatures. There may be a mismatch involving the old API 80 and newer APIs, but do not assume these version numbers are correct or simply change the version number to make errors disappear.

Determine the actual cause of the compatibility failures.

Inspect:

- "native-hooks.json"
- Native hook definitions and generated metadata.
- Native mod API definitions.
- Game function signatures and module paths.
- Current hook target validation.
- "horizon-mixins" integration, if applicable.
- Native mod compilation and compatibility checks.
- Any pinned versions or revisions used by the build system.
- Changes to relevant upstream game functions.

Bring the native hook implementation in line with the latest compatible mainstream implementation.

Ensure that native mods target the correct functions and that obsolete hook definitions are migrated or removed only when genuinely obsolete.

Preserve ".hnmod" functionality and its existing import/build workflow. Do not confuse ".hnmod" native mods with ".hmod" content mods. Both systems must continue to work.

Do not claim full compatibility simply because the project compiles. Validate the actual mod API and hook targets.

7. Phase 5: Performance, parity, and feature improvements

Integrate relevant improvements from mainstream.

Prioritize:

1. Performance improvements that are compatible with Android.
2. Vanilla parity fixes.
3. Memory and resource efficiency.
4. Rendering and Vulkan-related improvements.
5. Game logic and gameplay fixes.
6. Networking and multiplayer fixes.
7. New features that make sense on Android.
8. Developer and maintenance improvements.

Do not automatically bring in desktop-only functionality that cannot work on Android.

When an upstream feature requires a different platform implementation, adapt it properly or document why it cannot be supported.

Keep Android performance in mind, particularly on devices with limited RAM and mobile GPUs.

Avoid introducing performance regressions just to achieve source-code similarity with mainstream.

8. Phase 6: Fix all migration and compilation errors

Fix all errors discovered during the migration.

This includes:

- Rust compilation errors.
- Kotlin compilation errors.
- Gradle configuration errors.
- JNI signature mismatches.
- Android manifest and permission errors.
- Missing or moved dependencies.
- Outdated imports and module references.
- Broken native hooks.
- Incorrect trait implementations.
- Outdated APIs.
- Test failures.
- CI workflow failures.
- Android packaging problems.
- Runtime integration problems that can be validated through available automated tests.

Do not simply suppress errors, disable failing tests, remove functionality, or downgrade dependencies to make CI appear green.

For each failure, identify its root cause and fix it properly.

Distinguish existing failures from regressions introduced by this synchronization. Record any genuinely pre-existing failures that cannot reasonably be addressed during this task.

9. Phase 7: CI-only testing and validation

My tablet has limited RAM. Do not run heavy builds, full Rust compilation, Android compilation, Gradle builds, or large test suites locally.

Use GitHub Actions or another available remote CI environment for heavy workloads.

The local environment should only be used for lightweight repository inspection, editing, Git operations, and other low-resource tasks.

Inspect the existing workflows first. Reuse and improve them instead of creating unnecessary duplicate workflows.

Run the repository's appropriate checks, including:

- Rust formatting and compilation checks.
- Clippy where supported.
- Relevant Rust unit and integration tests.
- Android build validation.
- Kotlin and Gradle compilation.
- Android release audit tests.
- Public-tree and repository integrity checks.
- Native hook and mod compatibility tests.
- Existing vanilla parity and regression tests where practical.

Use the repository's documented commands and test infrastructure. Do not invent replacement tests just to avoid running the real ones.

Build the Android arm64 target at minimum.

Where possible, add CI coverage for additional supported configurations without wasting resources on irrelevant targets.

Do not upload personal Minecraft installations, proprietary game files, private account data, signing keys, or other sensitive files to CI. Respect the repository's existing resource preparation and distribution rules.

Keep upstream deployment workflows inactive. Only the fork's intended Android development workflow should produce APK artifacts.

A successful Rust build alone is not sufficient. Validate the Android-specific integration too.

10. Phase 8: Automated error recovery and continuation

This task must continue without my supervision.

Do not stop simply because one command, workflow, or test fails.

For every operation:

1. Capture its exit status and useful logs.
2. Identify the failure category.
3. Determine whether it can be retried safely.
4. Fix the underlying problem when possible.
5. Retry using a bounded retry strategy.
6. Record the result.
7. Continue with independent work when an unrelated task is blocked.

Use reasonable retry limits and backoff for temporary GitHub or network failures. Never create an infinite retry loop.

If a CI workflow fails, inspect its logs and fix the issue before rerunning it.

If a workflow is unavailable, identify another supported way to validate that specific component without performing heavy work on my tablet.

Durable progress and recovery

Create and maintain a progress file such as "SYNC_PROGRESS.md".

Update it after every major phase with:

- Completed work.
- Current integration branch and commit.
- Changes already pushed.
- Remaining tasks.
- CI workflow links and results.
- Current blockers.
- The exact next step.

Commit and push meaningful checkpoints to my repository so completed work survives an interrupted agent session.

If your execution environment supports resumable tasks or a documented background execution system, use it where appropriate.

Do not create an infinite shell loop, zombie process, or fake keep-alive script. Such a script cannot guarantee that the AI agent itself remains active. Use durable commits, checkpoints, and remote CI workflows so the work can resume safely if the agent session ends.

Never abandon the task just because an individual tool call fails. Recover, retry, or move to another independent phase.

11. Phase 9: Final integration and repository update

After the migration and validation are complete:

- Review the complete diff.
- Check for accidentally deleted Android functionality.
- Verify that Compose launcher code remains intact.
- Verify that the existing touch controls remain intact.
- Verify ".hmod" and ".hnmod" support has not regressed.
- Verify that no generated game resources, private data, or build artifacts have been committed.
- Check that upstream tracking documentation is accurate.
- Ensure the migration branch contains all completed changes.
- Ensure CI results are recorded.

Push the completed work to my repository.

Integrate it into the "android" branch only after the relevant checks pass and the repository's branch protection allows it. If direct integration is blocked, create a complete pull request targeting "android", with the migration report and CI results. Never bypass branch protection or force-push to make the task appear finished.

Do not leave half-integrated changes on the default branch.

12. Completion criteria

This task is complete only when:

- The latest intended mainstream "main" changes have been integrated.
- Android-specific code and functionality are preserved.
- The architecture and APIs have been migrated where necessary.
- Native hook compatibility has been investigated and modernized.
- Relevant upstream performance and vanilla parity improvements have been incorporated.
- Relevant new features have been adapted for Android.
- All migration errors that can reasonably be fixed have been addressed.
- Required CI checks have passed, or remaining external blockers are clearly documented.
- The completed changes have been pushed to my repository.
- The progress report and synchronization notes are updated.

At the end, provide a detailed but readable report containing the upstream commit used, the major changes integrated, Android-specific changes preserved, native mod API updates, errors fixed, CI results, remaining known issues, and links to the commits or pull request.

Final instruction

Work autonomously from Phase 1 through Phase 9. Do not ask me to approve individual stages. Do not stop after analysis or after the first successful merge. Do not waste time repeating completed work. Keep checkpoints, recover from tool errors, and continue until the full migration has been completed to the best possible extent.

The priority is a properly synchronized and working Android fork, not simply making the GitHub commit count look current.

Edit:

Additional Phase 8 Safeguard: Internet Disconnection and Network Recovery

Assume that the internet connection may become unstable or completely unavailable at any point during this unattended task.

You must not treat a temporary network failure as a reason to abandon the migration.

Network recovery rules:

1. Detect network-related failures separately from actual code or build errors.
2. If GitHub, Git, GitHub Actions, package registries, or other remote services become unreachable, preserve all current local changes immediately.
3. Never discard uncommitted work because a network operation failed.
4. Retry temporary failures using exponential backoff, for example: 5 seconds, 15 seconds, 30 seconds, 60 seconds, then 2 minutes.
5. After several unsuccessful attempts, pause network-dependent operations and continue with independent local tasks whenever possible.
6. Periodically retry connectivity instead of continuously hammering remote services.
7. Before pushing, fetch the latest remote state and check for changes made elsewhere. Never overwrite remote work.
8. If a push fails halfway or its result is uncertain, inspect the remote branch before retrying. Avoid duplicate commits and unnecessary force pushes.
9. If a CI workflow starts successfully but its status cannot be retrieved, preserve its workflow run ID and URL. Check the same run again when connectivity returns instead of starting duplicate builds.
10. If a clone, fetch, download, or dependency installation is interrupted, resume it when supported. Otherwise, retry safely without corrupting existing files.
11. Never assume that a failed network request means the remote operation did not succeed. Verify its actual state before repeating it.

When the internet is unavailable:

- Continue source analysis, code review, local editing, conflict preparation, and other independent tasks.
- Do not start operations that require unavailable remote resources.
- Save a checkpoint in "SYNC_PROGRESS.md", including the exact pending operation and recovery instructions.
- Resume network-dependent tasks automatically once connectivity returns.

Session interruption recovery:

If the agent process itself is terminated, a shell script cannot guarantee that it will restart. Therefore, use durable Git commits, pushed checkpoints, and resumable CI runs as the primary recovery mechanism. If a supported persistent agent execution or task-resumption feature exists, use it.

Never report the task as complete while network-dependent stages remain unfinished.
