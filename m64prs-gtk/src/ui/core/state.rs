use std::{
    env,
    error::Error,
    path::Path,
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
};

use m64prs_core::{
    error::{M64PError, SavestateError},
    plugin::PluginSet,
    save::SavestateFormat,
    tas_callbacks::{InputHandler, SaveHandler},
    Core,
};
use m64prs_sys::{CoreParam, EmuState};
use m64prs_vcr::VcrState;
use num_enum::TryFromPrimitive;
use relm4::{ComponentSender, Sender};

use crate::ui::core::{
    vidext::{VideoExtensionParameters, VideoExtensionState},
    CoreRequest,
};

use super::{vidext::VidextResponse, CoreResponse, MupenCore};

#[derive(Debug)]
pub(super) enum CoreState {
    Uninit,
    Ready(CoreReadyState),
    Running(CoreRunningState),
}

#[derive(Debug)]
pub(super) struct CoreReadyState {
    core: Core,
}
#[derive(Debug)]
pub(super) struct CoreRunningState {
    core: Arc<Core>,
    join_handle: JoinHandle<Result<(), M64PError>>,
    vcr_read_only: bool,
    vcr_state: Arc<Mutex<Option<VcrState>>>,
}

struct CoreInputHandler {
    vcr_state: Arc<Mutex<Option<VcrState>>>,
}

struct CoreSaveHandler {
    vcr_state: Arc<Mutex<Option<VcrState>>>,
}

pub(super) type VidextSender = mpsc::Sender<(usize, VidextResponse)>;

impl Default for CoreState {
    fn default() -> Self {
        Self::Uninit
    }
}

impl CoreState {
    pub(super) fn is_uninit(&self) -> bool {
        matches!(self, Self::Uninit)
    }

    pub(super) fn take(&mut self) -> CoreState {
        std::mem::take(self)
    }

    pub(super) fn borrow_ready(&mut self) -> Option<&mut CoreReadyState> {
        match self {
            CoreState::Ready(ready_state) => Some(ready_state),
            _ => None,
        }
    }

    pub(super) fn borrow_running(&mut self) -> Option<&mut CoreRunningState> {
        match self {
            CoreState::Running(running_state) => Some(running_state),
            _ => None,
        }
    }
}

impl CoreReadyState {
    pub(super) fn new(sender: &ComponentSender<MupenCore>) -> (Self, VidextSender) {
        #[cfg(target_os = "windows")]
        const MUPEN_FILENAME: &str = "mupen64plus.dll";
        #[cfg(target_os = "macos")]
        const MUPEN_FILENAME: &str = "libmupen64plus.dylib";
        #[cfg(target_os = "linux")]
        const MUPEN_FILENAME: &str = "libmupen64plus.so";

        let self_path = env::current_exe().expect("should be able to find current_exe");
        let self_dir = self_path
            .parent()
            .expect("self path should be in a directory");

        let mupen_dll_path = self_dir.join(MUPEN_FILENAME);
        let data_dir = self_dir.join("data\\");

        log::info!("Loading M64+ from {}", mupen_dll_path.display());
        log::info!("Data path is {}", data_dir.display());

        let mut core = m64prs_core::Core::init(mupen_dll_path, None, Some(&data_dir))
            .expect("core startup should succeed");

        let (vidext, vidext_inbound) =
            VideoExtensionParameters::new(sender.output_sender().clone());

        core.override_vidext::<VideoExtensionState, _>(Some(vidext))
            .expect("vidext override should succeed");

        {
            let sender = sender.clone();
            core.listen_state(move |param, value| match param {
                CoreParam::EmuState => {
                    sender.input(CoreRequest::EmuStateChanged(
                        (value as <EmuState as TryFromPrimitive>::Primitive)
                            .try_into()
                            .unwrap(),
                    ));
                }
                CoreParam::SavestateSlot => {
                    let _ = sender.output(CoreResponse::SavestateSlotChanged(value as u8));
                }
                _ => (),
            });
        }

        (Self { core }, vidext_inbound)
    }

