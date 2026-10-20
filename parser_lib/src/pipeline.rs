//! Post-parse processing as a chain of handlers.
//!
//! A [`Chain`] runs [`ReplayHandler`]s in registration order over a parsed
//! [`ReplayInfo`]. [`Chain::with_defaults`] registers the built-in blueprint and display-name
//! resolution when game data is available; consumers append their own with [`Chain::with`].

use std::fmt;
use std::path::{Path, PathBuf};

use crate::attrib::AttribMount;
use crate::blueprints::BlueprintCatalogues;
use crate::module::ModuleFile;
use crate::names::{DisplayNames, DEFAULT_LANGUAGE};
use crate::replay::ReplayInfo;

/// An opened game install: the module file and its attrib mount.
pub struct GameData {
    module_path: PathBuf,
    install_dir: PathBuf,
    module_file: ModuleFile,
    mount: AttribMount,
}

impl GameData {
    /// Opens `module_path` and mounts its attrib archives and folders. The install dir is the
    /// module's parent directory.
    pub fn open(module_path: &Path) -> Result<Self, String> {
        let read_err = |e| format!("could not read module {}: {}", module_path.display(), e);
        let install_dir = module_path.parent().unwrap_or_else(|| Path::new("."));
        let module_file = ModuleFile::open(module_path).map_err(read_err)?;
        let mount = AttribMount::open(install_dir, &module_file).map_err(read_err)?;
        Ok(GameData {
            module_path: module_path.to_path_buf(),
            install_dir: install_dir.to_path_buf(),
            module_file,
            mount,
        })
    }

    pub fn module_path(&self) -> &Path {
        &self.module_path
    }

    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    pub fn module_file(&self) -> &ModuleFile {
        &self.module_file
    }

    pub fn mount(&self) -> &AttribMount {
        &self.mount
    }
}

/// Why a handler stopped. `Warn` is reported and the chain continues; `Fatal` ends the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HandlerError {
    Warn(String),
    Fatal(String),
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandlerError::Warn(msg) | HandlerError::Fatal(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for HandlerError {}

/// What a handler may read besides the replay. A struct so fields can be added without
/// changing handler signatures.
pub struct Context<'a> {
    game: Option<&'a GameData>,
}

impl Context<'_> {
    /// The game data the chain ran with; `None` when no module was available.
    pub fn game(&self) -> Option<&GameData> {
        self.game
    }
}

/// One step of post-parse processing.
pub trait ReplayHandler: Send + Sync {
    fn handle(&self, replay: &mut ReplayInfo, ctx: &Context) -> Result<(), HandlerError>;
}

/// Fills `replay.blueprints` and `Command.blueprint` from the module's catalogues.
struct BlueprintHandler {
    catalogues: BlueprintCatalogues,
    module: String,
}

impl ReplayHandler for BlueprintHandler {
    fn handle(&self, replay: &mut ReplayInfo, _ctx: &Context) -> Result<(), HandlerError> {
        crate::resolve_blueprints(replay, &self.catalogues, &self.module);
        Ok(())
    }
}

/// Fills `display_name` on resolved blueprints.
struct DisplayNameHandler<'a> {
    names: DisplayNames<'a>,
}

impl ReplayHandler for DisplayNameHandler<'_> {
    fn handle(&self, replay: &mut ReplayInfo, _ctx: &Context) -> Result<(), HandlerError> {
        crate::resolve_display_names(replay, &self.names);
        Ok(())
    }
}

/// Handlers run in registration order over one replay.
pub struct Chain<'a> {
    handlers: Vec<Box<dyn ReplayHandler + 'a>>,
}

impl<'a> Chain<'a> {
    /// A chain with no handlers.
    pub fn empty() -> Self {
        Chain {
            handlers: Vec::new(),
        }
    }

    /// The built-in handlers: blueprint resolution, then display names. With no game data
    /// nothing is registered. When display names fail to load, a warning is printed once and
    /// that handler is skipped.
    pub fn with_defaults(game: Option<&'a GameData>) -> Self {
        let mut chain = Chain::empty();
        let Some(game) = game else {
            return chain;
        };
        chain.push(BlueprintHandler {
            catalogues: BlueprintCatalogues::from_mount(game.mount()),
            module: game.module_path().to_string_lossy().into_owned(),
        });
        match DisplayNames::load(
            game.mount(),
            game.install_dir(),
            game.module_file(),
            DEFAULT_LANGUAGE,
        ) {
            Ok(names) => chain.push(DisplayNameHandler { names }),
            Err(e) => eprintln!("warning: display names unavailable: {}", e),
        }
        chain
    }

