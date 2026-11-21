# TASK: Port Horizon Android to Minecraft Java 26.4-snapshot-10

## 1. PROJECT OBJECTIVE

Migrate `ProSteve123GH/horizon-android-old` from Minecraft 26.3 to Minecraft Java Edition 26.4-snapshot-10.

The implementation MUST use the current architecture, infrastructure, version-porting tools, and Rust implementation of upstream `horizon-rust/horizon`.

This is a dedicated development branch. It is NOT a replacement for the existing Android branch.

### Target repositories

- Upstream: `https://github.com/horizon-rust/horizon`
- Android fork: `https://github.com/ProSteve123GH/horizon-android-old`
- Target branch: `dev/26.4`
- Previous version: `26.3`
- Target Minecraft version: `26.4-snapshot-10`

### Final objective

Produce a functional Android build of Horizon running Minecraft 26.4-snapshot-10, preserving all existing Android integrations and all compatible Horizon features and optimizations.

Do not stop after updating version constants, metadata, or protocol identifiers. The actual game implementation must be migrated and validated.

---

## 2. MANDATORY PROJECT CONSTRAINTS

Before doing ANY implementation work:

1. Read the complete `AGENTS.md`.
2. Read the complete `LOCAL_ENVIRONMENT.md`.
3. Inspect repository structure and existing Git state.
4. Identify all project-specific instructions and environment restrictions.
5. Record the available CPU, RAM, storage budget, installed toolchains, and build capabilities.
6. Inspect the existing Android port's architecture and modifications.
7. Inspect the latest upstream Horizon architecture and its version-porting infrastructure.

These files are authoritative for the local environment:

- `AGENTS.md`
- `LOCAL_ENVIRONMENT.md`

Do not ignore, override, or silently contradict their instructions.

If these files are missing, report that fact and inspect the repository's other documentation before proceeding.

### Git branch rules

- Work exclusively on `dev/26.4`.
- Inspect whether `dev/26.4` already exists before creating it.
- If it does not exist, create it from the existing Android development baseline.
- Do not implement this migration directly on `main`.
- Do not merge the completed work into the existing Android branch.
- Do not push changes unless explicitly authorized.
- Never force-push.
- Preserve unrelated existing modifications.

The existing Android branch must remain usable throughout the migration.

---

## 3. VERSION MIGRATION

### Exact target

`26.4-snapshot-10`

Do not substitute:
- 26.4 release
- 26.3
- 26.4-snapshot-9
- An arbitrary future snapshot

Replace all required references to 26.3 with the exact target version where they represent the active game version.

This includes, where applicable:

- Version metadata
- Game version constants
- Minecraft version identifiers
- Version lockfiles
- Protocol version
- Data version
- Resource pack format
- Data pack format
- Packet registries
- Generated registries
- Asset manifests
- Client resource preparation
- Server resource preparation
- Version-dependent tests
- Android launcher metadata
- JNI version integration
- Build scripts and packaging metadata

Do not blindly replace every occurrence of `26.3`.

Historical migration documentation, compatibility notes, previous-version fixtures, and baseline test data must retain their original identifiers.

### Upstream infrastructure

Reuse the current upstream version-porting infrastructure wherever possible.

Investigate:

- `tools/version_port.py`
- `tools/port_regen.py`
- `tools/port_report.py`
- `tools/vanilla_map.py`
- `tools/class_index.py`
- `tools/lab.py`
- `docs/version-ports.md`
- Existing parity harnesses
- Existing Java oracle infrastructure
- Existing version-lock implementation

Important: upstream snapshot handling may only perform preparation, diffing, and reporting by default.

Determine whether the exact snapshot identifier is supported by the regeneration and candidate-worktree infrastructure.

If it is not:

1. Extend the existing tooling minimally.
2. Preserve its integrity checks.
3. Support the exact snapshot identifier.
4. Avoid introducing a separate, duplicated migration system.
5. Document any unavoidable deviations from the upstream pipeline.

Do not assume that `version_port.py port 26.4-snapshot-10` works without inspecting its implementation.

### Vanilla analysis permission

You are explicitly authorized to download the official Minecraft 26.4-snapshot-10 client and server JARs for analysis.

Requirements:

- Obtain artifacts through Mojang's official version manifest and download URLs.
- Verify SHA-1 checksums against the official version metadata.
- Verify server bundler extraction and its relevant artifacts.
- Do not commit vanilla JARs.
- Do not commit decompiled Minecraft source.
- Do not publish copyrighted game files or generated game resources.
- Keep temporary analysis files outside the repository where practical.

---

