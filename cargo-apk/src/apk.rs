use crate::error::Error;
use crate::manifest::{Inheritable, Manifest, Root};
use cargo_subcommand::{Artifact, ArtifactType, CrateType, Profile, Subcommand};
use ndk_build::apk::{Apk, ApkConfig};
use ndk_build::cargo::{VersionCode, cargo_ndk};
use ndk_build::dylibs::get_libs_search_paths;
use ndk_build::manifest::{IntentFilter, MetaData};
use ndk_build::ndk::{Key, Ndk};
use ndk_build::target::Target;
use ndk_build::util::output_error;
use std::path::PathBuf;
use std::process::{Stdio, exit};
use std::thread::sleep;
use std::time::Duration;

pub struct ApkBuilder<'a> {
    cmd: &'a Subcommand,
    ndk: Ndk,
    manifest: Manifest,
    build_dir: PathBuf,
    build_targets: Vec<Target>,
    device_serial: Option<String>,
}

impl<'a> ApkBuilder<'a> {
    pub fn from_subcommand(
        cmd: &'a Subcommand,
        device_serial: Option<String>,
    ) -> Result<Self, Error> {
        println!(
            "Using package `{}` in `{}`",
            cmd.package(),
            cmd.manifest().display()
        );
        let ndk = Ndk::from_env()?;
        let mut manifest = Manifest::parse_from_toml(cmd.manifest())?;
        let workspace_manifest: Option<Root> = cmd
            .workspace_manifest()
            .map(Root::parse_from_toml)
            .transpose()?;
        let build_targets = if let Some(target) = cmd.target() {
            vec![Target::from_rust_triple(target)?]
        } else if !manifest.build_targets.is_empty() {
            manifest.build_targets.clone()
        } else {
            vec![
                ndk.detect_abi(device_serial.as_deref())
                    .unwrap_or(Target::Arm64V8a),
            ]
        };
        let build_dir = dunce::simplified(cmd.target_dir())
            .join(cmd.profile())
            .join("apk");

        let package_version = match &manifest.version {
            Inheritable::Value(v) => v.clone(),
            Inheritable::Inherited { workspace: true } => {
                let workspace = workspace_manifest
                    .ok_or(Error::InheritanceMissingWorkspace)?
                    .workspace
                    .unwrap_or_else(|| {
                        // Unlikely to fail as cargo-subcommand should give us
                        // a `Cargo.toml` containing a `[workspace]` table
                        panic!(
                            "Manifest `{:?}` must contain a `[workspace]` table",
                            cmd.workspace_manifest().unwrap()
                        )
                    });

                workspace
                    .package
                    .ok_or(Error::WorkspaceMissingInheritedField("package"))?
                    .version
                    .ok_or(Error::WorkspaceMissingInheritedField("package.version"))?
            }
            Inheritable::Inherited { workspace: false } => return Err(Error::InheritedFalse),
        };
        let version_code = VersionCode::from_semver(&package_version)?.to_code(1);

        // Set default Android manifest values
        if manifest
            .android_manifest
            .version_name
            .replace(package_version)
            .is_some()
        {
            panic!("version_name should not be set in TOML");
        }

        if manifest
            .android_manifest
            .version_code
            .replace(version_code)
            .is_some()
        {
            panic!("version_code should not be set in TOML");
        }

        let target_sdk_version = *manifest
            .android_manifest
            .sdk
            .target_sdk_version
            .get_or_insert_with(|| ndk.default_target_platform());

        manifest
            .android_manifest
            .application
            .debuggable
            .get_or_insert_with(|| *cmd.profile() == Profile::Dev);

        let activity = &mut manifest.android_manifest.application.activity;

        // Add a default `MAIN` action to launch the activity, if the user didn't supply it by hand.
        if activity
            .intent_filter
            .iter()
            .all(|i| i.actions.iter().all(|f| f != "android.intent.action.MAIN"))
        {
            activity.intent_filter.push(IntentFilter {
                actions: vec!["android.intent.action.MAIN".to_string()],
                categories: vec!["android.intent.category.LAUNCHER".to_string()],
                data: vec![],
            });
        }

        // Export the sole Rust activity on Android S and up, if the user didn't explicitly do so.
        // Without this, apps won't start on S+.
        // https://developer.android.com/about/versions/12/behavior-changes-12#exported
        if target_sdk_version >= 31 {
            activity.exported.get_or_insert(true);
        }

        Ok(Self {
            cmd,
            ndk,
            manifest,
            build_dir,
            build_targets,
            device_serial,
        })
    }

