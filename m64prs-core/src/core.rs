use std::{
    ffi::{c_char, c_int, c_uint, c_void, CStr},
    fs,
    path::Path,
    pin::Pin,
    ptr::{null, null_mut},
    sync::{mpsc, Mutex},
};

use dlopen2::wrapper::Container;
use futures::{channel::oneshot, Future};
use log::{log, Level};

use crate::{
    ctypes::{self, Command, CoreParam, MsgLevel, PluginType},
    error::CoreError,
    types::APIVersion,
};

use crate::error::Result as CoreResult;

use self::st::{SavestateFuture, SavestateWaitManager, SavestateWaiter};

mod api;
mod st;

#[inline]
fn core_fn(err: ctypes::Error) -> CoreResult<()> {
    match err {
        ctypes::Error::SUCCESS => Ok(()),
        err => Err(CoreError::M64P(err.try_into().unwrap())),
    }
}

#[allow(unused)]
unsafe extern "C" fn debug_callback(context: *mut c_void, level: c_int, message: *const c_char) {
    let log_level = match MsgLevel(level as c_uint) {
        MsgLevel::ERROR => Level::Error,
        MsgLevel::WARNING => Level::Warn,
        MsgLevel::INFO => Level::Info,
        MsgLevel::STATUS => Level::Debug,
        MsgLevel::VERBOSE => Level::Trace,
        _ => panic!("Received invalid message level {}", level),
    };
    log!(log_level, "{}", CStr::from_ptr(message).to_str().unwrap());
}

#[allow(unused)]
extern "C" fn state_callback(context: *mut c_void, param: CoreParam, value: c_int) {
    let pinned_state = unsafe { &mut *(context as *mut PinnedCoreState) };

    match param {
        CoreParam::STATE_SAVECOMPLETE | CoreParam::STATE_LOADCOMPLETE => {
            pinned_state.st_wait_mgr.on_state_change(param, value);
        }
        _ => (),
    }
}

struct PinnedCoreState {
    pub st_wait_mgr: SavestateWaitManager,
}

static CORE_GUARD: Mutex<bool> = Mutex::new(false);

pub struct Core {
    api: Container<api::FullCoreApi>,
    plugins: Option<[Plugin; 4]>,
    sender: mpsc::Sender<SavestateWaiter>,
    pin_state: Pin<Box<PinnedCoreState>>,
}

unsafe impl Sync for Core {}

// initialization and cleanup
impl Core {
    /// Loads and starts up the core from a given path.
    ///
    /// # Panics
    /// Only one active instance of `Core` can exist at any given time. Attempting to create a
    /// second one will cause a panic.
    ///
    /// # Errors
    /// This function may error if:
    /// - Library loading fails ([`CoreError::Library`])
    /// - Initialization of Mupen64Plus fails ([`CoreError::M64P`])
    pub fn init(path: impl AsRef<Path>) -> CoreResult<Self> {
        let mut guard = CORE_GUARD.lock().unwrap();
        if *guard {
            drop(guard);
            panic!("Only one instance of Core may be created");
        }

        let api = unsafe { Container::<api::FullCoreApi>::load(path.as_ref()) }
            .map_err(CoreError::Library)?;

        let (tx, rx) = mpsc::channel();

        let mut core = Self {
            api,
            plugins: None,
            sender: tx,
            pin_state: Box::pin(PinnedCoreState {
                st_wait_mgr: SavestateWaitManager::new(rx),
            }),
        };

        core_fn(unsafe {
            core.api.core.startup(
                0x02_01_00,
                null(),
                null(),
                null_mut(),
                debug_callback,
                &mut *core.pin_state as *mut PinnedCoreState as *mut c_void,
                state_callback,
            )
        })?;

        *guard = true;
        Ok(core)
    }
}
impl Drop for Core {
    fn drop(&mut self) {
        // shutdown the core before it's freed
        unsafe { self.api.core.shutdown() };
        // drop the guard so that another core can be constructed
        {
            let mut guard = CORE_GUARD.lock().unwrap();
            *guard = false;
        }
    }
}