## 4. STORAGE AND RESOURCE MANAGEMENT

CRITICAL ENVIRONMENT CONSTRAINT:

The execution environment is a Dockerized container running under proot.

Its actual assigned storage is approximately 10 GB.

`df -h` misleadingly reports approximately 1 TB because it exposes the underlying host filesystem.

**NEVER use `df -h` as a storage-capacity measurement.**

Do not trust the reported host filesystem capacity.

### Storage policy

Treat 10 GB as the absolute storage allocation ceiling.

Before downloading or extracting anything:

1. Inspect the actual container environment.
2. Inspect mount information and any available container quota configuration.
3. Calculate existing workspace consumption using `du`.
4. Identify existing caches, build artifacts, and temporary files.
5. Estimate the storage requirements of the proposed operation.
6. Maintain a storage budget throughout the migration.

Use actual directory consumption, such as:

- `du -x -s`
- `du -x -h --max-depth=1`
- `du -sh`

Do not assume that an apparently large amount of host filesystem space is available to the container.

### Storage budget

Use these conservative operating limits:

| Allocation | Budget |
|---|---:|
| Absolute environment ceiling | 10 GB |
| Reserved safety margin | 2 GB |
| Maximum planned consumption | 8 GB |

The 8 GB limit includes existing workspace usage, downloaded artifacts, generated resources, caches, and build outputs.

These are planning limits, not a claim that the container's exact remaining quota has been measured.

### Download and extraction rules

- Avoid downloading duplicate JARs.
- Reuse verified artifacts when possible.
- Do not download unnecessary Minecraft client libraries.
- Avoid full decompilation when class-specific decompilation is sufficient.
- Avoid retaining unnecessary historical snapshots.
- Avoid building multiple full Android configurations simultaneously.
- Clean only migration-generated temporary files.
- Never delete unrelated user files or pre-existing caches without permission.

If the storage budget is approaching its limit, STOP additional downloads and builds.

Report the storage situation and propose cleanup options before deleting anything.

Do not silently remove files to make the build succeed.

---

## 5. FEATURE PRESERVATION POLICY

The migration must distinguish between general mod-loading support and independent Horizon/Android functionality.

### A. REMOVE MOD SUPPORT

Remove the general mod-loading infrastructure from this branch.

This includes, where present:

- Mod discovery
- Mod installation and management
- Mod loading
- Mod dependency resolution
- Mod compatibility checks
- Mod-specific configuration interfaces
- Mod-specific launch arguments
- Mod-specific resource loading
- Mod loader integrations
- Fabric/Quilt/Forge compatibility layers
- Mod management UI
- Mod-specific JNI interfaces
- Mod-only build dependencies
- Tests and documentation exclusively related to mod support

Remove obsolete mod-related dependencies when they are no longer required.

Do not leave broken or misleading mod support behind.

However, do not indiscriminately delete every feature that happens to interact with a modified Minecraft server.

### B. PRESERVE HORIZON EXTRAS

Preserve all existing upstream Horizon features that are independent of general mod loading.

Examples include:

- Performance optimizations
- Rendering optimizations
- Memory optimizations
- World loading improvements
- Networking improvements
- User interface improvements
- Accessibility features
- Quality-of-life features
- Horizon-specific gameplay improvements
- Existing configuration systems
- Existing launcher functionality
- Other non-mod Horizon features

Do not simplify the project into a minimal vanilla Minecraft implementation.

The goal is a complete Horizon port, not a stripped-down Minecraft client.

### C. PRESERVE ANDROID FUNCTIONALITY

The following existing Android features are mandatory:

**Touch controls**
- Existing virtual controls
- Movement joystick
- Jump and interaction buttons
- Camera movement
- Touch input mapping
- Existing touch configuration
- Android-specific input handling

**Simple Voice Chat client**

Preserve the existing native Simple Voice Chat client integration.

This refers to the client-side implementation for the popular Minecraft Java Simple Voice Chat mod.

Important distinction:

Removing general mod-loader support does NOT authorize removing this standalone voice-chat integration.

Preserve:

- Voice chat protocol implementation
- Server compatibility
- UDP communication
- Audio capture integration
- Audio playback integration
- Microphone permission handling
- Android audio lifecycle
- Existing voice-chat configuration
- Existing connection and error handling

Adapt the protocol integration if the target snapshot requires changes.

Do not introduce a dependency on Fabric or another mod loader just to retain voice chat.

**Other Android features**

Preserve existing:

- JNI integration
- Kotlin/Java launcher
- Android lifecycle handling
- Filesystem access
- Resource management
- Native libraries
- Audio backend
- Vulkan integration
- Android-specific build configuration
- ARM64 compatibility
- Existing performance improvements

