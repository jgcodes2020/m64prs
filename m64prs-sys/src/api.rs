use std::ffi::{c_char, c_int, c_uint, c_void};

use dlopen2::wrapper::{WrapperApi, WrapperMultiApi};

use crate::types::*;

#[derive(WrapperMultiApi)]
pub struct FullCoreApi {
    pub base: CoreBaseApi,
    pub tas: Option<CoreTasApi>,
}

#[derive(WrapperApi)]
pub struct CoreBaseApi {
    #[dlopen2_name = "PluginGetVersion"]
    get_version: unsafe extern "C" fn(
        plugin_type: *mut PluginType,
        plugin_version: *mut c_int,
        api_version: *mut c_int,
        plugin_name_ptr: *mut *const c_char,
        capabilities: *mut c_int,
    ) -> Error,
    #[dlopen2_name = "CoreErrorMessage"]
    error_message: unsafe extern "C" fn(return_code: Error) -> *const c_char,
    #[dlopen2_name = "CoreStartup"]
    startup: unsafe extern "C" fn(
        api_version: c_int,
        config_path: *const c_char,
        data_path: *const c_char,
        debug_context: *mut c_void,
        debug_callback: unsafe extern "C" fn(
            context: *mut c_void,
            level: c_int,
            message: *const c_char,
        ),
        state_context: *mut c_void,
        state_callback: unsafe extern "C" fn(
            context: *mut c_void,
            param: CoreParam,
            new_value: c_int,
        ),
    ) -> Error,
    #[dlopen2_name = "CoreShutdown"]
    shutdown: unsafe extern "C" fn() -> Error,
    #[dlopen2_name = "CoreAttachPlugin"]
    attach_plugin:
        unsafe extern "C" fn(plugin_type: PluginType, plugin_lib_handle: DynlibHandle) -> Error,
    #[dlopen2_name = "CoreDetachPlugin"]
    detach_plugin: unsafe extern "C" fn(plugin_type: PluginType) -> Error,
    #[dlopen2_name = "CoreDoCommand"]
    do_command:
        unsafe extern "C" fn(command: Command, int_param: c_int, ptr_param: *mut c_void) -> Error,
    #[dlopen2_name = "CoreOverrideVidExt"]
    override_vidext:
        unsafe extern "C" fn(video_function_struct: *mut VideoExtensionFunctions) -> Error,
}

#[derive(WrapperApi)]
pub struct CoreTasApi {
    set_input_callback: unsafe extern "C" fn(
        context: *mut c_void,
        callback: InputFilterCallback
    ) -> Error,
    set_audio_callbacks: unsafe extern "C" fn(
        context: *mut c_void,
        rate_callback: AudioRateCallbck,
    ),
    set_audio_tap_enabled: unsafe extern "C" fn(
        value: bool,
    ) -> Error
}

#[derive(WrapperApi)]
pub struct BasePluginApi {
    #[dlopen2_name = "PluginGetVersion"]
    get_version: unsafe extern "C" fn(
        plugin_type: *mut PluginType,
        plugin_version: *mut c_int,
        api_version: *mut c_int,
        plugin_name_ptr: *mut *const c_char,
        capabilities: *mut c_int,
    ) -> Error,
    #[dlopen2_name = "PluginStartup"]
    startup: unsafe extern "C" fn(
        core_lib_handle: DynlibHandle,
        debug_context: *mut c_void,
        debug_callback: unsafe extern "C" fn(
            context: *mut c_void,
            level: c_int,
            message: *const c_char,
        ),
    ) -> Error,
    #[dlopen2_name = "PluginShutdown"]
    shutdown: unsafe extern "C" fn() -> Error,
}