    /// Appends a handler, builder style.
    pub fn with(mut self, handler: impl ReplayHandler + 'a) -> Self {
        self.push(handler);
        self
    }

    /// Appends a handler.
    pub fn push(&mut self, handler: impl ReplayHandler + 'a) {
        self.handlers.push(Box::new(handler));
    }

    /// Runs every handler in order. A `Warn` is printed to stderr and the chain continues; a
    /// `Fatal` stops it and its message is returned.
    pub fn run(&self, replay: &mut ReplayInfo, game: Option<&GameData>) -> Result<(), String> {
        let ctx = Context { game };
        for handler in &self.handlers {
            match handler.handle(replay, &ctx) {
                Ok(()) => {}
                Err(HandlerError::Warn(msg)) => eprintln!("warning: {}", msg),
                Err(HandlerError::Fatal(msg)) => return Err(msg),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<&'static str>>>;

    struct Fake {
        name: &'static str,
        log: Log,
        result: Result<(), HandlerError>,
    }

    impl Fake {
        fn ok(name: &'static str, log: &Log) -> Fake {
            Fake {
                name,
                log: log.clone(),
                result: Ok(()),
            }
        }
    }

    impl ReplayHandler for Fake {
        fn handle(&self, _replay: &mut ReplayInfo, _ctx: &Context) -> Result<(), HandlerError> {
            self.log.lock().unwrap().push(self.name);
            self.result.clone()
        }
    }

    fn log() -> Log {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn run(chain: &Chain) -> Result<(), String> {
        chain.run(&mut ReplayInfo::default(), None)
    }

    #[test]
    fn runs_in_registration_order() {
        let log = log();
        let chain = Chain::empty()
            .with(Fake::ok("a", &log))
            .with(Fake::ok("b", &log))
            .with(Fake::ok("c", &log));
        assert_eq!(run(&chain), Ok(()));
        assert_eq!(*log.lock().unwrap(), ["a", "b", "c"]);
    }

    #[test]
    fn warn_continues() {
        let log = log();
        let warn = Fake {
            result: Err(HandlerError::Warn("careful".into())),
            ..Fake::ok("warn", &log)
        };
        let chain = Chain::empty().with(warn).with(Fake::ok("after", &log));
        assert_eq!(run(&chain), Ok(()));
        assert_eq!(*log.lock().unwrap(), ["warn", "after"]);
    }

    #[test]
    fn fatal_stops_the_chain() {
        let log = log();
        let fatal = Fake {
            result: Err(HandlerError::Fatal("boom".into())),
            ..Fake::ok("fatal", &log)
        };
        let chain = Chain::empty()
            .with(Fake::ok("before", &log))
            .with(fatal)
            .with(Fake::ok("after", &log));
        assert_eq!(run(&chain), Err("boom".to_string()));
        assert_eq!(*log.lock().unwrap(), ["before", "fatal"]);
    }

    #[test]
    fn defaults_without_game_register_nothing_but_custom_handlers_run() {
        let log = log();
        let chain = Chain::with_defaults(None);
        assert!(chain.handlers.is_empty());
        let chain = chain.with(Fake::ok("custom", &log));
        assert_eq!(run(&chain), Ok(()));
        assert_eq!(*log.lock().unwrap(), ["custom"]);
    }

    #[test]
    fn context_game_is_none_without_game_data() {
        struct Probe(Arc<Mutex<Option<bool>>>);
        impl ReplayHandler for Probe {
            fn handle(&self, _: &mut ReplayInfo, ctx: &Context) -> Result<(), HandlerError> {
                *self.0.lock().unwrap() = Some(ctx.game().is_none());
                Ok(())
            }
        }
        let seen = Arc::new(Mutex::new(None));
        let chain = Chain::empty().with(Probe(seen.clone()));
        assert_eq!(run(&chain), Ok(()));
        assert_eq!(*seen.lock().unwrap(), Some(true));
    }

    #[test]
    fn pipeline_types_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<GameData>();
        assert_send_sync::<Chain>();
        assert_send_sync::<HandlerError>();
        assert_send_sync::<dyn ReplayHandler>();
    }
}