Do not replace functioning Android components with upstream desktop implementations without a demonstrated technical necessity.

### D. PRESERVATION RULE

For every removed or modified component, classify it as one of:

1. General mod support — eligible for removal.
2. Upstream Horizon feature — preserve.
3. Android-specific feature — preserve.
4. Required 26.4 incompatibility — adapt.
5. Obsolete implementation — justify removal.

Document significant removals and adaptations.

---

## 6. MULTI-AGENT TASK DISTRIBUTION

Use multiple subagents to distribute independent research and implementation tasks.

The coordinator is responsible for architecture, integration, shared-file ownership, and final validation.

Do not allow agents to modify the same core files concurrently.

### Agent 0 — Lead Architect / Coordinator

Responsibilities:

- Read mandatory environment documentation.
- Inspect both repositories.
- Establish the `dev/26.4` branch.
- Create a migration baseline.
- Identify upstream changes.
- Define task dependencies.
- Assign subagents.
- Resolve conflicting changes.
- Review all integrated work.
- Maintain the migration checklist.
- Run final validation.

The coordinator owns shared version configuration and final integration.

### Agent 1 — Upstream Version-Port Research

Responsibilities:

- Analyze the 26.3 → 26.4-snapshot-10 differences.
- Inspect the version-porting pipeline.
- Analyze generated data and registries.
- Analyze packet changes.
- Analyze Minecraft version metadata.
- Analyze class-level changes.
- Produce a prioritized implementation report.

Do not independently modify shared core implementation files.

Deliverable: `reports/26.4-version-diff.md`

### Agent 2 — Core Rust Engine Migration

Responsibilities:

- Adapt upstream Rust implementations.
- Update affected gameplay systems.
- Update registries and generated constants.
- Adapt changed vanilla APIs and behaviors.
- Preserve existing Horizon improvements.
- Implement missing 26.4 functionality.

Follow upstream architecture rather than introducing parallel implementations.

Deliverable: working migrated engine components and associated tests.

### Agent 3 — Networking and Protocol

Responsibilities:

- Inspect client/server protocol changes.
- Update packet definitions.
- Update serialization and deserialization.
- Update connection negotiation.
- Update relevant networking tests.
- Verify multiplayer compatibility.

Preserve existing networking optimizations.

Deliverable: protocol implementation and networking validation report.

### Agent 4 — Rendering and Performance

Responsibilities:

- Inspect 26.4 rendering changes.
- Investigate the new default Vulkan backend.
- Adapt renderer integration where required.
- Preserve Horizon rendering optimizations.
- Check Android Vulkan compatibility.
- Check shader and resource changes.
- Investigate performance regressions.

Do not assume that desktop Vulkan support automatically guarantees Android Vulkan compatibility.

Deliverable: rendering integration and benchmark report.

### Agent 5 — Android Platform Integration

Responsibilities:

- Inspect existing Android-specific modifications.
- Preserve JNI interfaces.
- Preserve Kotlin launcher functionality.
- Preserve touch controls.
- Preserve Android lifecycle integration.
- Preserve filesystem and resource handling.
- Preserve native audio integration.
- Verify ARM64 build compatibility.

Coordinate changes to shared JNI and build files with the coordinator.

Deliverable: Android compatibility implementation.

### Agent 6 — Simple Voice Chat

Responsibilities:

- Identify the existing standalone Simple Voice Chat client.
- Map its dependencies and integration points.
- Ensure general mod-loader removal does not remove voice chat.
- Investigate any protocol or compatibility changes.
- Preserve UDP and audio functionality.
- Verify Android microphone permission and audio lifecycle handling.

Deliverable: preserved and validated voice-chat integration.

### Agent 7 — Mod Support Removal and Cleanup

Responsibilities:

- Identify general mod-loading functionality.
- Map dependencies before removal.
- Remove obsolete mod infrastructure.
- Preserve standalone Simple Voice Chat.
- Preserve independent Horizon extras.
- Remove obsolete tests and documentation.
- Verify that no remaining application path depends on removed mod infrastructure.

Deliverable: removal report and passing cleanup tests.

### Agent 8 — QA and Validation

Responsibilities:

- Establish baseline tests.
- Identify existing test failures before implementation.
- Validate regenerated data.
- Run Rust tests.
- Run parity harnesses.
- Validate Android compilation.
- Inspect APK packaging.
- Verify touch controls.
- Verify voice-chat integration.
- Check startup and world loading.
- Identify regressions.

Do not modify implementation code without coordinating with the owning agent.