    pub fn check(&self) -> Result<(), Error> {
        for target in &self.build_targets {
            let mut cargo = cargo_ndk(
                &self.ndk,
                *target,
                self.min_sdk_version(),
                self.cmd.target_dir(),
            )?;
            cargo.arg("check");
            if self.cmd.target().is_none() {
                let triple = target.rust_triple();
                cargo.arg("--target").arg(triple);
            }
            self.cmd.args().apply(&mut cargo);
            output_error(cargo)?;
        }
        Ok(())
    }

    pub fn build(&self, artifact: &Artifact) -> Result<Apk, Error> {
        // Set artifact specific manifest default values.
        let mut manifest = self.manifest.android_manifest.clone();

        if manifest.package.is_empty() {
            let name = artifact.name.replace('-', "_");
            manifest.package = match artifact.r#type {
                ArtifactType::Lib => format!("rust.{}", name),
                ArtifactType::Bin => format!("rust.{}", name),
                ArtifactType::Example => format!("rust.example.{}", name),
            };
        }

        if manifest.application.label.is_empty() {
            manifest.application.label = artifact.name.to_string();
        }

        manifest.application.activity.meta_data.push(MetaData {
            name: "android.app.lib_name".to_string(),
            value: artifact.name.replace('-', "_"),
        });

        let crate_path = self.cmd.manifest().parent().expect("invalid manifest path");

        let is_debug_profile = *self.cmd.profile() == Profile::Dev;

        let assets = self
            .manifest
            .assets
            .as_ref()
            .map(|assets| dunce::simplified(&crate_path.join(assets)).to_owned());
        let resources = self
            .manifest
            .resources
            .as_ref()
            .map(|res| dunce::simplified(&crate_path.join(res)).to_owned());
        let runtime_libs = self
            .manifest
            .runtime_libs
            .as_ref()
            .map(|libs| dunce::simplified(&crate_path.join(libs)).to_owned());
        let apk_name = self
            .manifest
            .apk_name
            .clone()
            .unwrap_or_else(|| artifact.name.to_string());

        let config = ApkConfig {
            ndk: self.ndk.clone(),
            build_dir: self.build_dir.join(artifact.build_dir()),
            apk_name,
            assets,
            resources,
            manifest,
            disable_aapt_compression: is_debug_profile,
            strip: self.manifest.strip,
            reverse_port_forward: self.manifest.reverse_port_forward.clone(),
        };
        let mut apk = config.create_apk()?;

        for target in &self.build_targets {
            let triple = target.rust_triple();
            let build_dir = self.cmd.build_dir(Some(triple));
            let artifact = self.cmd.artifact(artifact, Some(triple), CrateType::Cdylib);

            let mut cargo = cargo_ndk(
                &self.ndk,
                *target,
                self.min_sdk_version(),
                self.cmd.target_dir(),
            )?;
            cargo.arg("build");
            if self.cmd.target().is_none() {
                cargo.arg("--target").arg(triple);
            }
            self.cmd.args().apply(&mut cargo);

            output_error(cargo)?;

            let mut libs_search_paths =
                get_libs_search_paths(self.cmd.target_dir(), triple, self.cmd.profile().as_ref())?;
            libs_search_paths.push(build_dir.join("deps"));

            let libs_search_paths = libs_search_paths
                .iter()
                .map(|path| path.as_path())
                .collect::<Vec<_>>();

            apk.add_lib_recursively(&artifact, *target, libs_search_paths.as_slice())?;

            if let Some(runtime_libs) = &runtime_libs {
                apk.add_runtime_libs(runtime_libs, *target, libs_search_paths.as_slice())?;
            }
        }

        let profile_name = match self.cmd.profile() {
            Profile::Dev => "dev",
            Profile::Release => "release",
            Profile::Custom(c) => c.as_str(),
        };

        let keystore_env = format!(
            "CARGO_APK_{}_KEYSTORE",
            profile_name.to_uppercase().replace('-', "_")
        );
        let password_env = format!("{}_PASSWORD", keystore_env);

        let path = std::env::var_os(&keystore_env).map(PathBuf::from);
        let password = std::env::var(&password_env).ok();

