//! Project creation, platform configuration and native folder picking.
use super::*;

impl StudioEnvironment {
    fn sdk() -> PathBuf {
        std::env::var_os("OMEN_SDK")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."))
    }
    pub fn create(&self) {
        if self.creating.get().is_some() {
            return;
        }
        self.project_parent.set(self.open_path.get());
        let mut manifest = omen_cli::project::template::default_manifest(
            &self.project_name.get(),
            Some(Self::sdk()),
        );
        manifest.targets.platforms = vec![omen_cli::project::manifest::TargetPlatform::Desktop];
        match toml::to_string_pretty(&manifest) {
            Ok(source) => {
                self.manifest_source.set(source);
                self.creation_error.set(String::new());
                self.creation_visible.set(true);
            }
            Err(error) => self
                .workspace
                .update(|state| state.message = error.to_string()),
        }
    }
    pub fn creating_project(&self) -> bool {
        self.creating.get().is_some()
    }
    pub fn project_name_input(&self) -> Binding<String> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || read.project_name.get(),
            move |name| {
                if write.creating_project() {
                    return;
                }
                let previous = write.project_name.get();
                write.project_name.set(name.clone());
                write.edit_creation_manifest(|manifest| {
                    let before = omen_cli::project::template::default_manifest(&previous, None);
                    let after = omen_cli::project::template::default_manifest(&name, None);
                    if manifest.app.name == before.app.name {
                        manifest.app.name = after.app.name;
                    }
                    if manifest.app.id == before.app.id {
                        manifest.app.id = after.app.id;
                    }
                    if manifest.window.title == before.window.title {
                        manifest.window.title = after.window.title;
                    }
                });
            },
        )
    }
    pub fn project_identifier(&self) -> Binding<String> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || {
                toml::from_str::<omen_cli::project::manifest::Manifest>(&read.manifest_source.get())
                    .map(|manifest| manifest.app.id)
                    .unwrap_or_default()
            },
            move |identifier| {
                if !write.creating_project() {
                    write.edit_creation_manifest(|manifest| manifest.app.id = identifier);
                }
            },
        )
    }
    fn edit_creation_manifest(
        &self,
        update: impl FnOnce(&mut omen_cli::project::manifest::Manifest),
    ) {
        let result = (|| -> Result<String, String> {
            let mut manifest = toml::from_str::<omen_cli::project::manifest::Manifest>(
                &self.manifest_source.get(),
            )
            .map_err(|error| error.to_string())?;
            update(&mut manifest);
            toml::to_string_pretty(&manifest).map_err(|error| error.to_string())
        })();
        match result {
            Ok(source) => {
                self.manifest_source.set(source);
                self.creation_error.set(String::new());
            }
            Err(error) => self.creation_error.set(error),
        }
    }
    pub fn folder_picker(&self, creating: bool) -> omen::FilePicker {
        let app = self.clone();
        let mut picker = omen::FilePicker::folders(move |result| match result {
            Ok(Some(paths)) => {
                if let Some(path) = paths.into_iter().next() {
                    if creating {
                        app.project_parent.set(path.to_string_lossy().into());
                        app.creation_error.set(String::new());
                    } else {
                        app.open_path.set(path.to_string_lossy().into());
                        app.send(Command::OpenProject(path));
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                if creating {
                    app.creation_error.set(error);
                } else {
                    app.workspace.update(|state| state.message = error);
                }
            }
        });
        picker.prompt = Some(
            if creating {
                "Choose location"
            } else {
                "Open project"
            }
            .into(),
        );
        picker
    }
    pub fn cancel_create(&self) {
        if self.creating.get().is_none() {
            self.creation_visible.set(false);
        }
    }
    pub fn submit_create(&self) {
        if self.creating.get().is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            if self.project_parent.get().trim().is_empty() {
                return Err("Choose a parent folder for the new project".into());
            }
            omen_cli::project::manifest::validate_name(&self.project_name.get())
                .map_err(|error| error.to_string())?;
            let manifest: omen_cli::project::manifest::Manifest =
                toml::from_str(&self.manifest_source.get()).map_err(|error| error.to_string())?;
            manifest.validate().map_err(|error| error.to_string())
        })();
        if let Err(error) = result {
            self.creation_error.set(error);
            return;
        }
        if self.service.is_none() {
            self.creation_error
                .set("Workspace worker is unavailable".into());
            return;
        }
        self.creation_error.set("Creating application…".into());
        self.send(Command::CreateConfigured {
            name: self.project_name.get(),
            parent: PathBuf::from(self.project_parent.get()),
            sdk: Self::sdk(),
            source: self.manifest_source.get(),
        });
        if self.workspace.get().message == "Working…" {
            self.creating.set(Some(self.sequence.get()));
        } else {
            self.creation_error.set(self.workspace.get().message);
        }
    }
    pub fn target(&self, target: omen_cli::project::manifest::TargetPlatform) -> Binding<bool> {
        let read = self.clone();
        let write = self.clone();
        Binding::new(
            move || {
                toml::from_str::<omen_cli::project::manifest::Manifest>(&read.manifest_source.get())
                    .is_ok_and(|manifest| manifest.targets.platforms.contains(&target))
            },
            move |enabled| {
                if write.creating.get().is_some() {
                    return;
                }
                let result = (|| -> Result<String, String> {
                    let mut manifest: omen_cli::project::manifest::Manifest =
                        toml::from_str(&write.manifest_source.get())
                            .map_err(|error| error.to_string())?;
                    manifest.targets.platforms.retain(|item| *item != target);
                    if enabled {
                        manifest.targets.platforms.push(target);
                    }
                    toml::to_string_pretty(&manifest).map_err(|error| error.to_string())
                })();
                match result {
                    Ok(source) => {
                        write.manifest_source.set(source);
                        write.creation_error.set(String::new());
                    }
                    Err(error) => write.creation_error.set(error),
                }
            },
        )
    }
}