Deliverable: `reports/26.4-validation.md`

### Agent coordination rules

- Delegate independent research first.
- Define file ownership before parallel implementation.
- Avoid overlapping edits.
- Use separate worktrees when agents need independent Git modifications.
- Require agents to report changed files and test results.
- Have the coordinator review changes before integration.
- Never treat an agent's completion message as proof of a successful implementation.

If the available subagent system supports task dependencies, use them.

Suggested execution order:

1. Architecture and environment inspection.
2. Version-port research.
3. Android feature inventory.
4. Independent protocol/rendering/core research.
5. Core implementation.
6. Android and voice-chat adaptation.
7. Mod support cleanup.
8. Integration and validation.

---

## 7. IMPLEMENTATION PHASES

### Phase 0 — Environment and repository audit

- Read `AGENTS.md`.
- Read `LOCAL_ENVIRONMENT.md`.
- Inspect Git remotes and branches.
- Establish `dev/26.4`.
- Inspect the current Android fork.
- Inspect upstream Horizon.
- Inventory existing features.
- Record existing build/test failures.
- Establish storage consumption and budget.

Do not begin destructive cleanup.

### Phase 1 — Version preparation

- Identify the exact official snapshot metadata.
- Download the official client and server JARs.
- Verify checksums.
- Prepare the vanilla data.
- Inspect server bundler contents.
- Run the upstream version-port analysis.
- Generate the 26.3 → 26.4-snapshot-10 diff.
- Identify affected Horizon modules.
- Generate the prioritized migration report.

### Phase 2 — Version-dependent regeneration

Regenerate every applicable version-dependent artifact.

This includes:

- Registry tables
- Packet definitions
- Protocol constants
- Block state information
- Item components
- Command structures
- Data-driven resources
- Asset indexes
- Version constants
- Other generated tables

Record every successful and failed generator.

Do not manually edit generated files when a working generator exists.

When a generator fails, investigate the underlying vanilla API change instead of bypassing the failure.

### Phase 3 — Core engine migration

Implement the actual 26.4 changes.

Investigate the snapshot's:

- Data-driven dispensers
- Improved Fog setting
- Vulkan-related rendering changes
- Repeat Recipe/Trade keybind
- Recipe book changes
- Accessibility changes
- Gameplay and protocol changes
- Other changes identified by the diff

Do not assume the changes listed above are exhaustive.

Preserve upstream Horizon extras and performance work.

### Phase 4 — Android adaptation

Integrate the migrated engine into the Android platform.

Verify:

- Native compilation
- JNI compatibility
- Android packaging
- Vulkan initialization
- Touch controls
- Audio
- Simple Voice Chat
- Resource loading
- Application lifecycle

Resolve compatibility issues without unnecessarily replacing existing working Android implementations.

### Phase 5 — Mod support removal

Remove general mod-loading infrastructure after mapping dependencies.

Verify that:

- No general mod loader is required.
- No obsolete mod manager remains.
- No stale mod-specific launch configuration remains.
- Simple Voice Chat still exists.
- Other independent Horizon features still work.

### Phase 6 — Testing and optimization

Run relevant upstream self-checks and parity harnesses.

Run the Android build and targeted runtime tests.

Compare performance against the existing 26.3 Android baseline where possible.

Do not claim a performance improvement without measurements.

### Phase 7 — Final audit

Review:

- Git diff
- Removed files
- Generated files
- Build configuration
- Version references
- Android functionality
- Voice chat
- Feature preservation
- Storage consumption
- Test results

Produce a final migration report.

---

## 8. BUILD AND VALIDATION REQUIREMENTS

Use the installed toolchains documented in `LOCAL_ENVIRONMENT.md`.

Do not assume a desktop Linux environment.

Do not install unnecessary SDKs or toolchains.

Do not modify system-wide configurations unless explicitly authorized.

### Rust

Run applicable:

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- Relevant crate-specific tests
- Upstream parity harnesses

Use workspace-aware and targeted testing to avoid unnecessary rebuilds.

### Android

Verify:

- Correct Android target architecture.
- Native Rust compilation.
- JNI linking.
- APK assembly.
- Correct application version metadata.
- Native library packaging.
- Vulkan initialization.
- Touch input.
- Audio integration.

### Gameplay

Where the available environment permits, verify:

- Client startup.
- Main menu.
- World creation.
- World loading.
- Block interaction.
- Inventory.
- Rendering.
- Chunk loading.
- Networking.
- Multiplayer.
- Server connection.
- Relevant 26.4 snapshot mechanics.

### Simple Voice Chat