// Extension points (plugins and vidext)
impl Core {
    /// Attaches the four plugins to the core.
    ///
    /// # Errors
    /// This function can error if:
    /// - The plugins passed in do not match their supposed type (e.g. `gfx_plugin` expects a graphics plugin)
    /// - Starting up any of the four plugins fails
    /// - Attaching any of the four plugins fails
    pub fn attach_plugins(
        &mut self,
        mut gfx_plugin: Plugin,
        mut audio_plugin: Plugin,
        mut input_plugin: Plugin,
        mut rsp_plugin: Plugin,
    ) -> CoreResult<()> {
        if self.plugins.is_some() {
            panic!("Plugins have already been attached")
        }
        // check all plugin types
        if !gfx_plugin
            .get_type()
            .is_ok_and(|ptype| ptype == PluginType::GFX)
        {
            return Err(CoreError::PluginInvalid(PluginType::GFX));
        }
        if !audio_plugin
            .get_type()
            .is_ok_and(|ptype| ptype == PluginType::AUDIO)
        {
            return Err(CoreError::PluginInvalid(PluginType::AUDIO));
        }
        if !input_plugin
            .get_type()
            .is_ok_and(|ptype| ptype == PluginType::INPUT)
        {
            return Err(CoreError::PluginInvalid(PluginType::INPUT));
        }
        if !rsp_plugin
            .get_type()
            .is_ok_and(|ptype| ptype == PluginType::RSP)
        {
            return Err(CoreError::PluginInvalid(PluginType::RSP));
        }

        // startup the four plugins
        let core_ptr = unsafe { self.api.into_raw() };
        gfx_plugin.startup(core_ptr)?;
        audio_plugin.startup(core_ptr)?;
        input_plugin.startup(core_ptr)?;
        rsp_plugin.startup(core_ptr)?;

        // attach all plugins. If one fails, detach everything.
        if let Err(err) = core_fn(unsafe {
            self.api
                .core
                .attach_plugin(PluginType::GFX, gfx_plugin.api.into_raw())
        }) {
            unsafe { self.api.core.detach_plugin(PluginType::GFX) };
            return Err(err);
        }
        if let Err(err) = core_fn(unsafe {
            self.api
                .core
                .attach_plugin(PluginType::AUDIO, audio_plugin.api.into_raw())
        }) {
            unsafe { self.api.core.detach_plugin(PluginType::GFX) };
            unsafe { self.api.core.detach_plugin(PluginType::AUDIO) };
            return Err(err);
        }
        if let Err(err) = core_fn(unsafe {
            self.api
                .core
                .attach_plugin(PluginType::INPUT, input_plugin.api.into_raw())
        }) {
            unsafe { self.api.core.detach_plugin(PluginType::GFX) };
            unsafe { self.api.core.detach_plugin(PluginType::AUDIO) };
            unsafe { self.api.core.detach_plugin(PluginType::INPUT) };
            return Err(err);
        }
        if let Err(err) = core_fn(unsafe {
            self.api
                .core
                .attach_plugin(PluginType::RSP, rsp_plugin.api.into_raw())
        }) {
            unsafe { self.api.core.detach_plugin(PluginType::GFX) };
            unsafe { self.api.core.detach_plugin(PluginType::AUDIO) };
            unsafe { self.api.core.detach_plugin(PluginType::INPUT) };
            unsafe { self.api.core.detach_plugin(PluginType::RSP) };
            return Err(err);
        }

        self.plugins = Some([gfx_plugin, audio_plugin, input_plugin, rsp_plugin]);

        Ok(())
    }

    /// Detaches all plugins.
    pub fn detach_plugins(&mut self) {
        if self.plugins.is_none() {
            panic!("Plugins are not attached")
        }

        // detach plugins from core.
        unsafe { self.api.core.detach_plugin(PluginType::GFX) };
        unsafe { self.api.core.detach_plugin(PluginType::AUDIO) };
        unsafe { self.api.core.detach_plugin(PluginType::INPUT) };
        unsafe { self.api.core.detach_plugin(PluginType::RSP) };
        // drop the plugins. this shuts them down.
        self.plugins = None;
    }
}

