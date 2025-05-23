# cargo apk

[![Actions Status](https://github.com/5GameMaker/cargo-apk/actions/workflows/rust.yml/badge.svg)](https://github.com/5GameMaker/cargo-apk/actions)
[![Latest version](https://img.shields.io/crates/v/cargo-apk.svg?logo=rust)](https://crates.io/crates/cargo-apk)
[![MSRV](https://img.shields.io/badge/rustc-1.86.0+-ab6000.svg)](https://blog.rust-lang.org/2023/06/01/Rust-1.86.0.html)
[![Documentation](https://docs.rs/cargo-apk/badge.svg)](https://docs.rs/cargo-apk)
[![Lines of code](https://tokei.rs/b1/github/5GameMaker/cargo-apk)](https://github.com/5GameMaker/cargo-apk)
![MIT](https://img.shields.io/badge/License-MIT-green.svg)
![Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-green.svg)

Tool for creating Android packages.

## Installation

From git:

```console
$ cargo install --git https://github.com/5GameMaker/cargo-apk
```

From source:

```console
$ cargo install --path cargo-apk/
```

## Commands

- `build`: Compiles the current package
- `run`: Run a binary or example of the local package
- `gdb`: Start a gdb session attached to an adb device with symbols loaded

## Manifest

`cargo` supports the `metadata` table for configurations for external tools like `cargo apk`.
Following configuration options are supported by `cargo apk` under `[package.metadata.android]`:

```toml
[package.metadata.android]
# Specifies the package property of the manifest.
package = "com.foo.bar"

# Specifies the array of targets to build for.
build_targets = [ "armv7-linux-androideabi", "aarch64-linux-android", "i686-linux-android", "x86_64-linux-android" ]

# Path to your application's resources folder.
# If not specified, resources will not be included in the APK.
resources = "path/to/resources_folder"

# Path to the folder containing your application's assets.
# If not specified, assets will not be included in the APK.
assets = "path/to/assets_folder"

# Name for final APK file.
# Defaults to package name.
apk_name = "myapp"

# `default` (or unspecified) - Debug symbols, if they exist, are not treated
#                              specially.
#
# `strip`                    - Debug symbols are stripped from the shared
#                              libraries before being copied into the APK.
#
# `split`                    - Functions the same as `strip`, except the debug
#                              symbols are written to the apk output directory
#                              alongside the stripped shared libraries, with
#                              a `.dwarf` extension.
#
# Note that the `strip` and `split` options will only have an effect if
# debug symbols are present in the `.so` file(s) produced by your build, enabling
# https://doc.rust-lang.org/cargo/reference/profiles.html#strip or
# https://doc.rust-lang.org/cargo/reference/profiles.html#split-debuginfo
# in your cargo manifest can cause debug symbols to no longer be present
# in the `.so`.
strip = "default"

# Folder containing extra shared libraries intended to be dynamically loaded at runtime.
# Files matching `libs_folder/${android_abi}/*.so` are added to the apk
# according to the specified build_targets.
runtime_libs = "path/to/libs_folder"

# The name of a Linux user ID that is shared with other apps. By
# default, Android assigns each app its own unique user ID. However, if
# this attribute is set to the same value for two or more apps, they all
# share the same ID, provided that their certificate sets are identical.
# Apps with the same user ID can access each other's data and, if
# desired, run in the same process.
shared_user_id = "my.shared.user.id"

# Defaults to `$HOME/.android/debug.keystore` for the `dev` profile. Will ONLY
# generate a new debug.keystore if this file does NOT exist. A keystore is never
# auto-generated for other profiles.
#
# The keystore path can be absolute, or relative to the Cargo.toml file.
#
# The environment variables `CARGO_APK_<PROFILE>_KEYSTORE` and
# `CARGO_APK_<PROFILE>_KEYSTORE_PASSWORD` can be set to a keystore path
# and keystore password respectively. The profile portion follows the same rules
# as `<cfg>`, it is the uppercased profile name with `-` replaced with `_`.
#
# If present they take precedence over the signing information in the manifest.
[package.metadata.android.signing.<profile>]
path = "relative/or/absolute/path/to/my.keystore"
keystore_password = "android"

# See https://developer.android.com/guide/topics/manifest/uses-sdk-element
#
# Defaults to a `min_sdk_version` of 23 and `target_sdk_version` of 30 (or lower if the detected NDK doesn't support this).
[package.metadata.android.sdk]
min_sdk_version = 23
target_sdk_version = 30
max_sdk_version = 29

# See https://developer.android.com/guide/topics/manifest/uses-feature-element
#
# Note: there can be multiple .uses_feature entries.
[[package.metadata.android.uses_feature]]
name = "android.hardware.vulkan.level"
required = true
version = 1

# See https://developer.android.com/guide/topics/manifest/uses-permission-element
#
# Note: there can be multiple .uses_permission entries.
[[package.metadata.android.uses_permission]]
name = "android.permission.WRITE_EXTERNAL_STORAGE"
max_sdk_version = 18

# See https://developer.android.com/guide/topics/manifest/queries-element#provider
[[package.metadata.android.queries.provider]]
authorities = "org.khronos.openxr.runtime_broker;org.khronos.openxr.system_runtime_broker"
# Note: The `name` attribute is normally not required for a queries provider, but is non-optional
# as a workaround for aapt throwing errors about missing `android:name` attribute.
# This will be made optional if/when cargo-apk migrates to aapt2.
name = "org.khronos.openxr"

# See https://developer.android.com/guide/topics/manifest/queries-element#intent
[[package.metadata.android.queries.intent]]
actions = ["android.intent.action.SEND"]

# See https://developer.android.com/guide/topics/manifest/queries-element#intent
# Note: there can be several .data entries.
[[package.metadata.android.queries.intent.data]]
mime_type = "image/jpeg"

# See https://developer.android.com/guide/topics/manifest/queries-element#package
[[package.metadata.android.queries.package]]
name = "org.freedesktop.monado.openxr_runtime.in_process"

# See https://developer.android.com/guide/topics/manifest/application-element
[package.metadata.android.application]

# See https://developer.android.com/guide/topics/manifest/application-element#debug
#
# Defaults to false.
debuggable = false

# See https://developer.android.com/guide/topics/manifest/application-element#theme
#
# Example shows setting the theme of an application to fullscreen.
theme = "@android:style/Theme.DeviceDefault.NoActionBar.Fullscreen"

# Virtual path your application's icon for any mipmap level.
# If not specified, an icon will not be included in the APK.
icon = "@mipmap/ic_launcher"

# See https://developer.android.com/guide/topics/manifest/application-element#label
#
# Defaults to the compiled artifact's name.
label = "Application Name"

# See https://developer.android.com/guide/topics/manifest/application-element#extractNativeLibs
extract_native_libs = true

# See https://developer.android.com/guide/topics/manifest/application-element#usesCleartextTraffic
uses_cleartext_traffic = true

# See https://developer.android.com/guide/topics/manifest/meta-data-element
#
# Note: there can be several .meta_data entries.
# Note: the `resource` attribute is currently not supported.
[[package.metadata.android.application.meta_data]]
name = "com.samsung.android.vr.application.mode"
value = "vr_only"

# See https://developer.android.com/guide/topics/manifest/activity-element
[package.metadata.android.application.activity]

# See https://developer.android.com/guide/topics/manifest/activity-element#config
#
# Defaults to "orientation|keyboardHidden|screenSize".
config_changes = "orientation"

# See https://developer.android.com/guide/topics/manifest/activity-element#label
#
# Defaults to the application's label.
label = "Activity Name"

# See https://developer.android.com/guide/topics/manifest/activity-element#lmode
#
# Defaults to "standard".
launch_mode = "singleTop"

# See https://developer.android.com/guide/topics/manifest/activity-element#screen
#
# Defaults to "unspecified".
orientation = "landscape"

# See https://developer.android.com/guide/topics/manifest/activity-element#exported
#
# Unset by default, or true when targeting Android >= 31 (S and up).
exported = true

# See https://developer.android.com/guide/topics/manifest/activity-element#resizeableActivity
#
# Defaults to true on Android >= 24, no effect on earlier API levels
resizeable_activity = false

# See https://developer.android.com/guide/topics/manifest/activity-element#always
always_retain_task_state = true

# See https://developer.android.com/guide/topics/manifest/meta-data-element
#
# Note: there can be several .meta_data entries.
# Note: the `resource` attribute is currently not supported.
[[package.metadata.android.application.activity.meta_data]]
name = "com.oculus.vr.focusaware"
value = "true"

# See https://developer.android.com/guide/topics/manifest/intent-filter-element
#
# Note: there can be several .intent_filter entries.
[[package.metadata.android.application.activity.intent_filter]]
# See https://developer.android.com/guide/topics/manifest/action-element
actions = ["android.intent.action.VIEW", "android.intent.action.WEB_SEARCH"]
# See https://developer.android.com/guide/topics/manifest/category-element
categories = ["android.intent.category.DEFAULT", "android.intent.category.BROWSABLE"]

# See https://developer.android.com/guide/topics/manifest/data-element
#
# Note: there can be several .data entries.
# Note: not specifying an attribute excludes it from the final data specification.
[[package.metadata.android.application.activity.intent_filter.data]]
scheme = "https"
host = "github.com"
port = "8080"
path = "/rust-windowing/android-ndk-rs/tree/master/cargo-apk"
path_prefix = "/rust-windowing/"
mime_type = "image/jpeg"

# Set up reverse port forwarding through `adb reverse`, meaning that if the
# Android device connects to `localhost` on port `1338` it will be routed to
# the host on port `1338` instead. Source and destination ports can differ,
# see the `adb` help page for possible configurations.
[package.metadata.android.reverse_port_forward]
"tcp:1338" = "tcp:1338"
```

If a manifest attribute is not supported by `cargo apk` feel free to create a PR that adds the missing attribute.
