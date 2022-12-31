/*!

The `backend` module allows one to link between glium and the OpenGL context..

There are three concepts in play:

 - The `Backend` trait describes the glue between glium and the OpenGL context provider like
   glutin, SDL, GLFW, etc.
 - The `Context` struct is the main brick of glium. It manages everything that glium needs to
   execute OpenGL commands. Creating a `Context` requires a `Backend`.
 - The `Facade` trait. Calling functions like `VertexBuffer::new` requires passing an object
   that implements this trait. It is implemented on `Rc<Context>`.

*/
use std::rc::Rc;
use std::ops::Deref;
use std::os::raw::c_void;

use crate::CapabilitiesSource;

use crate::context::Capabilities;
use crate::context::ExtensionsList;
use crate::version::Version;

pub use crate::context::Context;
pub use crate::context::ReleaseBehavior;

#[cfg(feature = "glutin")]
pub mod glutin;

/// Trait for types that can be used as a backend for a glium context.
///
/// This trait is unsafe, as you can get undefined behaviors or crashes if you don't implement
/// the methods correctly.
pub unsafe trait Backend {
    /// Returns the address of an OpenGL function.
    ///
    /// Supposes that the context has been made current before this function is called.
    unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void;

    /// Returns the dimensions of the window, or screen, etc.
    fn get_framebuffer_dimensions(&self) -> (u32, u32);

    /// Returns the dimensions of the window, or screen, etc.
    fn set_framebuffer_dimensions(&self, new_dimensions: (u32, u32));

    /// Returns true if the OpenGL context is the current one in the thread.
    fn is_current(&self) -> bool;
}

unsafe impl<T> Backend for Rc<T> where T: Backend {
    unsafe fn get_proc_address(&self, symbol: &str) -> *const c_void {
        self.deref().get_proc_address(symbol)
    }

    fn get_framebuffer_dimensions(&self) -> (u32, u32) {
        self.deref().get_framebuffer_dimensions()
    }

    fn set_framebuffer_dimensions(&self, new_dimensions: (u32, u32)) {
        self.deref().set_framebuffer_dimensions(new_dimensions);
    }

    fn is_current(&self) -> bool {
        self.deref().is_current()
    }
}

/// Trait for types that provide a safe access for glium functions.
pub trait Facade {
    /// Returns an opaque type that contains the OpenGL state, extensions, version, etc.
    fn get_context(&self) -> &Rc<Context>;
}

impl<T: ?Sized> CapabilitiesSource for T where T: Facade {
    fn get_version(&self) -> &Version {
        self.get_context().deref().get_opengl_version()
    }

    fn get_extensions(&self) -> &ExtensionsList {
        self.get_context().deref().get_extensions()
    }

    fn get_capabilities(&self) -> &Capabilities {
        self.get_context().deref().get_capabilities()
    }
}

impl Facade for Rc<Context> {
    #[inline]
    fn get_context(&self) -> &Rc<Context> {
        self
    }
}
