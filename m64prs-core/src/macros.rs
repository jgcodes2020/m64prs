/// Macro wrapping closure-like syntax, generating an appropriately-typed `unsafe extern "C" fn()` instead.
#[macro_export]
#[doc(hidden)]
macro_rules! __vidext_closure {
    ( | $($param:ident : $ptype:ty),* $(,)? | $(-> $rtype:ty)? { $($code:tt)* } ) => {
        {
            {
                unsafe extern "C" fn f($($param: $ptype),*) $(-> $rtype)? {
                    $($code)*
                }
                Some(f)
            }
        }

    };
    ( || $(-> $rtype:ty)? { $($code:tt)* } ) => {
        {
            {
                unsafe extern "C" fn f() $(-> $rtype)? {
                    $($code)*
                }
                Some(f)
            }
        }
    };
}

/// Macro for testing an FFI result generated by the video extension.
#[macro_export]
#[doc(hidden)]
macro_rules! __try_ffi_result {
    ($e:expr) => {
        match ($e) {
            Ok(x) => x,
            Err(error) => return error.into(),
        }
    };
}

/// Utility macro for generating a function table from a static object implementing [`VideoExtension`][crate::types::VideoExtension].
/// 
/// # Syntax
/// ```ignore
/// vidext_table!([&mut instance_expr()] pub TABLE_NAME)
/// ```
/// - The brackets must contain an expression evaluating to a value of type `&mut impl DerefMut<Target = impl VideoExtension>`.
/// - It is expected that the bracketed expression references the same object each time it is called. Not doing so is undefined behaviour.
/// - The resulting table is a static object. It may be encapsulated in a method if desired.
/// 
/// ```ignore
/// vidext_table!(pub TABLE_NAME: Type)
/// ```
/// - The type must be `impl VideoExtension`.
/// - The visibility qualifier may be omitted.
/// - The resulting wrapper assumes that the object isn't thread-safe; attempting to use the video extension 
/// outside of the thread it was first created on results in a panic.
/// - The resulting table is a static object. It may be encapsulated in a method if desired.
/// 
/// ```ignore
/// vidext_table!(pub thread_safe TABLE_NAME: Type)
/// ```
/// - The type must be `impl VideoExtension`.
/// - The visibility qualifier may be omitted.
/// - The resulting wrapper assumes thread-safety, but not concurrency-safety. Use of the video extension
/// from multiple threads is controlled by a [`Mutex`][::std::sync::Mutex].
/// - The resulting table is a static object. It may be encapsulated in a method if desired.
///
/// You may also add a visibility qualifer before the table name; this gives the resulting table that visibility.
///
/// # Example
///
/// ```no_run
/// # use std::{ffi::{c_char, c_int, c_void, CStr}, sync::Mutex};
/// # use m64prs_core::{ctypes::{RenderMode, Size2D, VideoFlags, VideoMode}, types::{FFIResult, VideoExtension}};
/// #
/// struct VidextState {}
///
/// impl VideoExtension for VidextState {
///     // ...
/// #    unsafe fn init_with_render_mode(&mut self, mode: RenderMode) -> FFIResult<()> {
/// #        let _ = mode;
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn quit(&mut self) -> FFIResult<()> {
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn list_fullscreen_modes(&mut self) -> FFIResult<&'static [Size2D]> {
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn list_fullscreen_rates(&mut self, size: Size2D) -> FFIResult<&'static [c_int]> {
/// #        let _ = size;
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn set_video_mode(
/// #        &mut self,
/// #        width: c_int,
/// #        height: c_int,
/// #        bits_per_pixel: c_int,
/// #        screen_mode: VideoMode,
/// #        flags: VideoFlags,
/// #    ) -> FFIResult<()> {
/// #        let _ = (width, height, bits_per_pixel, screen_mode, flags);
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn set_video_mode_with_rate(
/// #        &mut self,
/// #        width: c_int,
/// #        height: c_int,
/// #        refresh_rate: c_int,
/// #        bits_per_pixel: c_int,
/// #        screen_mode: VideoMode,
/// #        flags: VideoFlags,
/// #    ) -> FFIResult<()> {
/// #        let _ = (width, height, refresh_rate, bits_per_pixel, screen_mode, flags);
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn set_caption(&mut self, title: &CStr) -> FFIResult<()> {
/// #        let _ = title;
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn toggle_full_screen(&mut self) -> FFIResult<()> {
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn resize_window(&mut self, width: c_int, height: c_int) -> FFIResult<()> {
/// #        let _ = (width, height);
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn gl_get_proc_address(&mut self, symbol: &CStr) -> *mut c_void {
/// #        let _ = symbol;
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn gl_set_attribute(&mut self, attr: m64prs_core::ctypes::GLAttribute, value: c_int) -> FFIResult<()> {
/// #        let _ = (attr, value);
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn gl_get_attribute(&mut self, attr: m64prs_core::ctypes::GLAttribute) -> FFIResult<c_int> {
/// #        let _ = attr;
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn gl_swap_buffers(&mut self) -> FFIResult<()> {
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn gl_get_default_framebuffer(&mut self) -> u32 {
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn vk_get_surface(&mut self, inst: ash::vk::Instance) -> FFIResult<ash::vk::SurfaceKHR> {
/// #        let _ = inst;
/// #        todo!()
/// #    }
/// #
/// #    unsafe fn vk_get_instance_extensions(&mut self) -> FFIResult<&'static [*const c_char]> {
/// #        todo!()
/// #    }
/// }
/// 
/// vidext_table!(pub VIDEXT_TABLE: VidextState);
/// ```
#[macro_export]
macro_rules! vidext_table {
    ([$inst:expr] $pub:vis $name:ident) => {
        $pub static $name: ::m64prs_sys::VideoExtensionFunctions = ::m64prs_sys::VideoExtensionFunctions {
            Functions: 17,
            VidExtFuncInit: ::m64prs_core::__vidext_closure!(|| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::init_with_render_mode(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    ::m64prs_sys::RenderMode::OpenGl,
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncQuit: ::m64prs_core::__vidext_closure!(|| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::quit(::std::ops::DerefMut::deref_mut(
                    ($inst)
                ),));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncListModes: ::m64prs_core::__vidext_closure!(|size_array: *mut ::m64prs_sys::Size2D,
                                                num_sizes: *mut ::std::ffi::c_int|
            -> ::m64prs_sys::Error {
                let size_in_src = ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::list_fullscreen_modes(
                    ::std::ops::DerefMut::deref_mut(($inst))
                ));
                let size_in = size_in_src.as_ref();
                let size_out =
                    ::std::slice::from_raw_parts_mut(size_array, (*num_sizes).try_into().unwrap());

                if size_in.len() < size_out.len() {
                    size_out[..size_in.len()].copy_from_slice(size_in);
                    *num_sizes = size_in.len().try_into().unwrap();
                } else {
                    size_out.copy_from_slice(&size_in[..size_out.len()]);
                    *num_sizes = size_out.len().try_into().unwrap();
                }

                ::m64prs_sys::Error::Success
            }),
            VidExtFuncListRates: ::m64prs_core::__vidext_closure!(|size: ::m64prs_sys::Size2D,
                                                num_rates: *mut ::std::ffi::c_int,
                                                rates: *mut ::std::ffi::c_int|
            -> ::m64prs_sys::Error {
                let rate_in_src = ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::list_fullscreen_rates(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    size
                ));
                let rate_in = rate_in_src.as_ref();
                let rate_out = ::std::slice::from_raw_parts_mut(rates, *num_rates as usize);

                if rate_in.len() < rate_out.len() {
                    rate_out[..rate_in.len()].copy_from_slice(rate_in);
                    *num_rates = rate_in.len().try_into().unwrap();
                } else {
                    rate_out.copy_from_slice(&rate_in[..rate_out.len()]);
                    *num_rates = rate_out.len().try_into().unwrap();
                }

                ::m64prs_sys::Error::Success
            }),
            VidExtFuncSetMode: ::m64prs_core::__vidext_closure!(|width: ::std::ffi::c_int,
                                                height: ::std::ffi::c_int,
                                                bits_per_pixel: ::std::ffi::c_int,
                                                screen_mode: ::std::ffi::c_int,
                                                flags: ::std::ffi::c_int|
            -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::set_video_mode(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    width,
                    height,
                    bits_per_pixel,
                    match ::m64prs_sys::VideoMode::try_from(screen_mode as u32) {
                        Ok(val) => val,
                        Err(_) => return ::m64prs_sys::Error::InputAssert
                    },
                    ::m64prs_sys::VideoFlags(flags as u32)
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncSetModeWithRate: ::m64prs_core::__vidext_closure!(|width: ::std::ffi::c_int,
                                                        height: ::std::ffi::c_int,
                                                        rate: ::std::ffi::c_int,
                                                        bits_per_pixel: ::std::ffi::c_int,
                                                        screen_mode: ::std::ffi::c_int,
                                                        flags: ::std::ffi::c_int|
            -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::set_video_mode_with_rate(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    width,
                    height,
                    rate,
                    bits_per_pixel,
                    match ::m64prs_sys::VideoMode::try_from(screen_mode as u32) {
                        Ok(val) => val,
                        Err(_) => return ::m64prs_sys::Error::InputAssert
                    },
                    ::m64prs_sys::VideoFlags(flags as u32)
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncGLGetProc: ::m64prs_core::__vidext_closure!(
                |symbol: *const ::std::ffi::c_char| -> Option<unsafe extern "C" fn()> {
                    let ptr = ::m64prs_core::types::VideoExtension::gl_get_proc_address(
                        ::std::ops::DerefMut::deref_mut(($inst)),
                        ::std::ffi::CStr::from_ptr(symbol),
                    );
                    Some(::std::mem::transmute(ptr))
                }
            ),
            VidExtFuncGLSetAttr: ::m64prs_core::__vidext_closure!(|attr: ::m64prs_sys::GLAttribute, value: ::std::ffi::c_int| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::gl_set_attribute(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    attr,
                    value,
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncGLGetAttr: ::m64prs_core::__vidext_closure!(|attr: ::m64prs_sys::GLAttribute,
                                                value_out: *mut ::std::ffi::c_int|
            -> ::m64prs_sys::Error {
                *value_out = ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::gl_get_attribute(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    attr
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncGLSwapBuf: ::m64prs_core::__vidext_closure!(|| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::gl_swap_buffers(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncSetCaption: ::m64prs_core::__vidext_closure!(|title: *const ::std::ffi::c_char| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(VideoExtension::set_caption(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    ::std::ffi::CStr::from_ptr(title)
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncToggleFS: ::m64prs_core::__vidext_closure!(|| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::toggle_full_screen(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncResizeWindow: ::m64prs_core::__vidext_closure!(|width: ::std::ffi::c_int, height: ::std::ffi::c_int| -> ::m64prs_sys::Error {
                ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::resize_window(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    width,
                    height
                ));
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncGLGetDefaultFramebuffer: ::m64prs_core::__vidext_closure!(|| -> u32 {
                ::m64prs_core::types::VideoExtension::gl_get_default_framebuffer(::std::ops::DerefMut::deref_mut(
                    ($inst),
                ))
            }),
            VidExtFuncInitWithRenderMode: ::m64prs_core::__vidext_closure!(
                |render_mode: ::m64prs_sys::RenderMode| -> ::m64prs_sys::Error {
                    ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::init_with_render_mode(
                        ::std::ops::DerefMut::deref_mut(($inst)),
                        render_mode,
                    ));
                    ::m64prs_sys::Error::Success
                }
            ),
            VidExtFuncVKGetSurface: ::m64prs_core::__vidext_closure!(|surface_out: *mut *mut ::std::ffi::c_void,
                                                     instance: *mut ::std::ffi::c_void|
            -> ::m64prs_sys::Error {
                let surface = ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::vk_get_surface(
                    ::std::ops::DerefMut::deref_mut(($inst)),
                    <::ash::vk::Instance as ::ash::vk::Handle>::from_raw(instance as u64)
                ));
                *surface_out = ::ash::vk::Handle::as_raw(surface) as *mut ::std::ffi::c_void;
                ::m64prs_sys::Error::Success
            }),
            VidExtFuncVKGetInstanceExtensions: ::m64prs_core::__vidext_closure!(
                |extensions_out: *mut *mut *const ::std::ffi::c_char,
                extensions_count_out: *mut u32|
                -> ::m64prs_sys::Error {
                    let extensions_slice = ::m64prs_core::__try_ffi_result!(::m64prs_core::types::VideoExtension::vk_get_instance_extensions(
                        ::std::ops::DerefMut::deref_mut(($inst))
                    ));
                    *extensions_out = extensions_slice.as_ptr() as *mut *const ::std::ffi::c_char;
                    *extensions_count_out = extensions_slice.len().try_into().unwrap();
                    ::m64prs_sys::Error::Success
                }
            ),
        };
    };
    ($pub:vis $name:ident : $VT:ty) => {
        ::m64prs_core::reexports::paste! {
            #[allow(non_snake_case_name)]
            fn [<__ $name _get_instance>]() -> ::std::cell::RefMut<'static, $VT> {
                static INSTANCE: ::std::sync::OnceLock<::m64prs_core::reexports::SendWrapper<::std::cell::RefCell<$VT>>> =
                    ::std::sync::OnceLock::new();
                INSTANCE.get_or_init(|| ::m64prs_core::reexports::SendWrapper::new($VT::default().into())).borrow_mut()
            }
            ::m64prs_core::vidext_table!([&mut [<__ $name _get_instance>]()] $pub $name);
        }
    };
    ($pub:vis thread_safe $name:ident : $VT:ty) => {
        ::m64prs_core::reexports::paste! {
            #[allow(non_snake_case_name)]
            fn [<__ $name _get_instance>]() -> ::std::sync::MutexGuard<'static, $VT> {
                static INSTANCE: ::std::sync::OnceLock<::std::sync::Mutex<$VT>> = ::std::sync::OnceLock::new();
                INSTANCE.get_or_init(|| ::std::sync::Mutex::new($VT::default())).lock().unwrap()
            }
            ::m64prs_core::vidext_table!([&mut [<__ $name _get_instance>]()] $pub $name);
        }
    };
}