        let signing_key = match (path, password) {
            (Some(path), Some(password)) => Key { path, password },
            (Some(path), None) if is_debug_profile => {
                eprintln!(
                    "{} not specified, falling back to default password",
                    password_env
                );
                Key {
                    path,
                    password: ndk_build::ndk::DEFAULT_DEV_KEYSTORE_PASSWORD.to_owned(),
                }
            }
            (Some(path), None) => {
                eprintln!(
                    "`{}` was specified via `{}`, but `{}` was not specified, both or neither must be present for profiles other than `dev`",
                    path.display(),
                    keystore_env,
                    password_env
                );
                return Err(Error::MissingReleaseKey(profile_name.to_owned()));
            }
            (None, _) => {
                if let Some(msk) = self.manifest.signing.get(profile_name) {
                    Key {
                        path: crate_path.join(&msk.path),
                        password: msk.keystore_password.clone(),
                    }
                } else if is_debug_profile {
                    self.ndk.debug_key()?
                } else {
                    return Err(Error::MissingReleaseKey(profile_name.to_owned()));
                }
            }
        };

        let unsigned = apk.add_pending_libs_and_align()?;

        println!(
            "Signing `{}` with keystore `{}`",
            config.apk().display(),
            signing_key.path.display()
        );
        Ok(unsigned.sign(signing_key)?)
    }

    pub fn run(&self, artifact: &Artifact, no_logcat: bool) -> Result<(), Error> {
        let apk = self.build(artifact)?;
        apk.reverse_port_forwarding(self.device_serial.as_deref())?;
        apk.install(self.device_serial.as_deref())?;
        apk.start(self.device_serial.as_deref())?;
        //let uid = apk.uidof(self.device_serial.as_deref())?;

        if !no_logcat {
            let mut waiting = false;
            let pid = loop {
                sleep(Duration::from_millis(250));
                let out = self
                    .ndk
                    .adb(self.device_serial.as_deref())?
                    .arg("shell")
                    .arg("pidof")
                    .arg(apk.package())
                    .output()?;
                if out.status.success() {
                    break out.stdout;
                } else if !waiting {
                    waiting = true;
                    eprintln!("Waiting for the app to start!");
                }
            };
            let Ok(pid) = String::from_utf8(pid) else {
                eprintln!("App not running!");
                exit(1);
            };
            let mut process = self
                .ndk
                .adb(self.device_serial.as_deref())?
                .arg("logcat")
                .arg("-v")
                .arg("color")
                .arg("--pid")
                .arg(pid.trim())
                .spawn()?;
            loop {
                sleep(Duration::from_secs(1));
                if matches!(
                    self.ndk
                        .adb(self.device_serial.as_deref())?
                        .arg("shell")
                        .arg("pidof")
                        .arg(apk.package())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .stdin(Stdio::null())
                        .status()
                        .map(|x| x.success()),
                    Err(_) | Ok(false)
                ) {
                    break;
                }
            }
            sleep(Duration::from_millis(250));
            process.kill()?;
        }

        Ok(())
    }

    pub fn gdb(&self, artifact: &Artifact) -> Result<(), Error> {
        let apk = self.build(artifact)?;
        apk.install(self.device_serial.as_deref())?;

        let target_dir = self.build_dir.join(artifact.build_dir());
        self.ndk.ndk_gdb(
            target_dir,
            "android.app.NativeActivity",
            self.device_serial.as_deref(),
        )?;
        Ok(())
    }

    pub fn default(&self, cargo_cmd: &str, cargo_args: &[String]) -> Result<(), Error> {
        for target in &self.build_targets {
            let mut cargo = cargo_ndk(
                &self.ndk,
                *target,
                self.min_sdk_version(),
                self.cmd.target_dir(),
            )?;
            cargo.arg(cargo_cmd);
            self.cmd.args().apply(&mut cargo);

            if self.cmd.target().is_none() {
                let triple = target.rust_triple();
                cargo.arg("--target").arg(triple);
            }

            for additional_arg in cargo_args {
                cargo.arg(additional_arg);
            }

            output_error(cargo)?;
        }
        Ok(())
    }

    /// Returns `minSdkVersion` for use in compiler target selection:
    /// <https://developer.android.com/ndk/guides/sdk-versions#minsdkversion>
    ///
    /// Has a lower bound of `23` to retain backwards compatibility with
    /// the previous default.
    fn min_sdk_version(&self) -> u32 {
        self.manifest
            .android_manifest
            .sdk
            .min_sdk_version
            .unwrap_or(23)
            .max(23)
    }
}