    pub(super) fn start_rom(
        mut self,
        rom_data: &[u8],
        plugins: PluginSet,
        sender: &Sender<CoreResponse>,
    ) -> Result<CoreRunningState, (Box<dyn Error + Send + Sync>, Self)> {
        if let Err(err) = self.core.open_rom(rom_data) {
            return Err((Box::new(err), self));
        }
        if let Err(err) = self.core.attach_plugins(plugins) {
            self.core.close_rom().unwrap();
            return Err((Box::new(err), self));
        }

        let mut core = self.core;
        let vcr_state = Arc::new(Mutex::new(None));

        let input_handler = CoreInputHandler { vcr_state: Arc::clone(&vcr_state) };
        core.set_input_handler(input_handler).expect("should be able to set input handler");

        let save_handler = CoreSaveHandler { vcr_state: Arc::clone(&vcr_state) };
        core.set_save_handler(save_handler).expect("should be able to set save handler");

        let core = Arc::new(core);
        let join_handle = {
            let core = Arc::clone(&core);
            thread::spawn(move || core.execute())
        };

        let vcr_read_only = true;

        sender.emit(CoreResponse::VcrReadOnlyChanged(vcr_read_only));

        Ok(CoreRunningState {
            core,
            join_handle,
            vcr_read_only,
            vcr_state,
        })
    }
}

impl CoreRunningState {
    pub(super) fn stop_rom(self) -> (CoreReadyState, Option<M64PError>) {
        // stop the core
        let _ = self.core.request_stop();
        let error = self.join_handle.join().unwrap().err();

        // this should now be the only remaining reference; so extract the core
        let mut core = Arc::into_inner(self.core).expect("this should be the only ref to core");

        core.clear_input_handler().expect("should be able to clear input handler");
        core.clear_save_handler().expect("should be able to clear save handler");

        (CoreReadyState { core }, error)
    }

    pub(super) fn toggle_pause(&mut self) -> Result<(), M64PError> {
        match self.core.emu_state() {
            EmuState::Running => self.core.request_pause(),
            EmuState::Paused => self.core.request_resume(),
            _ => unreachable!(),
        }
    }

    pub(super) fn advance_frame(&mut self) -> Result<(), M64PError> {
        self.core.request_advance_frame()
    }

    pub(super) fn reset(&mut self, hard: bool) -> Result<(), M64PError> {
        self.core.reset(hard)
    }

    pub(super) async fn save_slot(&mut self) -> Result<(), SavestateError> {
        self.core.save_slot().await
    }

    pub(super) async fn load_slot(&mut self) -> Result<(), SavestateError> {
        self.core.load_slot().await
    }

    pub(super) fn set_save_slot(&mut self, slot: u8) -> Result<(), M64PError> {
        self.core.set_state_slot(slot)
    }

    pub(super) async fn save_file<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<(), SavestateError> {
        self.core
            .save_file(path.as_ref(), SavestateFormat::Mupen64Plus)
            .await
    }

    pub(super) async fn load_file<P: AsRef<Path>>(
        &mut self,
        path: P,
    ) -> Result<(), SavestateError> {
        self.core.load_file(path.as_ref()).await
    }

    pub(super) fn toggle_read_only(&mut self, sender: &Sender<CoreResponse>) {
        self.vcr_read_only ^= true;
        sender.emit(CoreResponse::VcrReadOnlyChanged(self.vcr_read_only));
    }
}

impl InputHandler for CoreInputHandler {
    fn filter_inputs(
        &mut self,
        port: std::ffi::c_int,
        mut input: m64prs_sys::Buttons,
    ) -> m64prs_sys::Buttons {
        {
            let mut vcr_state = self.vcr_state.lock().unwrap();
            if let Some(vcr_state) = vcr_state.as_mut() {
                input = vcr_state.filter_inputs(port, input);
            }
        }
        input
    }

    fn poll_present(&mut self, port: std::ffi::c_int) -> bool {
        let mut vcr_state = self.vcr_state.lock().unwrap();
        if let Some(vcr_state) = vcr_state.as_mut() {
            vcr_state.poll_present(port)
        } else {
            false
        }
    }
}

impl SaveHandler for CoreSaveHandler {
    const SIGNATURE: u32 = u32::from_le_bytes([b'R', b'S', b'X', b'T']);

    fn save_xd(&mut self) -> Result<Box<[u8]>, Box<dyn Error>> {
        let mut vcr_state = self.vcr_state.lock().unwrap();
        if let Some(vcr_state) = vcr_state.as_mut() {
            Ok(
                bincode::serialize(&vcr_state.freeze())?
                    .into_boxed_slice(),
            )
        } else {
            Ok(Box::new([]))
        }
    }

    fn load_xd(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut vcr_state = self.vcr_state.lock().unwrap();
        if let Some(vcr_state) = vcr_state.as_mut() {
            let freeze = bincode::deserialize(data)?;
            vcr_state.load_freeze(freeze)?;
        }

        Ok(())
    }
}