// Synchronous core commands
impl Core {
    /// Opens a ROM that is pre-loaded into memory.
    pub fn open_rom(&mut self, rom_data: &[u8]) -> CoreResult<()> {
        core_fn(unsafe {
            self.api.core.do_command(
                Command::ROM_OPEN,
                rom_data.len() as c_int,
                rom_data.as_ptr() as *mut c_void,
            )
        })
    }

    /// Loads and opens a ROM from a given file path.
    pub fn load_rom(&mut self, path: impl AsRef<Path>) -> CoreResult<()> {
        let rom_data = fs::read(path.as_ref()).map_err(|err| CoreError::IO(err))?;
        self.open_rom(&rom_data)
    }

    /// Closes a currently open ROM.
    pub fn close_rom(&mut self) -> CoreResult<()> {
        core_fn(unsafe { self.api.core.do_command(Command::ROM_CLOSE, 0, null_mut()) })
    }

    /// Executes the currently-open ROM.
    pub fn execute(&self) -> CoreResult<()> {
        core_fn(unsafe { self.api.core.do_command(Command::EXECUTE, 0, null_mut()) })
    }
}

impl Core {
    pub fn stop(&self) -> CoreResult<()> {
        core_fn(unsafe { self.api.core.do_command(Command::STOP, 0, null_mut()) })
    }

    pub fn save_state(&self) -> CoreResult<SavestateFuture> {
        let (tx, rx) = oneshot::channel::<bool>();
        let future = SavestateFuture::new(rx);

        self.sender.send(SavestateWaiter {
            param: CoreParam::STATE_SAVECOMPLETE,
            tx,
        }).unwrap();

        core_fn(unsafe { self.api.core.do_command(Command::STATE_SAVE, 0, null_mut()) })?;

        Ok(future)
    }

    pub fn load_state(&self) -> CoreResult<SavestateFuture> {
        let (tx, rx) = oneshot::channel::<bool>();
        let future = SavestateFuture::new(rx);

        self.sender.send(SavestateWaiter {
            param: CoreParam::STATE_LOADCOMPLETE,
            tx,
        }).unwrap();

        core_fn(unsafe { self.api.core.do_command(Command::STATE_LOAD, 0, null_mut()) })?;

        Ok(future)
    }
}

/// Holds a loaded instance of a Mupen64Plus plugin.
///
/// The core is responsible for startup/shutdown of plugins; they are never started while you own them.
pub struct Plugin {
    api: Container<api::BasePluginApi>,
}

impl Plugin {
    /// Loads a plugin from a path.
    pub fn load<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let plugin = Plugin {
            api: unsafe { Container::load(path.as_ref()) }
                .map_err(|err| CoreError::Library(err))?,
        };

        Ok(plugin)
    }

    /// Gets the type of this plugin.
    pub fn get_type(&self) -> CoreResult<PluginType> {
        let mut plugin_type: PluginType = PluginType::NULL;
        core_fn(unsafe {
            self.api.get_version(
                &mut plugin_type,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        })?;

        Ok(plugin_type)
    }

    /// Obtains version information about this plugin.
    pub fn get_version(&self) -> CoreResult<APIVersion> {
        unsafe {
            let mut plugin_type: PluginType = PluginType::NULL;
            let mut plugin_version: c_int = 0;
            let mut api_version: c_int = 0;
            let mut plugin_name: *const c_char = null();

            core_fn(self.api.get_version(
                &mut plugin_type,
                &mut plugin_version,
                &mut api_version,
                &mut plugin_name,
                null_mut(),
            ))?;

            Ok(APIVersion {
                api_type: plugin_type.try_into().unwrap(),
                plugin_version: plugin_version,
                api_version: api_version,
                plugin_name: CStr::from_ptr(plugin_name).to_str().unwrap(),
                capabilities: 0,
            })
        }
    }

    fn startup(&mut self, core_ptr: *mut c_void) -> CoreResult<()> {
        core_fn(unsafe { self.api.startup(core_ptr, null_mut(), debug_callback) })
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        unsafe {
            self.api.shutdown();
        }
    }
}