Verify:

- Existing client initialization.
- Audio permissions.
- Audio capture.
- Audio playback.
- UDP connection handling.
- Compatibility with a corresponding server-side Simple Voice Chat installation.

If a real compatible server is unavailable, report the untested portion honestly.

### Testing honesty

Never mark a feature as working merely because it compiles.

Classify validation as:

- PASS — successfully tested.
- PARTIAL — some functionality tested.
- BLOCKED — unable to test because of environment or dependencies.
- FAIL — test executed and failed.
- NOT TESTED — no actual validation performed.

---

## 9. VERSION SWITCHER — EXPLICITLY OUT OF SCOPE

Do NOT implement a version switcher.

Do NOT introduce multi-version runtime support.

Do NOT implement a general version manager.

Do NOT add automatic 26.3/26.4 switching.

This branch must simply replace the required active game version with `26.4-snapshot-10`.

Version switching can be considered in a future project after this branch becomes stable.

Keep the implementation clean enough that future version-switching work remains possible, but do not build it now.

---

## 10. ACCEPTANCE CRITERIA

The migration is complete only when the following conditions are satisfied.

### Repository and branch

- [ ] Work is on `dev/26.4`.
- [ ] Existing Android branch remains untouched by the migration.
- [ ] Mandatory environment documentation was followed.
- [ ] Upstream architecture is preserved.

### Minecraft version

- [ ] Active game version is exactly `26.4-snapshot-10`.
- [ ] Required version-dependent constants are updated.
- [ ] Registries are regenerated.
- [ ] Protocol implementation is updated.
- [ ] Data and assets are updated.
- [ ] Snapshot-specific gameplay changes are implemented.

### Feature preservation

- [ ] General mod support removed.
- [ ] Horizon extras preserved.
- [ ] Horizon performance optimizations preserved.
- [ ] Android touch controls preserved.
- [ ] Simple Voice Chat client preserved.
- [ ] JNI integration preserved.
- [ ] Android audio integration preserved.
- [ ] Vulkan integration preserved.
- [ ] Android filesystem integration preserved.

### Android build

- [ ] ARM64 compilation succeeds.
- [ ] JNI linking succeeds.
- [ ] APK packaging succeeds.
- [ ] Application starts successfully.
- [ ] Vulkan initialization validated on Android.
- [ ] Touch controls validated.
- [ ] Audio integration validated.

### Quality assurance

- [ ] Rust tests pass.
- [ ] Relevant parity tests pass.
- [ ] No unexplained regressions.
- [ ] No accidental feature removals.
- [ ] No unintended changes to the existing Android branch.
- [ ] Storage remains within the 8 GB planned consumption limit.
- [ ] Final migration report exists.

---

## 11. REQUIRED DELIVERABLES

Produce:

1. Working implementation on `dev/26.4`.
2. `reports/26.4-version-diff.md`
3. `reports/26.4-mod-removal.md`
4. `reports/26.4-android-compatibility.md`
5. `reports/26.4-validation.md`
6. Updated build and version configuration.
7. Updated tests and relevant parity fixtures.
8. Final list of preserved, adapted, and removed features.

The final report must include:

- Summary of changes.
- Important architectural changes.
- Files and modules modified.
- Files and modules removed.
- Version-port tooling changes.
- Android-specific adaptations.
- Simple Voice Chat compatibility status.
- Build commands executed.
- Tests executed and their results.
- APK output location, if successfully generated.
- Storage consumption.
- Remaining known issues.
- Features that remain untested.

---

## 12. FINAL EXECUTION INSTRUCTIONS

Work autonomously within the permissions granted by this task.

You are authorized to download and analyze the official 26.4-snapshot-10 client and server JARs.

You are authorized to modify the `dev/26.4` branch to accomplish this migration.

However:

- Do not delete unrelated user data.
- Do not overwrite unrelated local modifications.
- Do not force-push.
- Do not merge into the existing Android branch.
- Do not exceed the storage budget.
- Do not silently remove Android functionality.
- Do not remove Simple Voice Chat under the justification of removing mod support.
- Do not implement the version switcher.
- Do not claim successful testing without executing the tests.

If a critical operation would destroy or overwrite existing user data, stop and request approval.

Prioritize correctness, feature preservation, upstream architectural consistency, and actual Android functionality over superficial completion.

**FINAL GOAL: A fully functional Horizon Android port targeting Minecraft Java Edition 26.4-snapshot-10, built on the latest upstream Horizon architecture, with general mod support removed but all independent Horizon extras, performance optimizations, Android touch controls, and Simple Voice Chat functionality preserved.**
