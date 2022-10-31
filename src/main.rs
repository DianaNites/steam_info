use std::{
    ffi::{CStr, CString},
    fs,
    num::NonZeroU32,
    path::Path,
};

use anyhow::{anyhow, Result};
use glutin::{
    config::ConfigTemplateBuilder,
    context::{ContextApi, ContextAttributesBuilder},
    display::{Display, DisplayApiPreference},
    prelude::{GlDisplay, NotCurrentGlContextSurfaceAccessor},
    surface::{SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    event::Event,
    event_loop::EventLoop,
    platform::{run_return::EventLoopExtRunReturn, unix::register_xlib_error_hook},
    window::WindowBuilder,
};

mod gl {
    #![allow(warnings)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

mod imp {
    use std::{ffi::CStr, mem::MaybeUninit};

    // TODO: Replace with linapi
    use libc::utsname;

    #[inline]
    fn to_cstr<'a>(ptr: *const u8) -> &'a CStr {
        // SAFETY:
        // kernel is always null terminated
        unsafe { CStr::from_ptr(ptr.cast()) }
    }

    /// Uname struct
    pub struct Uname(utsname);

    impl Uname {
        /// System name
        pub fn sys_name(&self) -> &str {
            to_cstr(self.0.sysname.as_ptr().cast())
                .to_str()
                .expect("non-ascii uname")
        }

        /// OS release version
        pub fn release(&self) -> &str {
            to_cstr(self.0.release.as_ptr().cast())
                .to_str()
                .expect("non-ascii uname")
        }

        /// Hardware architecture
        pub fn machine(&self) -> &str {
            to_cstr(self.0.machine.as_ptr().cast())
                .to_str()
                .expect("non-ascii uname")
        }
    }

    /// Uname
    pub fn uname() -> Uname {
        let mut uts: MaybeUninit<utsname> = MaybeUninit::uninit();
        // SAFETY: pointer is never null
        let u = unsafe { libc::uname(uts.as_mut_ptr()) };
        assert!(u >= 0, "uname call failed, should be impossible");
        // SAFETY: uname call initialized.
        let uts = unsafe { uts.assume_init() };
        Uname(uts)
    }
}
use imp::*;

fn gl_string(gl: &gl::Gles2, renderer: u32) -> Option<&CStr> {
    // SAFETY:
    // GetString surely is always safe???
    // CStr is not given invalid pointers
    unsafe {
        let s = gl.GetString(renderer);
        (!s.is_null()).then(|| CStr::from_ptr(s.cast()))
    }
}

fn get_cpu() -> Result<String> {
    let path = Path::new("/proc/cpuinfo");
    let data = fs::read_to_string(path)?;
    let model = data
        .lines()
        .find(|s| s.starts_with("model name"))
        .ok_or_else(|| anyhow!("Couldn't get CPU model"))?;
    let (_, model) = model
        .split_once(':')
        .ok_or_else(|| anyhow!("Couldn't get CPU model"))?;
    Ok(model.trim().into())
}

fn get_mem() -> Result<String> {
    let path = Path::new("/proc/meminfo");
    let data = fs::read_to_string(path)?;
    let model = data
        .lines()
        .find(|s| s.starts_with("MemTotal"))
        .ok_or_else(|| anyhow!("Couldn't get RAM info"))?;
    let (mem, unit) = model
        .split_once(':')
        .ok_or_else(|| anyhow!("Couldn't get RAM info"))
        .map(|s| s.1.trim())?
        .split_once(' ')
        .ok_or_else(|| anyhow!("Couldn't get RAM info"))?;
    let mem: u64 = mem.parse()?;
    let mem = match unit {
        "kB" => mem / 1024,
        // Will linux ever have a different unit?
        // probably not
        _ => return Err(anyhow!("Couldn't get RAM info")),
    };
    Ok(format!("{mem} MB"))
}

fn get_os_name() -> Result<String> {
    let path = Path::new("/etc/lsb-release");
    let data = fs::read_to_string(path)?;
    let name = data
        .lines()
        .find(|s| s.starts_with("DISTRIB_DESCRIPTION="))
        .ok_or_else(|| anyhow!("Couldn't get LSB OS name"))?;
    let (_, name) = name
        .split_once('=')
        .ok_or_else(|| anyhow!("Couldn't get LSB OS name"))?;

    Ok(name.trim().into())
}

fn get_driver() -> Result<(String, String)> {
    let mut driver: Option<String> = None;
    let mut driver_version: Option<String> = None;

    let mut event_loop = EventLoop::new();
    let raw_display = event_loop.raw_display_handle();
    // SAFETY:
    // Display is valid for the lifetime of gl.
    // Preferences are valid.
    let display = unsafe {
        Display::new(
            raw_display,
            DisplayApiPreference::EglThenGlx(Box::new(register_xlib_error_hook)),
        )?
    };

    let context_attributes = ContextAttributesBuilder::new()
        .with_profile(glutin::context::GlProfile::Compatibility)
        .build(None);
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_profile(glutin::context::GlProfile::Compatibility)
        .with_context_api(ContextApi::Gles(None))
        .build(None);

    // SAFETY:
    // ConfigTemplateBuilder is never given a RawWindowHandle.
    let config = unsafe { display.find_configs(ConfigTemplateBuilder::new().build()) }?
        .next()
        .ok_or_else(|| anyhow!("Couldn't get OpenGL info"))?;

    let mut not_current_gl_context = Some(unsafe {
        display
            .create_context(&config, &context_attributes)
            .or_else(|_| {
                display
                    .create_context(&config, &fallback_context_attributes)
                    .map_err(|_| anyhow!("Couldn't get OpenGL info"))
            })?
    });

    event_loop.run_return(|event, event_loop_window_target, control_flow| {
        if !matches!(event, Event::Resumed) {
            return;
        }
        control_flow.set_exit();
        let window = match WindowBuilder::new().build(event_loop_window_target) {
            Ok(w) => w,
            Err(_) => return,
        };

        let surface = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window.raw_window_handle(),
            NonZeroU32::new(1).unwrap(),
            NonZeroU32::new(1).unwrap(),
        );

        // SAFETY:
        // RawWindowHandle is valid
        let surface = match unsafe { display.create_window_surface(&config, &surface) } {
            Ok(s) => s,
            Err(_) => return,
        };

        let _context = match not_current_gl_context
            .take()
            .map(|c| c.make_current(&surface))
        {
            Some(Ok(g)) => g,
            _ => return,
        };

        let gl = gl::Gles2::load_with(|symbol| {
            if let Ok(symbol) = CString::new(symbol) {
                display.get_proc_address(symbol.as_c_str()).cast()
            } else {
                std::ptr::null()
            }
        });

        if let Some(vendor) = gl_string(&gl, gl::VENDOR) {
            let v = vendor.to_string_lossy();
            if let Some(renderer) = gl_string(&gl, gl::RENDERER) {
                let r = renderer.to_string_lossy();
                driver = Some(format!("{v} {r}"));
            }
        }

        if let Some(version) = gl_string(&gl, gl::VERSION) {
            driver_version = Some(version.to_string_lossy().into());
        }
    });

    Ok((
        driver.ok_or_else(|| anyhow!("Couldn't get OpenGL info"))?,
        driver_version.ok_or_else(|| anyhow!("Couldn't get OpenGL info"))?,
    ))
}

fn main() -> Result<()> {
    let cpu = get_cpu()?;
    let uname = uname();
    let os_name = get_os_name()?;
    let os_arch = match uname.machine() {
        "x86_64" => "64 bit",
        a => a,
    };
    let kernel_name = uname.sys_name();
    let kernel_version = uname.release();
    let (driver, driver_version) = get_driver()?;
    let ram = get_mem()?;
    println!(
        "\
System Info:


Processor Information:
    CPU Brand:  {cpu}

Operating System Version:
    {os_name} ({os_arch})
    Kernel Name:  {kernel_name}
    Kernel Version:  {kernel_version}

Video Card:
    Driver:  {driver}
    Driver Version:  {driver_version}



Memory:
    RAM:  {ram}
\
"
    );
    Ok(())
}
